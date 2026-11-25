//! Printer list and capabilities from the local CUPS server.

use crate::ppd::{self, PrinterCaps};
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const SOCKET: &str = "/run/cups/cups.sock";

/// A message to show to the user as it is.
#[derive(Clone, Debug, thiserror::Error)]
#[error("{0}")]
pub struct PrintError(pub String);

#[derive(Clone, Debug, PartialEq)]
pub struct Printer {
    pub name: String,
    pub is_default: bool,
    pub stopped: bool,
    /// The "printer" writes a PDF file instead of printing.
    pub saves_to_file: bool,
}

/// Runs a CUPS tool in the C locale and returns its standard output.
fn run<A: AsRef<OsStr>>(program: &str, args: &[A]) -> Result<String, PrintError> {
    let out = Command::new(program)
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| PrintError(format!("cannot run {program}: {e}")))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(PrintError(
            String::from_utf8_lossy(&out.stderr).trim().into(),
        ))
    }
}

/// Reads the output of `lpstat -e`, `lpstat -d` and `lpstat -p`.
pub fn parse_printers(names: &str, default: &str, status: &str) -> Vec<Printer> {
    let default = default
        .lines()
        .find_map(|l| l.strip_prefix("system default destination: "))
        .map(str::trim);
    names
        .lines()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| Printer {
            saves_to_file: false,
            name: name.into(),
            is_default: default == Some(name),
            stopped: status.lines().any(|l| {
                l.strip_prefix("printer ")
                    .and_then(|rest| rest.strip_prefix(name))
                    .is_some_and(|rest| rest.starts_with(" disabled"))
            }),
        })
        .collect()
}

pub fn printers() -> Result<Vec<Printer>, PrintError> {
    let names = match run("lpstat", &["-e"]) {
        Ok(names) => names,
        Err(e) if e.0.contains("No destinations") => String::new(),
        Err(e) => return Err(e),
    };
    // Both fail harmlessly when there is no default or no queue yet.
    let default = run("lpstat", &["-d"]).unwrap_or_default();
    let status = run("lpstat", &["-p"]).unwrap_or_default();
    let devices = run("lpstat", &["-v"]).unwrap_or_default();
    let mut printers = parse_printers(&names, &default, &status);
    for printer in &mut printers {
        printer.saves_to_file = file_printer(&devices, &printer.name);
    }
    Ok(printers)
}

/// The queue behind a destination name, which may be "queue/instance".
fn queue(name: &str) -> &str {
    name.split_once('/').map_or(name, |(queue, _)| queue)
}

/// Whether `lpstat -v` shows the printer as the one that writes PDF files.
pub fn file_printer(devices: &str, name: &str) -> bool {
    devices.lines().any(|line| {
        line.strip_prefix("device for ")
            .and_then(|rest| rest.strip_prefix(queue(name)))
            .is_some_and(|rest| rest.starts_with(": cups-pdf:"))
    })
}

fn ppd_path(name: &str) -> String {
    let escaped: String = queue(name)
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    format!("/printers/{escaped}.ppd")
}

fn http_body(response: &[u8]) -> Option<&[u8]> {
    let end = response.windows(4).position(|w| w == b"\r\n\r\n")?;
    let status = response.split(|&b| b == b'\r').next()?;
    let ok = status.split(|&b| b == b' ').nth(1) == Some(b"200");
    ok.then(|| &response[end + 4..])
}

fn get_ppd(name: &str) -> Option<String> {
    let mut socket = UnixStream::connect(SOCKET).ok()?;
    socket
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok()?;
    let path = ppd_path(name);
    write!(socket, "GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n").ok()?;
    let mut response = Vec::new();
    socket.read_to_end(&mut response).ok()?;
    http_body(&response).map(|body| ppd::decode(body.to_vec()))
}

/// The queue's PPD, first having CUPS set up a discovered printer if need be.
fn ppd_for(name: &str) -> Option<String> {
    get_ppd(name).or_else(|| {
        // A printer found on the network has no queue yet, and listing its
        // options is what makes CUPS create one.
        let _ = run("lpoptions", &["-p", name, "-l"]);
        get_ppd(name)
    })
}

/// What the printer offers. Can take a while for a network printer.
pub fn capabilities(name: &str) -> PrinterCaps {
    ppd_for(name)
        .and_then(|text| ppd::parse(&text))
        .unwrap_or_else(PrinterCaps::fallback)
}

#[derive(Clone, Debug)]
pub struct SubmitRequest {
    pub printer: String,
    pub title: String,
    pub paper_id: String,
    pub options: Vec<(String, String)>,
}

const TITLE_CHARS: usize = 64;

/// What the print queue shows: the first file name and how many more there are.
pub fn job_title(files: &[PathBuf]) -> String {
    let mut title = String::from("Print Pictures");
    if let Some(name) = files.first().and_then(|f| f.file_name()) {
        title = format!("{title} - {}", name.to_string_lossy());
    }
    let more = match files.len() {
        0 | 1 => String::new(),
        n => format!(" (+{})", n - 1),
    };
    let head: String = title.chars().take(TITLE_CHARS - more.len()).collect();
    head + &more
}

pub fn lp_args(req: &SubmitRequest, pdf: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec![
        "-d".into(),
        req.printer.clone().into(),
        "-t".into(),
        req.title.clone().into(),
        "-o".into(),
        format!("PageSize={}", req.paper_id).into(),
        // The PDF is already the size of the paper; any scaling would be wrong.
        "-o".into(),
        "print-scaling=none".into(),
    ];
    for (key, value) in &req.options {
        args.push("-o".into());
        args.push(format!("{key}={value}").into());
    }
    args.push("--".into());
    args.push(pdf.into());
    args
}

pub fn parse_job_id(stdout: &str) -> Option<String> {
    let rest = stdout.trim().strip_prefix("request id is ")?;
    rest.split_whitespace().next().map(str::to_string)
}

/// Sends the PDF and returns the job id.
pub fn submit(req: &SubmitRequest, pdf: &Path) -> Result<String, PrintError> {
    // A network printer's temporary queue may have expired while the window was open.
    let _ = ppd_for(&req.printer);
    let reply = run("lp", &lp_args(req, pdf))?;
    // The job has gone once lp succeeds, so an unexpected reply is not a failure.
    Ok(parse_job_id(&reply).unwrap_or_else(|| reply.trim().to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lp_arguments_are_exact() {
        let request = SubmitRequest {
            printer: "PDF".into(),
            title: "A title".into(),
            paper_id: "A4".into(),
            options: vec![("MediaType".into(), "Glossy".into())],
        };
        let expected = [
            "-d",
            "PDF",
            "-t",
            "A title",
            "-o",
            "PageSize=A4",
            "-o",
            "print-scaling=none",
            "-o",
            "MediaType=Glossy",
            "--",
            "/tmp/x.pdf",
        ];
        assert_eq!(
            lp_args(&request, Path::new("/tmp/x.pdf")),
            expected.map(OsString::from)
        );
    }

    #[test]
    fn job_titles() {
        let files = |names: &[&str]| names.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(
            job_title(&files(&["/x/-a \"b\" 100%.jpg"])),
            "Print Pictures - -a \"b\" 100%.jpg"
        );
        let long = "ü".repeat(200);
        let cut = job_title(&files(&[&long]));
        assert_eq!(cut.chars().count(), 64);
        assert!(cut.starts_with("Print Pictures - üü"));
        let three = job_title(&files(&[&long, "b", "c"]));
        assert_eq!(three.chars().count(), 64);
        assert!(three.ends_with("ü (+2)"), "{three}");
        assert!(job_title(&files(&["a.jpg", "b", "c"])).ends_with("a.jpg (+2)"));
    }

    #[test]
    fn job_id() {
        assert_eq!(
            parse_job_id("request id is PDF-3 (1 file(s))\n"),
            Some("PDF-3".into())
        );
        assert_eq!(parse_job_id("something else"), None);
    }

    #[test]
    fn only_the_pdf_file_backend_saves_to_file() {
        let devices = "device for PDF: cups-pdf:/\ndevice for Office: ipp://host/ipp\ndevice for PDF2: usb://x\n";
        assert!(file_printer(devices, "PDF"));
        assert!(file_printer(devices, "PDF/draft"));
        for other in ["Office", "PDF2", "PD", "Missing"] {
            assert!(!file_printer(devices, other), "{other}");
        }
    }

    #[test]
    fn printer_list() {
        let printers = parse_printers(
            "PDF\nOffice\nOff\n",
            "system default destination: PDF\n",
            "printer PDF is idle.  enabled since x\nprinter Office disabled since x -\n\treason\n",
        );
        let seen: Vec<(&str, bool, bool)> = printers
            .iter()
            .map(|p| (p.name.as_str(), p.is_default, p.stopped))
            .collect();
        assert_eq!(
            seen,
            [
                ("PDF", true, false),
                ("Office", false, true),
                ("Off", false, false)
            ]
        );
        assert!(parse_printers("", "no system default destination\n", "").is_empty());
    }

    #[test]
    fn ppd_address_drops_the_instance_and_escapes_the_name() {
        assert_eq!(ppd_path("Office"), "/printers/Office.ppd");
        assert_eq!(ppd_path("Büro_1?/draft"), "/printers/B%C3%BCro_1%3F.ppd");
    }

    #[test]
    fn only_a_200_response_has_a_body() {
        assert_eq!(
            http_body(b"HTTP/1.0 200 OK\r\nA: b\r\n\r\n*PPD"),
            Some(&b"*PPD"[..])
        );
        assert_eq!(http_body(b"HTTP/1.0 404 Not Found\r\n\r\nno"), None);
        assert_eq!(http_body(b""), None);
    }
}
