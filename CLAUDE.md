# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Status

As of 2026-11-25, Tasks 1–3 of the plan are built on branch `print-pictures`: layout, picture loading, drawing, PDF building and the `render` command. The window, printing, `printers`, `print` and `install` are not built yet, so anything below about them describes what the plan will create. Update this file as the code lands and where it diverges from the plan.

Known limit: gdk-pixbuf loads many cut-off picture files without an error, so they print with part missing.

- `docs/superpowers/specs/2026-11-25-print-pictures-design.md`: what to build and why. Its "Approaches" and "Not yet confirmed" sections are assumptions the user has not agreed to.
- `docs/superpowers/plans/2026-11-25-print-pictures.md`: nine tasks with interfaces, tests and exact constraints. Execute it with `superpowers:subagent-driven-development` or `superpowers:executing-plans`, ticking its checkboxes.

## The product

`print-pictures` is a Rust/GTK4 copy of the Windows "Print Pictures" window for Linux Mint 22.3 (XFCE/X11, Thunar 4.18, CUPS 2.4). The user wants "the closest thing possible to what i had": select pictures in Thunar → right-click → Print → pick a layout → Print.

- Avoid: any extra step compared with Windows, settings that must be redone for each job, prints at the wrong size.
- Out of scope: photo editing, a photo library, layouts or options Windows did not have, packaging for other distributions.

## Setup

The user has to run this once, since it needs `sudo`:

```sh
sudo apt install libgtk-4-dev liblcms2-dev printer-driver-cups-pdf
```

Check it with `pkg-config --modversion gtk4` (4.14.x) and `lpstat -v` (lists `PDF`, a test printer that writes to `~/PDF`). PDF tests also need `pdfinfo` and `pdftoppm`.

## Commands

```sh
cargo build
cargo test
cargo test a4_counts_match_windows          # one test, by name
cargo test --test render                    # one integration test file
cargo clippy --all-targets -- -D warnings

cargo run -- FILES...                       # open the window
cargo run -- render --layout wallet -o out.pdf FILES...   # PDF only, no printer needed
cargo run -- printers                       # printers, default paper, option keywords
cargo run -- print --printer PDF --layout 10x15 FILES...
cargo run --release -- install              # also `uninstall`; then `thunar -q`
```

Layout ids: `full`, `20x25`, `13x18`, `10x15`, `9x13`, `wallet`, `contact`.

Every plan task ends with `cargo test` passing and clippy clean, then a commit.

## Architecture

The crate is a library plus a thin `main.rs`. One job flows through it like this:

1. `job::check_files` probes the selected files and splits them into usable and rejected.
2. `cups` lists printers and fetches each PPD over `/run/cups/cups.sock`; `ppd` turns it into `PrinterCaps` (papers, margins, options), with a fallback if that fails.
3. `ui::model::Model` holds all dialog state, merging those with remembered `settings`. It has no GTK types, so dialog behaviour is tested here. The other `ui` modules are a thin widget shell over it.
4. `layout` (pure) turns layout, paper, picture count and copies into pages of slots.
5. `image_source` decodes with gdk-pixbuf only, into an upright sRGB `Raster`; `sharpen` filters it.
6. `render` draws a raster into a slot with cairo. The preview and the print PDF **share this path**, so do not add a second drawing routine.
7. `job::build_pdf` writes the PDF at 300 dpi, decoding one picture at a time; `cups::submit` sends it with `lp`.

`integration` is separate: it adds the Thunar custom action to `~/.config/Thunar/uca.xml` and writes the desktop entry.

## Rules that span modules

- Geometry is in millimetres, including cairo user units in `render`. PPD points convert at 25.4/72.
- Slot counts come from the paper with a nominal 5 mm margin and 4 mm gap. A printer's larger margins shrink the whole block slightly and never change the count.
- Physical sizes must be exact: jobs go out with `print-scaling=none`.
- Run `lp`/`lpstat` with `LC_ALL=C`, passing arguments as a list (never a shell), with `--` before the file.
- UI strings, layout sizes and A4 counts must match the plan's Global Constraints and the spec's layout table exactly.
- Never modify the user's pictures. Never rewrite Thunar's config without a backup.
- No network access beyond the local CUPS server. No libadwaita.
- Decoding, PDF building and submission stay off the UI thread, and only the visible page is decoded.

## Cases to keep working

- File names with spaces, non-ASCII, quotes, `%` or a leading `-`.
- A corrupt, missing or non-picture file is skipped and named while the rest print; with none usable, nothing is sent.
- Hundreds of selected pictures: the window still opens promptly.
- Remembered printer gone: use the default. No printers: open with Print disabled and say why.
- Transparent PNGs print on white.
