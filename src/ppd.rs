//! Reads a CUPS PPD into the capabilities the dialog needs.

use crate::layout::{Margins, Paper, SizeMm};
use std::collections::HashMap;

const MM_PER_POINT: f64 = 25.4 / 72.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrinterOption {
    pub keyword: String,
    pub label: String,
    pub choices: Vec<Choice>,
    pub default: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PaperChoice {
    pub id: String,
    pub label: String,
    pub paper: Paper,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrinterCaps {
    pub papers: Vec<PaperChoice>,
    pub default_paper: String,
    pub options: Vec<PrinterOption>,
}

impl PrinterCaps {
    pub fn paper(&self, id: &str) -> Option<&PaperChoice> {
        self.papers.iter().find(|p| p.id == id)
    }

    fn option(&self, keyword: &str) -> Option<&PrinterOption> {
        self.options.iter().find(|o| o.keyword == keyword)
    }

    /// Standard keywords first, then the names some drivers use instead.
    fn first(&self, keywords: &[&str]) -> Option<&PrinterOption> {
        keywords.iter().find_map(|k| self.option(k))
    }

    pub fn quality(&self) -> Option<&PrinterOption> {
        self.first(&[
            "Resolution",
            "cupsPrintQuality",
            "OutputMode",
            "Quality",
            "HPPrintQuality",
            "HPPJLPrintQuality",
        ])
    }

    pub fn paper_type(&self) -> Option<&PrinterOption> {
        self.first(&["MediaType", "BRMediaType", "OKMediaType"])
    }

    /// Used when a printer cannot tell us what it supports.
    pub fn fallback() -> PrinterCaps {
        let margins = Margins {
            left: 5.0,
            top: 5.0,
            right: 5.0,
            bottom: 5.0,
        };
        let paper = |id: &str, w, h| PaperChoice {
            id: id.into(),
            label: id.into(),
            paper: Paper {
                size: SizeMm { w, h },
                margins,
            },
        };
        PrinterCaps {
            papers: vec![paper("A4", 210.0, 297.0), paper("Letter", 215.9, 279.4)],
            default_paper: "A4".into(),
            options: Vec::new(),
        }
    }
}

/// PPDs are UTF-8 or, in older drivers, Latin-1.
pub fn decode(bytes: Vec<u8>) -> String {
    String::from_utf8(bytes)
        .unwrap_or_else(|e| e.into_bytes().into_iter().map(char::from).collect())
}

/// "id/Label" or just "id".
fn id_and_label(s: &str) -> (&str, &str) {
    let s = s.trim();
    s.split_once('/').unwrap_or((s, s))
}

fn hex_bytes(hex: &str) -> Option<Vec<u8>> {
    let digits: Vec<u8> = hex.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    if digits.is_empty() || !digits.len().is_multiple_of(2) {
        return None;
    }
    digits
        .chunks(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok())
        .collect()
}

/// A label as shown: PPD translations may write bytes as `<3C>`.
fn readable(label: &str) -> String {
    let mut text = String::new();
    let mut rest = label.trim();
    while let Some((before, after)) = rest.split_once('<') {
        text.push_str(before);
        match after
            .split_once('>')
            .and_then(|(hex, tail)| Some((hex_bytes(hex)?, tail)))
        {
            Some((bytes, tail)) => {
                text.push_str(&decode(bytes));
                rest = tail;
            }
            None => {
                text.push('<');
                rest = after;
            }
        }
    }
    text.push_str(rest);
    text
}

fn numbers<const N: usize>(value: &str) -> Option<[f64; N]> {
    let found: Vec<f64> = value
        .trim_matches('"')
        .split_whitespace()
        .map_while(|n| n.parse().ok())
        .collect();
    found.try_into().ok()
}

fn paper(size: [f64; 2], area: Option<[f64; 4]>) -> Paper {
    let [w, h] = size;
    let mut margins = Margins::default();
    if let Some([llx, lly, urx, ury]) = area {
        // Borderless sizes may overspray, which would read as a negative margin.
        let mm = |points: f64| points.max(0.0) * MM_PER_POINT;
        if urx > llx && ury > lly {
            margins = Margins {
                left: mm(llx),
                top: mm(h - ury),
                right: mm(w - urx),
                bottom: mm(lly),
            };
        }
    }
    Paper {
        size: SizeMm {
            w: w * MM_PER_POINT,
            h: h * MM_PER_POINT,
        },
        margins,
    }
}

/// `None` when the file names no usable page size.
pub fn parse(ppd: &str) -> Option<PrinterCaps> {
    let mut sizes: Vec<(&str, &str)> = Vec::new();
    let mut dimensions = HashMap::new();
    let mut areas = HashMap::new();
    let mut defaults = HashMap::new();
    let mut options: Vec<PrinterOption> = Vec::new();
    let mut open: Option<PrinterOption> = None;
    // Installable options describe the hardware; they are not per-job choices.
    let mut installable = false;

    let mut lines = ppd.lines();
    while let Some(line) = lines.next() {
        let Some(rest) = line.strip_prefix('*') else {
            continue;
        };
        if rest.starts_with('%') {
            continue;
        }
        let Some((head, value)) = rest.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if value.starts_with('"') && !value[1..].contains('"') {
            lines.by_ref().find(|l| l.contains('"'));
        }
        let (keyword, option) = head.split_once(char::is_whitespace).unwrap_or((head, ""));
        let (id, label) = id_and_label(option);
        match keyword {
            "OpenGroup" => installable = value.starts_with("InstallableOptions"),
            "CloseGroup" => installable = false,
            "OpenUI" | "JCLOpenUI" => {
                open = Some(PrinterOption {
                    keyword: id.trim_start_matches('*').into(),
                    label: readable(label.trim_start_matches('*')),
                    choices: Vec::new(),
                    default: String::new(),
                });
            }
            "CloseUI" | "JCLCloseUI" => {
                if let Some(o) = open.take()
                    && !installable
                    && !o.choices.is_empty()
                {
                    options.push(o);
                }
            }
            "PageSize" => sizes.push((id, label)),
            "PageRegion" => {}
            "PaperDimension" => {
                if let Some(size) = numbers::<2>(value) {
                    dimensions.insert(id, size);
                }
            }
            "ImageableArea" => {
                if let Some(area) = numbers::<4>(value) {
                    areas.insert(id, area);
                }
            }
            _ => {
                if let Some(name) = keyword.strip_prefix("Default") {
                    defaults.insert(name, value);
                } else if let Some(o) = open.as_mut()
                    && o.keyword == keyword
                    && !id.is_empty()
                {
                    o.choices.push(Choice {
                        id: id.into(),
                        label: readable(label),
                    });
                }
            }
        }
    }

    let papers: Vec<PaperChoice> = sizes
        .into_iter()
        .filter_map(|(id, label)| {
            let size = *dimensions.get(id).filter(|[w, h]| *w > 0.0 && *h > 0.0)?;
            Some(PaperChoice {
                id: id.into(),
                label: readable(label),
                paper: paper(size, areas.get(id).copied()),
            })
        })
        .collect();
    let default_paper = match defaults.get("PageSize") {
        Some(&id) if papers.iter().any(|p| p.id == id) => id.to_string(),
        _ => papers.first()?.id.clone(),
    };
    for option in &mut options {
        option.default = match defaults.get(option.keyword.as_str()) {
            Some(&id) if option.choices.iter().any(|c| c.id == id) => id.into(),
            _ => option.choices[0].id.clone(),
        };
    }
    Some(PrinterCaps {
        papers,
        default_paper,
        options,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo() -> PrinterCaps {
        parse(include_str!("../tests/fixtures/photo-printer.ppd")).unwrap()
    }

    fn margins(caps: &PrinterCaps, id: &str) -> [f64; 4] {
        let m = caps.paper(id).unwrap().paper.margins;
        [m.left, m.top, m.right, m.bottom]
    }

    #[test]
    fn papers_and_margins() {
        let caps = photo();
        let ids: Vec<&str> = caps.papers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["A4", "4x6.Borderless"]);
        assert_eq!(caps.default_paper, "A4");
        assert_eq!(
            caps.paper("4x6.Borderless").unwrap().label,
            "4 x 6 in (borderless)"
        );
        assert!(margins(&caps, "A4").iter().all(|m| (m - 3.0).abs() <= 0.05));
        assert_eq!(margins(&caps, "4x6.Borderless"), [0.0; 4]);
        let a4 = caps.paper("A4").unwrap().paper.size;
        assert!((a4.w - 209.9).abs() < 0.1 && (a4.h - 297.0).abs() < 0.1);
    }

    #[test]
    fn options_quality_and_paper_type() {
        let caps = photo();
        let kind = caps.paper_type().unwrap();
        let ids: Vec<&str> = kind.choices.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["Plain", "Glossy"]);
        assert_eq!(kind.default, "Plain");
        assert_eq!(kind.label, "Media Type");
        assert_eq!(kind.choices[1].label, "Glossy Photo");
        assert_eq!(caps.quality().unwrap().keyword, "cupsPrintQuality");
        let keywords: Vec<&str> = caps.options.iter().map(|o| o.keyword.as_str()).collect();
        assert_eq!(keywords, ["MediaType", "cupsPrintQuality", "ColorModel"]);
    }

    #[test]
    fn without_page_sizes_there_is_nothing() {
        assert!(parse("garbage").is_none());
    }

    #[test]
    fn oddities_of_other_drivers() {
        // Overspray borderless sizes, no imageable area, hardware-only options,
        // and a default that is not one of the choices.
        let caps = parse(concat!(
            "*DefaultPageSize: Nowhere\n",
            "*PageSize Over/Overspray: \"\"\n",
            "*PaperDimension Over/Overspray: \"288 432\"\n",
            "*ImageableArea Over/Overspray: \"-8 -9 296 441\"\n",
            "*PageSize Bare: \"\"\n",
            "*PaperDimension Bare: \"595 842\"\n",
            "*OpenGroup: InstallableOptions/Installed\n",
            "*OpenUI *Duplexer/Duplexer: Boolean\n",
            "*Duplexer True/Fitted: \"\"\n",
            "*CloseUI: *Duplexer\n",
            "*CloseGroup: InstallableOptions\n",
            "*JCLOpenUI *JCLEco/Economy: PickOne\n",
            "*DefaultJCLEco: Gone\n",
            "*JCLEco Off: \"\"\n",
            "*JCLCloseUI: *JCLEco\n",
        ))
        .unwrap();
        assert_eq!(margins(&caps, "Over"), [0.0; 4]);
        assert_eq!(margins(&caps, "Bare"), [0.0; 4]);
        assert_eq!(caps.paper("Bare").unwrap().label, "Bare");
        assert_eq!(caps.default_paper, "Over");
        assert_eq!(caps.options.len(), 1);
        assert_eq!(
            (
                caps.options[0].keyword.as_str(),
                caps.options[0].default.as_str()
            ),
            ("JCLEco", "Off")
        );
    }

    #[test]
    fn driverless_printer() {
        // As CUPS generates it for an IPP Everywhere printer (`ippeveprinter`).
        let caps = parse(include_str!("../tests/fixtures/driverless.ppd")).unwrap();
        assert_eq!(caps.default_paper, "Letter");
        assert_eq!(caps.papers.len(), 5);
        assert_eq!(caps.quality().unwrap().keyword, "cupsPrintQuality");
        assert_eq!(caps.paper_type().unwrap().keyword, "MediaType");
    }

    #[test]
    fn vendor_names_and_escaped_labels() {
        let option = |keyword: &str, label: &str, choice: &str| {
            format!(
                "*OpenUI *{keyword}/{label}: PickOne\n*{keyword} {choice}: \"\"\n*CloseUI: *{keyword}\n"
            )
        };
        let sizes = "*PageSize A4: \"\"\n*PaperDimension A4: \"595 842\"\n";
        // BRPrintQuality is a colour/mono switch, not a quality setting.
        let vendor = [
            sizes.to_string(),
            option("BRPrintQuality", "Colour", "Auto/Auto"),
            option(
                "BRMediaType",
                "Media<3A> Type",
                "Thin/   Light (<3c>70 g/m2)",
            ),
            option(
                "HPPrintQuality",
                "Print Quality",
                "Best/Best <E2 80 93> 1200 <x>",
            ),
        ]
        .concat();
        let caps = parse(&vendor).unwrap();
        let kind = caps.paper_type().unwrap();
        assert_eq!(kind.keyword, "BRMediaType");
        assert_eq!(kind.label, "Media: Type");
        assert_eq!(kind.choices[0].label, "Light (<70 g/m2)");
        let quality = caps.quality().unwrap();
        assert_eq!(quality.keyword, "HPPrintQuality");
        assert_eq!(quality.choices[0].label, "Best \u{2013} 1200 <x>");

        let both = [
            sizes.to_string(),
            option("Quality", "Q", "a/a"),
            option("Resolution", "R", "b/b"),
        ];
        assert_eq!(
            parse(&both.concat()).unwrap().quality().unwrap().keyword,
            "Resolution"
        );
    }

    #[test]
    fn latin_1_files_keep_their_accents() {
        assert_eq!(decode(b"Qualit\xe9".to_vec()), "Qualit\u{e9}");
        assert_eq!(decode("Qualit\u{e9}".as_bytes().to_vec()), "Qualit\u{e9}");
    }

    #[test]
    fn fallback_is_a4_then_letter() {
        let caps = PrinterCaps::fallback();
        assert_eq!(caps.default_paper, "A4");
        assert_eq!(caps.papers.len(), 2);
        assert_eq!(caps.paper("Letter").unwrap().paper.size.w, 215.9);
        assert_eq!(margins(&caps, "A4"), [5.0; 4]);
        assert!(caps.options.is_empty() && caps.quality().is_none() && caps.paper_type().is_none());
    }
}
