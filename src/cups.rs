//! Printer list and capabilities from the local CUPS server.

use crate::ppd::{self, PrinterCaps};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
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
}

/// Runs a CUPS tool in the C locale and returns its standard output.
fn run(program: &str, args: &[&str]) -> Result<String, PrintError> {
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
    Ok(parse_printers(&names, &default, &status))
}

/// The queue behind a destination name, which may be "queue/instance".
fn queue(name: &str) -> &str {
    name.split_once('/').map_or(name, |(queue, _)| queue)
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

#[cfg(test)]
mod tests {
    use super::*;

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
