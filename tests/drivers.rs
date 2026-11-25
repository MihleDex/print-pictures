//! "Any printer": parses the PPDs of the installed CUPS drivers. Slow, and
//! depends on what is installed, so it is ignored by default:
//! `cargo test --release --test drivers -- --ignored --nocapture`
//! `DRIVER_STEP=1` tries every driver instead of every tenth (which takes about
//! 12 minutes for 20,000 installed drivers).

use print_pictures::ppd::{PrinterCaps, decode, parse};
use std::collections::BTreeMap;
use std::process::Command;

const DRIVERD: &str = "/usr/lib/cups/daemon/cups-driverd";

fn broken(caps: &PrinterCaps) -> Option<String> {
    if caps.paper(&caps.default_paper).is_none() {
        return Some("default paper is not listed".into());
    }
    caps.papers.iter().find_map(|p| {
        let area = p.paper.printable();
        (area.w <= 0.0 || area.h <= 0.0).then(|| format!("{} has no printable area", p.id))
    })
}

#[test]
#[ignore]
fn installed_drivers_parse() {
    let step: usize = std::env::var("DRIVER_STEP").map_or(10, |s| s.parse().unwrap());
    let list = Command::new("lpinfo").arg("-m").output().unwrap().stdout;
    let list = String::from_utf8_lossy(&list);
    let names: Vec<&str> = list
        .lines()
        .filter_map(|l| l.split_whitespace().next())
        .step_by(step)
        .collect();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    let results: Vec<(String, Result<PrinterCaps, String>)> = std::thread::scope(|scope| {
        let workers: Vec<_> = names
            .chunks(names.len().div_ceil(threads).max(1))
            .map(|chunk| {
                scope.spawn(move || {
                    chunk
                        .iter()
                        .filter_map(|name| {
                            let out = Command::new(DRIVERD).args(["cat", name]).output().ok()?;
                            let text = decode(out.stdout);
                            // Not every listed driver can produce a file (e.g. "raw").
                            if !text.starts_with("*PPD-Adobe") {
                                return None;
                            }
                            let caps = parse(&text).ok_or_else(|| "no page sizes".to_string());
                            Some((name.to_string(), caps))
                        })
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|w| w.join().unwrap())
            .collect()
    });

    let mut failures = Vec::new();
    let (mut no_quality, mut no_type) = (0, 0);
    let mut other_keywords: BTreeMap<String, usize> = BTreeMap::new();
    let mut escaped_labels = 0;
    for (name, caps) in &results {
        let caps = match caps {
            Ok(caps) => caps,
            Err(e) => {
                failures.push(format!("{name}: {e}"));
                continue;
            }
        };
        if let Some(why) = broken(caps) {
            failures.push(format!("{name}: {why}"));
        }
        let missing_quality = caps.quality().is_none();
        let missing_type = caps.paper_type().is_none();
        no_quality += usize::from(missing_quality);
        no_type += usize::from(missing_type);
        for o in &caps.options {
            let k = o.keyword.as_str();
            if (missing_quality && k.contains("Qual")) || (missing_type && k.contains("Medi")) {
                *other_keywords.entry(k.to_string()).or_default() += 1;
            }
        }
        escaped_labels += usize::from(
            caps.options
                .iter()
                .flat_map(|o| o.choices.iter().map(|c| &c.label))
                .chain(caps.papers.iter().map(|p| &p.label))
                .any(|label| label.contains('<') && label.contains('>')),
        );
    }

    println!(
        "{} driver files parsed (every {step}th of the list)",
        results.len()
    );
    println!("without a Quality option: {no_quality}; without a Paper type option: {no_type}");
    println!("with <..> in a label: {escaped_labels}");
    let mut common: Vec<_> = other_keywords.into_iter().collect();
    common.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    println!(
        "other quality/media keywords: {:?}",
        &common[..common.len().min(15)]
    );
    assert!(!results.is_empty(), "no driver files could be read");
    assert!(
        failures.is_empty(),
        "{} failed, first ones:\n{}",
        failures.len(),
        failures[..failures.len().min(20)].join("\n")
    );
}
