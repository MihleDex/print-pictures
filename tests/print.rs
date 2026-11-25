mod common;

use common::write_png;
use std::process::Command;

fn print(args: &[&str], file: &std::path::Path) -> (bool, String) {
    let run = Command::new(env!("CARGO_BIN_EXE_print-pictures"))
        .args(["print", "--layout", "full"])
        .args(args)
        .arg(file)
        .output()
        .unwrap();
    (
        run.status.success(),
        String::from_utf8_lossy(&run.stderr).into_owned(),
    )
}

// Nothing is sent by either case, so no printer is needed.
#[test]
fn failures_are_reported() {
    let dir = tempfile::tempdir().unwrap();
    let picture = dir.path().join("a.png");
    write_png(&picture, 8, 8, |_, _| [0, 0, 0, 255]);

    let (ok, stderr) = print(&["--printer", "NoSuchPrinter_pp_test"], &picture);
    assert!(!ok);
    assert!(stderr.contains("does not exist"), "{stderr}");

    let (ok, stderr) = print(
        &["--printer", "NoSuchPrinter_pp_test", "--paper", "Z9"],
        &picture,
    );
    assert!(!ok);
    assert!(stderr.contains("Z9") && stderr.contains("A4"), "{stderr}");
}
