# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

The app, a `.deb` package and GitHub workflows are written; no release has been published yet. The checks that gate the first release are in `docs/release-checks.md`; what the app is for and why it works as it does is in `docs/design.md`. `origin` is a public GitHub repository, so nothing private goes into a commit.

Known limit: gdk-pixbuf loads many cut-off picture files without an error, so they print with part missing.

## The product

`print-pictures` is a Rust/GTK4 copy of the Windows "Print Pictures" window, for people who moved to Linux and print many photos: select pictures in the file manager → right-click → Print → pick a layout → Print.

- Avoid: any extra step compared with Windows, settings that must be redone for each job, prints at the wrong size.
- Out of scope: photo editing, a photo library, layouts or options Windows did not have; for now also Fedora, Arch, Flatpak and translations.
- Targets: Linux Mint 22, Zorin OS 18, the Ubuntu family 24.04, Debian 13, or newer; amd64.

## Setup

Building needs Rust 1.92 or newer and, once, with `sudo`:

```sh
sudo apt install libgtk-4-dev liblcms2-dev cups-client poppler-utils printer-driver-cups-pdf
```

`printer-driver-cups-pdf` adds a test printer, `PDF`. Two tests need the CUPS service running.

## Commands

```sh
cargo build
cargo test
cargo test a4_counts_match_windows          # one test, by name
cargo test --test render                    # one integration test file
cargo clippy --all-targets -- -D warnings
cargo fmt --check                           # CI runs this too
cargo test --lib -- --ignored dialogs welcome   # needs a display
cargo test --release --test drivers -- --ignored --nocapture   # parses installed drivers, ~12 min
cargo deb                                   # the package (cargo install cargo-deb)

cargo run -- FILES...                       # open the window; no files: welcome window
cargo run -- render --layout wallet -o out.pdf FILES...   # PDF only, no printer needed
cargo run -- printers                       # printers, default paper, option keywords
cargo run -- print --printer PDF --layout 10x15 FILES...
cargo run --release -- install              # also `uninstall`; then `thunar -q`
```

Layout ids: `full`, `wallet`, `contact`, `20x25`, `13x18`, `10x15`, `9x13`, `8x10in`, `5x7in`, `4x6in`, `3.5x5in`.

## Architecture

The crate is a library plus a thin `main.rs`. One job flows through it like this:

1. `job::check_files` probes the selected files and splits them into usable and rejected.
2. `cups` lists printers and fetches each PPD over `/run/cups/cups.sock`; `ppd` turns it into `PrinterCaps` (papers, margins, options), with a fallback if that fails.
3. `ui::model::Model` holds all dialog state, merging those with remembered `settings`. It has no GTK types, so dialog behaviour is tested here. The other `ui` modules are a thin widget shell over it.
4. `layout` (pure) turns layout, paper, picture count and copies into pages of slots.
5. `image_source` decodes with gdk-pixbuf only, into an upright sRGB `Raster`; `sharpen` filters it.
6. `render` draws a raster into a slot with cairo. The preview and the print PDF **share this path**, so do not add a second drawing routine.
7. `job::build_pdf` writes the PDF at 300 dpi, decoding one picture at a time; `cups::submit` sends it with `lp`.

`integration` is separate: user folders, the Thunar custom action (`uca.xml`) and the desktop entry used by source installs. `ui::welcome` offers the Thunar action. `data/` holds what the package installs for other file managers; a packaged program (under `/usr`) never copies itself.

## Rules that span modules

- Geometry is in millimetres, including cairo user units in `render`. PPD points convert at 25.4/72.
- Slot counts come from the paper with a nominal 5 mm margin and 4 mm gap. A printer's larger margins shrink the whole block slightly and never change the count.
- Physical sizes must be exact: jobs go out with `print-scaling=none`.
- Run `lp`/`lpstat` with `LC_ALL=C`, passing arguments as a list (never a shell), with `--` before the file.
- UI strings, layout sizes and counts must match `docs/design.md` and the tests.
- Never modify the user's pictures. Never rewrite Thunar's config without a backup.
- No network access beyond the local CUPS server. No libadwaita.
- It must work with any printer: never special-case one model. A network printer found but not set up gets its queue from `lpoptions -p NAME -l` before its PPD is fetched; that path has not been tried against real hardware.
- Decoding, PDF building and submission stay off the UI thread, and only the visible page is decoded.
- A printer on the `cups-pdf:` device gets no driver settings: the window asks where to save and writes the PDF itself.
- Installing the package must not pull in a file manager the person does not use.

## Cases to keep working

- File names with spaces, non-ASCII, quotes, `%` or a leading `-`.
- A corrupt, missing or non-picture file is skipped and named while the rest print; with none usable, nothing is sent.
- Hundreds of selected pictures: the window still opens promptly.
- Remembered printer gone: use the default. No printers: open with Print disabled and say why.
- Transparent PNGs print on white.
- A remembered metric size shows as the matching inch size, and the reverse.
