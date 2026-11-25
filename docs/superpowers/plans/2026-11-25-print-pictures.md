# Print Pictures Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A Rust copy of the Windows "Print Pictures" window for Linux Mint XFCE, opened from Thunar's right-click menu.

**Architecture:** A pure layout engine works in millimetres; one cairo drawing routine serves both the on-screen preview and the print PDF; the PDF is sent to CUPS with `lp`. Printer settings come from the CUPS PPD. The GTK widgets are a thin shell over a testable dialog model.

**Tech Stack:** Rust (edition 2024), gtk4 0.11 (`v4_14`), cairo-rs 0.22 (`pdf`), pangocairo 0.22, gdk-pixbuf 0.22, lcms2 6.3, serde 1 + toml 1, clap 4.6, thiserror 2, tempfile 3.

**Spec:** `docs/superpowers/specs/2026-11-25-print-pictures-design.md`

## Global Constraints

- Target: Linux Mint 22.3, XFCE/X11, Thunar 4.18, system GTK 4.14, CUPS 2.4. No libadwaita.
- Crate and binary name `print-pictures`; GTK application id `local.printpictures.PrintPictures`.
- No network access other than the local CUPS server.
- Exact UI strings: "Print Pictures", "How do you want to print your pictures?", "Printer:", "Paper size:", "Quality:", "Paper type:", "Copies of each picture:", "Fit picture to frame", "Options...", "Print", "Cancel", "Page {n} of {total}", "Print Settings", "Sharpen for printing", "Printer Properties...".
- Layout sizes and A4 counts exactly as in the spec table.
- Never modify the user's pictures. Never rewrite Thunar's config without a backup.
- Run `lp`/`lpstat` with `LC_ALL=C`; pass arguments as a list, never through a shell.
- Every task ends with `cargo test` passing and `cargo clippy --all-targets -- -D warnings` clean.
- Checks that need clicking or typing in a window are done by your human partner: there is no input-automation tool here (`xdotool` is not installed). Prepare the files and a numbered list, and tick the box only after they report the result.

## Review Focus

1. File names with spaces, non-ASCII, quotes, `%`, or a leading `-` → printed like any other (Tasks 3, 5, 9).
2. One corrupt, missing or non-picture file among good ones → skipped and named, the rest print; none usable → message, nothing sent (Task 3). This includes a picture that passes the header check and then fails to load (Tasks 3, 7). Known limit: gdk-pixbuf loads many cut-off files without an error, so those print with part missing.
3. Hundreds of pictures selected → window opens promptly; only the visible page is decoded (Tasks 6, 7).
4. Remembered printer no longer exists, or no printers at all → default printer is used; with none, the window opens with Print disabled and says why (Task 6).
5. Transparent PNG → white background, never black (Task 2).

## File Structure

```
src/lib.rs           module list
src/layout.rs        paper, layouts, slot geometry, pagination (pure)
src/image_source.rs  decode, orient, scale, colour-convert to sRGB
src/sharpen.rs       unsharp filter (pure)
src/render.rs        fit rules and cairo drawing, shared by preview and PDF
src/job.rs           file checking and PDF building
src/ppd.rs           PPD text → printer capabilities (pure)
src/cups.rs          printer list, PPD fetch, job submission
src/settings.rs      remembered choices
src/integration.rs   Thunar action and desktop entry
src/main.rs          command line
src/ui/mod.rs        application start
src/ui/model.rs      dialog state (no GTK types)
src/ui/window.rs     main window
src/ui/preview.rs    preview and paging
src/ui/layout_list.rs  layout sidebar
src/ui/options.rs    Print Settings and Printer Properties
tests/common/mod.rs  picture fixtures and PDF rasteriser
```

---

### Task 1: Project setup and layout engine

**Files:** Create `Cargo.toml`, `.gitignore`, `src/lib.rs`, `src/main.rs`, `src/layout.rs` (tests inline).

**Interfaces — Produces** (`print_pictures::layout`):
```rust
pub const NOMINAL_MARGIN_MM: f64 = 5.0;
pub const GAP_MM: f64 = 4.0;
pub struct SizeMm { pub w: f64, pub h: f64 }
pub struct RectMm { pub x: f64, pub y: f64, pub w: f64, pub h: f64 }
pub struct Margins { pub left: f64, pub top: f64, pub right: f64, pub bottom: f64 } // Default = zero
pub struct Paper { pub size: SizeMm, pub margins: Margins }
impl Paper { pub fn a4(margins: Margins) -> Paper; pub fn printable(&self) -> RectMm; }
pub enum LayoutId { FullPage, P20x25, P13x18, P10x15, P9x13, Wallet, ContactSheet } // Default = FullPage; serde
impl LayoutId { pub const ALL: [LayoutId; 7]; pub fn id(self) -> &'static str; pub fn from_id(s: &str) -> Option<LayoutId>; }
// ids: full, 20x25, 13x18, 10x15, 9x13, wallet, contact
pub fn slots(layout: LayoutId, paper: &Paper) -> Vec<RectMm>;      // reading order; empty = unavailable
pub fn label(layout: LayoutId, paper: &Paper) -> String;
pub struct Placement { pub image: usize, pub slot: RectMm }
pub type PagePlan = Vec<Placement>;
pub fn paginate(layout: LayoutId, paper: &Paper, image_count: usize, copies: u32) -> Vec<PagePlan>;
```
All structs and enums derive `Clone, Debug, PartialEq`; geometry types and `LayoutId` also `Copy`.

- [x] **Step 1:** Ask your human partner to run `sudo apt install libgtk-4-dev liblcms2-dev printer-driver-cups-pdf`. Verify: `pkg-config --modversion gtk4` prints `4.14.x` and `lpstat -v` lists `PDF`. Also verify `git var GIT_AUTHOR_IDENT` exits 0; if not, ask them to set `user.name` and `user.email` (never choose these yourself).
- [x] **Step 2:** `git init` (skip if already a repository) and commit `CLAUDE.md`, `docs/` and `plans/` as `docs: print pictures spec and plan`; then create the package with every dependency from Tech Stack, `.gitignore` containing `/target`, an empty `main`. Run `cargo build`. Expected: success.
- [x] **Step 3: Write the failing tests** in `src/layout.rs`:

```rust
fn a4() -> Paper { Paper::a4(Margins::default()) }
fn counts(p: &Paper) -> Vec<usize> { LayoutId::ALL.iter().map(|&l| slots(l, p).len()).collect() }

#[test] fn a4_counts_match_windows() { assert_eq!(counts(&a4()), [1, 1, 2, 2, 4, 9, 35]); }

#[test] fn printer_margins_never_change_counts_and_slots_stay_printable() {
    let p = Paper::a4(Margins { left: 3.0, top: 3.0, right: 3.0, bottom: 12.7 });
    assert_eq!(counts(&p), [1, 1, 2, 2, 4, 9, 35]);
    // every slot of every layout lies within p.printable() (tolerance 1e-6)
}
#[test] fn captions() {
    assert_eq!(label(LayoutId::FullPage, &a4()), "Full page photo");
    assert_eq!(label(LayoutId::P13x18, &a4()), "13 x 18 cm (2)");
    assert_eq!(label(LayoutId::Wallet, &a4()), "Wallet (9)");
    assert_eq!(label(LayoutId::ContactSheet, &a4()), "Contact sheet (35)");
}
#[test] fn exact_physical_sizes() {
    let s = slots(LayoutId::P10x15, &a4()); assert_eq!((s[0].w, s[0].h), (150.0, 100.0)); // turned
    let s = slots(LayoutId::P9x13, &a4());  assert_eq!((s[0].w, s[0].h), (90.0, 130.0));
}
#[test] fn a3_fits_four_13x18() { /* 297x420 → 4 */ }
#[test] fn matching_photo_paper() { /* paper 101.6x152.4: P10x15 → 1 slot inside printable; P13x18 → 0 */ }
#[test] fn grids_are_centred_without_overlap() { /* for all layouts on a4: left space == right space, top == bottom, no two slots intersect */ }
#[test] fn copies_are_consecutive() {
    let pages = paginate(LayoutId::P13x18, &a4(), 3, 2);
    let ids: Vec<Vec<usize>> = pages.iter().map(|p| p.iter().map(|x| x.image).collect()).collect();
    assert_eq!(ids, [[0, 0], [1, 1], [2, 2]]);
}
#[test] fn nothing_to_paginate() { assert!(paginate(LayoutId::Wallet, &a4(), 0, 1).is_empty()); }
```
- [x] **Step 4:** `cargo test layout` → FAIL (unresolved items).
- [x] **Step 5: Implement.** For sized layouts, on the *nominal* area (paper less 5 mm each side) try the catalogue size and its quarter turn; `cols = floor((avail_w + GAP) / (w + GAP))`, rows likewise; keep the larger `cols*rows`, ties to unturned. If zero but the size fits the paper itself (either way, 2 mm tolerance), use 1×1. Then centre the block in `printable()`, scaling uniformly down if it does not fit. Full page = `printable()`. Contact sheet divides `printable()` ∩ nominal area into 5×7 cells with the same gap.
- [x] **Step 6:** `cargo test layout` → PASS.
- [x] **Step 7:** Commit `feat: layout engine`.

---

### Task 2: Picture loading

**Files:** Create `src/image_source.rs`, `tests/common/mod.rs`, `tests/image_source.rs`.

**Interfaces — Produces:**
```rust
pub struct Raster { pub width: u32, pub height: u32, pub rgb: Vec<u8> } // sRGB, 3 bytes/px, no padding, upright
impl Raster { pub fn pixel(&self, x: u32, y: u32) -> [u8; 3]; }
pub enum LoadError { Unreadable(String), Unsupported(String) }          // thiserror
pub fn probe(path: &Path) -> Result<(u32, u32), LoadError>;            // stored size, header only
pub fn load(path: &Path, cover_long: u32, cover_short: u32) -> Result<Raster, LoadError>;
// test helpers
pub fn write_png(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 4]);
pub fn write_jpeg(path: &Path, w: u32, h: u32, px: impl Fn(u32, u32) -> [u8; 3]); // quality 95
pub fn add_exif_orientation(jpeg: &Path, orientation: u8);
```
`add_exif_orientation` inserts these bytes right after the first two bytes of the file (`XX` = orientation):
`FF E1 00 22 45 78 69 66 00 00 4D 4D 00 2A 00 00 00 08 00 01 01 12 00 03 00 00 00 01 00 XX 00 00 00 00 00 00`

- [x] **Step 1: Write the failing tests** (`tests/image_source.rs`):
  - `exif_6_is_turned_upright`: 40×20 JPEG, left half red, right blue, orientation 6 → loaded 20×40; pixel (10,5) reddish, (10,35) bluish.
  - `reduced_load_keeps_orientation`: 4000×2000, orientation 6, `load(p, 400, 200)` → 200×400.
  - `never_enlarges`: 40×20 with `load(p, 4000, 2000)` → 40×20.
  - `transparency_becomes_white`: fully transparent PNG → `[255,255,255]`; black at alpha 128 → each channel within 126..=129.
  - `embedded_profile_is_converted`: build with lcms2 an RGB profile whose red and blue primaries are swapped relative to sRGB; save a pure-red JPEG carrying it (pixbuf `savev` options `quality` = `95` and `icc-profile` = base64 of the profile) → loaded pixel has b > 200 and r < 60. It must be JPEG: gdk-pixbuf 2.42.10 refuses to save a PNG with an RGB profile.
  - `bad_files`: a text file named `x.jpg` → `Unsupported`; a missing path → `Unreadable`.
- [x] **Step 2:** `cargo test --test image_source` → FAIL.
- [x] **Step 3: Implement.** gdk-pixbuf only. Scale `s = max(cover_long / long, cover_short / short).min(1.0)` from `probe`; load with `from_file_at_scale`; read the `icc-profile` option *before* `apply_embedded_orientation`; convert with lcms2; composite alpha onto white; strip row padding.
- [x] **Step 4:** `cargo test --test image_source` → PASS.
- [x] **Step 5:** Commit `feat: picture loading`.

---

### Task 3: Drawing, PDF building and the `render` command

**Files:** Create `src/sharpen.rs`, `src/render.rs`, `src/job.rs`, `tests/render.rs`; modify `src/main.rs`, `src/lib.rs`, `tests/common/mod.rs`.

**Interfaces — Consumes:** `layout::*`, `image_source::{Raster, LoadError, load, probe}`.
**Produces:**
```rust
// sharpen.rs
pub fn sharpen(raster: &mut Raster);
// render.rs
pub struct Fit { pub turn: bool, pub image: RectMm, pub clip: RectMm }
pub fn fit(img_w: u32, img_h: u32, slot: RectMm, fill: bool) -> Fit;
pub fn split_caption(slot: RectMm) -> (RectMm, RectMm);   // picture area, 4 mm strip below
pub fn draw_picture(cr: &cairo::Context, raster: &Raster, fit: &Fit) -> Result<(), cairo::Error>; // cr units = mm
pub fn draw_caption(cr: &cairo::Context, text: &str, area: RectMm) -> Result<(), cairo::Error>;   // centred, ellipsized, pangocairo
// job.rs
pub const PRINT_DPI: u32 = 300;
pub struct JobSpec { pub files: Vec<PathBuf>, pub layout: LayoutId, pub paper: Paper, pub copies: u32, pub fill: bool, pub sharpen: bool }
pub struct Checked { pub usable: Vec<PathBuf>, pub rejected: Vec<(PathBuf, String)> }
pub fn check_files(files: &[PathBuf]) -> Checked;
pub struct Built { pub pages: usize, pub skipped: Vec<(PathBuf, String)> } // skipped = passed the check, failed to load
pub fn build_pdf(spec: &JobSpec, out: &Path) -> Result<Built, JobError>;
pub enum JobError { NothingToPrint(Vec<(PathBuf, String)>), LayoutUnavailable, Pdf(String) } // the Vec lists skipped pictures
```
Command: `print-pictures render --layout <id> [--paper A4|A3|Letter|A6] [--copies N] [--no-fit] [--no-sharpen] -o OUT.pdf FILES...` (zero margins; rejected and skipped files listed on stderr).

- [x] **Step 1: Write the failing unit tests.**
  - `fit(300, 200, slot{x:10,y:10,w:90,h:130}, true)` → `turn`, image `{10, 7.5, 90, 135}`, clip = slot.
  - Same with `false` → image w ≈ 86.667, h = 130, x ≈ 11.667, clip = image.
  - Square picture is never turned. A 4000×400 picture with `false` lies wholly inside the slot.
  - `sharpen`: uniform grey unchanged; across a 100|200 step the dark side goes below 100 and the light side above 200.
- [x] **Step 2: Write the failing PDF tests** (`tests/render.rs`). Add helper `raster_pdf(pdf: &Path, page: u32, dpi: u32) -> Raster` running `pdftoppm` and parsing its PPM; use 127 dpi = 5 px/mm.
  - `sizes_and_pages`: 3 solid pictures, 13x18, A4 → returns `pages == 2`; `pdfinfo` shows `Pages: 2` and A4 (595.x × 841.x pts); each slot centre has its picture's colour; pixel at 2 mm,2 mm is white.
  - `quarter_turn_direction`: landscape left-red/right-blue picture into 9x13 → top of the first slot red, bottom blue (turn is clockwise).
  - `awkward_names`: a file named `-Ünï "quoted" 100%.png` renders.
  - `bad_files_are_skipped`: `check_files([good, text-as-jpg, missing])` → 1 usable, 2 rejected with those paths; `build_pdf` with no files → `NothingToPrint` with an empty list and no output file.
  - `damaged_body_is_skipped`: write a 200×200 PNG with varied pixels; keep it up to and including the 16 bytes that follow `IDAT`, and set every later byte to zero. Assert first that `check_files` lists it as usable and `load` returns an error. Then `build_pdf` of `[good, damaged, good]` at 13x18 → `pages == 1`, `skipped` names only the damaged file, output exists. `build_pdf` of `[damaged]` → `NothingToPrint` listing it, and no output file.
- [x] **Step 3:** `cargo test` → FAIL.
- [x] **Step 4: Implement.** Fit scale is `max` (fill) or `min` (fit) of the slot/picture ratios after any turn, centred. Sharpen: `v + 0.5 * (v - mean3x3)`, clamped. Draw via a cairo `Rgb24` surface (BGRX in memory). `build_pdf` decodes one picture at a time at `PRINT_DPI` for its slot, writes to a temporary file beside `out` and renames on success. Contact sheet slots use `split_caption` with the file name. `check_files` uses `probe`. When a picture fails to load, `build_pdf` removes that file from its list, records it in `skipped` and builds again from the start, so no slot is left blank (add `// shortcut: one rebuild per damaged picture; fine while they are rare`). If none remain, return `NothingToPrint(skipped)`.
- [x] **Step 5:** `cargo test` → PASS.
- [x] **Step 6: Early look (Human check).** Ask your human partner for a few of their own pictures, then render every layout:
  `mkdir -p target/layout-check && for l in full 20x25 13x18 10x15 9x13 wallet contact; do cargo run -- render --layout $l -o target/layout-check/$l.pdf <pictures>; done`
  Also render one layout with `--no-sharpen`. Ask them to compare with what they remember (holding a printed page against a frame if they can): picture sizes, metric captions such as "13 x 18 cm (2)", file names on the contact sheet, sharpening on by default. Record each answer under "Not yet confirmed" in the spec. If a size or caption changes, update the spec's layout table and Task 1's tests before going on.
- [x] **Step 7: Measure a big job.** Run:
  ```sh
  mkdir -p target/timing
  i=0; while [ $i -lt 200 ]; do for f in /usr/share/backgrounds/*/*.jpg; do [ $i -lt 200 ] || break; i=$((i+1)); ln -sf "$f" target/timing/$i.jpg; done; done
  cargo build --release
  time target/release/print-pictures render --layout full -o target/timing/big.pdf target/timing/*.jpg
  ls -lh target/timing/big.pdf
  ```
  Tell your human partner both numbers (the estimate beforehand was about 2.4 GB). Do not change how pictures are embedded without their decision. Then `rm -r target/timing`.
- [x] **Step 8:** Commit `feat: render pictures to PDF`.

---

### Task 4: Printer list and capabilities

**Files:** Create `src/ppd.rs`, `src/cups.rs`, `tests/fixtures/photo-printer.ppd`; modify `src/main.rs`.

**Interfaces — Produces:**
```rust
pub struct Choice { pub id: String, pub label: String }
pub struct PrinterOption { pub keyword: String, pub label: String, pub choices: Vec<Choice>, pub default: String }
pub struct PaperChoice { pub id: String, pub label: String, pub paper: Paper }
pub struct PrinterCaps { pub papers: Vec<PaperChoice>, pub default_paper: String, pub options: Vec<PrinterOption> }
impl PrinterCaps {
    pub fn paper(&self, id: &str) -> Option<&PaperChoice>;
    pub fn quality(&self) -> Option<&PrinterOption>;    // first of Resolution, cupsPrintQuality, OutputMode
    pub fn paper_type(&self) -> Option<&PrinterOption>; // MediaType
    pub fn fallback() -> PrinterCaps;                   // A4 (default) and Letter, 5 mm margins, no options
}
pub fn parse(ppd: &str) -> Option<PrinterCaps>;          // None without page sizes
// cups.rs   (all structs in this task derive Clone, Debug)
pub struct PrintError(pub String);                  // thiserror; message shown to the user
pub struct Printer { pub name: String, pub is_default: bool, pub stopped: bool }
pub fn parse_printers(names: &str, default: &str, status: &str) -> Vec<Printer>; // lpstat -e, -d, -p
pub fn printers() -> Result<Vec<Printer>, PrintError>;
pub fn capabilities(name: &str) -> PrinterCaps;          // fallback() on any failure
```

- [x] **Step 1:** Write the fixture: PageSize/ImageableArea/PaperDimension for `A4` (`"8.5 8.5 586.5 833.5"`, dimension `"595 842"`) and `4x6.Borderless` (`"0 0 288 432"`), `MediaType` (Plain default, Glossy), `cupsPrintQuality` (Normal default, High), `ColorModel`, and one multi-line quoted value.
- [x] **Step 2: Write the failing tests.**
  - A4 margins are each 3.0 mm ±0.05; borderless margins are 0.
  - `paper_type()` choices `Plain`,`Glossy`, default `Plain`; `quality().keyword == "cupsPrintQuality"`; no option has keyword `PageSize` or `PageRegion`.
  - `parse("garbage")` is `None`.
  - `parse_printers("PDF\nOffice\n", "system default destination: PDF\n", "printer PDF is idle.  enabled since x\nprinter Office disabled since x -\n\treason\n")` → PDF default and not stopped, Office stopped. Empty names → empty list.
- [x] **Step 3:** `cargo test` → FAIL.
- [x] **Step 4: Implement.** Fetch the PPD with an HTTP/1.0 `GET /printers/<percent-encoded name>.ppd` over the Unix socket `/run/cups/cups.sock`, requiring status 200. PPD points convert at 25.4/72; margins are `llx`, `h-ury`, `w-urx`, `lly`.
- [x] **Step 5:** `cargo test` → PASS.
- [x] **Step 6:** Add `print-pictures printers` (each printer, its default paper and option keywords). Run it. Expected: `PDF`, default paper `A4`, options including `Resolution`. If it shows the fallback instead, the PPD fetch is wrong: fix before continuing.
- [x] **Step 7:** Commit `feat: printer capabilities`.

---

### Task 5: Sending jobs and the `print` command

**Files:** Modify `src/cups.rs`, `src/main.rs`.

**Interfaces — Produces:**
```rust
pub struct SubmitRequest { pub printer: String, pub title: String, pub paper_id: String, pub options: Vec<(String, String)> }
pub fn job_title(files: &[PathBuf]) -> String;   // "Print Pictures - <first name>" plus " (+N)", max 64 chars
pub fn lp_args(req: &SubmitRequest, pdf: &Path) -> Vec<OsString>;
pub fn parse_job_id(stdout: &str) -> Option<String>;
pub fn submit(req: &SubmitRequest, pdf: &Path) -> Result<String, PrintError>; // job id; error carries lp's stderr
```
Command: `print-pictures print --printer NAME --layout <id> [--paper ID] [--copies N] [--no-fit] [--no-sharpen] [-o KEY=VALUE]... FILES...` (rejected and skipped files listed on stderr).

- [ ] **Step 1: Write the failing tests.**
  - `lp_args` for printer `PDF`, paper `A4`, option `MediaType=Glossy` equals exactly
    `["-d","PDF","-t",<title>,"-o","PageSize=A4","-o","print-scaling=none","-o","MediaType=Glossy","--","/tmp/x.pdf"]`.
  - `job_title` keeps `-a "b" 100%.jpg` intact; a 200-character non-ASCII name is cut at a character boundary to 64 characters; 3 files end in ` (+2)`.
  - `parse_job_id("request id is PDF-3 (1 file(s))\n") == Some("PDF-3".into())`.
- [ ] **Step 2:** `cargo test` → FAIL. **Step 3:** Implement. **Step 4:** `cargo test` → PASS.
- [ ] **Step 5: End-to-end size check.** Print one solid-colour picture with `--printer PDF --layout 10x15`. Expected within 30 s: a new file in `~/PDF`; `pdfinfo` shows A4; rasterised at 127 dpi the first coloured area measures 750×500 px ±5 (150×100 mm). If it is scaled, find the option or filter responsible before continuing.
- [ ] **Step 6:** Commit `feat: send jobs to CUPS`.

---

### Task 6: Dialog state and remembered settings

**Files:** Create `src/settings.rs`, `src/ui/mod.rs`, `src/ui/model.rs`.

**Interfaces — Consumes:** `layout`, `job::{JobSpec, Checked}`, `ppd::*`, `cups::{Printer, SubmitRequest, job_title}`.
**Produces:**
```rust
pub struct PrinterPrefs { pub paper: Option<String>, pub options: BTreeMap<String, String> }
pub struct Settings { pub printer: Option<String>, pub layout: LayoutId, pub fit_to_frame: bool, pub sharpen: bool, pub printers: BTreeMap<String, PrinterPrefs> }
// Default: fit_to_frame = true, sharpen = true
impl Settings { pub fn load_from(path: &Path) -> Settings; pub fn save_to(&self, path: &Path) -> io::Result<()>; pub fn default_path() -> PathBuf; } // ~/.config/print-pictures/settings.toml, honouring XDG_CONFIG_HOME

pub struct Model { /* private */ }
impl Model {
    pub fn new(checked: Checked, printers: Vec<Printer>, caps_of: Box<dyn Fn(&str) -> PrinterCaps>, settings: Settings) -> Model;
    pub fn files(&self) -> &[PathBuf];
    pub fn printers(&self) -> &[Printer];
    pub fn printer(&self) -> Option<&Printer>;
    pub fn caps(&self) -> &PrinterCaps;
    pub fn paper(&self) -> &PaperChoice;
    pub fn option_value(&self, keyword: &str) -> Option<&str>;
    pub fn set_printer(&mut self, name: &str);
    pub fn set_paper(&mut self, id: &str);
    pub fn set_option(&mut self, keyword: &str, value: &str);
    pub fn layouts(&self) -> Vec<(LayoutId, String)>;      // available ones, with captions
    pub fn layout(&self) -> LayoutId;
    pub fn set_layout(&mut self, layout: LayoutId);
    pub fn copies(&self) -> u32;  pub fn set_copies(&mut self, n: u32);  // clamped 1..=99, starts at 1
    pub fn fill(&self) -> bool;   pub fn set_fill(&mut self, on: bool);
    pub fn sharpen(&self) -> bool; pub fn set_sharpen(&mut self, on: bool);
    pub fn pages(&self) -> Vec<PagePlan>;
    pub fn page_index(&self) -> usize;  pub fn next_page(&mut self);  pub fn prev_page(&mut self);
    pub fn page_label(&self) -> String;
    pub fn visible_images(&self) -> Vec<usize>;            // distinct, current page only
    pub fn advanced_options(&self) -> Vec<&PrinterOption>; // all but quality and paper type
    pub fn can_print(&self) -> bool;
    pub fn notice(&self) -> Option<String>;
    pub fn job(&self) -> Option<(JobSpec, SubmitRequest)>;
    pub fn settings(&self) -> Settings;                    // what to remember
}
```
Notices, one per line: "No printers are installed." / "Printer <name> is stopped." / "Skipped: <file names>".

- [ ] **Step 1: Write the failing tests.**
  - Settings round-trip; a corrupt or missing file gives defaults.
  - Remembered printer `Gone` with printers `[Office, PDF(default)]` → `PDF` selected.
  - No printers → `!can_print()`, `job()` is `None`, notice contains "No printers are installed.", `pages()` still non-empty.
  - Stopped printer → still `can_print()`, notice names it.
  - 250 files, Wallet → 28 pages, label "Page 1 of 28", `visible_images() == 0..9`; after `next_page` `9..18`.
  - On page 28, switching to Full page keeps the index in range; paging never leaves 0..len.
  - Changing to a paper where the layout is unavailable selects `FullPage`.
  - `set_copies(0)` → 1, `set_copies(500)` → 99.
  - Paper and option choices are remembered per printer and restored on switching back; remembered values the printer no longer offers are ignored.
  - `job()` carries paper id, chosen options (including advanced ones), copies, fill, sharpen.
  - No usable files → `!can_print()`.
- [ ] **Step 2:** `cargo test` → FAIL. **Step 3:** Implement. **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** Commit `feat: dialog model and settings`.

---

### Task 7: The Print Pictures window

**Files:** Create `src/ui/window.rs`, `src/ui/preview.rs`, `src/ui/layout_list.rs`; modify `src/ui/mod.rs`, `src/main.rs`.

**Interfaces — Consumes:** `Model`, `render::*`, `image_source::load`, `job::build_pdf`, `cups::submit`.
**Produces:** `pub fn run(files: Vec<PathBuf>) -> glib::ExitCode;` — used by `print-pictures FILES...`; `pub fn turn_view(page: &PagePlan, fits: &[Fit]) -> bool;` (true only for a one-slot page whose picture is turned).

- [ ] **Step 1:** Write failing tests for `turn_view`: one slot turned → true; one slot unturned → false; two turned slots → false. Implement; `cargo test` → PASS.
- [ ] **Step 2: Build the window** exactly as the spec sketch, default size 735×550. Requirements:
  - Application flags `NON_UNIQUE` (each right-click gets its own window) and `run_with_args::<&str>(&[])` so GTK ignores file arguments.
  - Quality and Paper type are hidden when the printer has none.
  - Sidebar entries: a small page drawing of the slot rectangles plus the caption; the selection is highlighted.
  - Preview draws the white page with `render` functions; when `turn_view`, the page is shown landscape with the picture upright. "◀ Page n of m ▶" below.
  - Pictures decode off the UI thread (`gio::spawn_blocking`), only `visible_images()`; others are dropped from the cache; stale results are discarded; a grey placeholder shows meanwhile.
  - Notice text appears above the bottom row when present.
  - Print: disable controls, show "Printing...", build to a temporary PDF and submit off the UI thread; success saves settings and closes; failure shows the message and keeps the window open. If `build_pdf` skipped pictures, success first shows a `gtk4::AlertDialog` reading `Skipped: <file names>` and closes the window when it is dismissed. `NothingToPrint` with a non-empty list shows the same text.
  - Enter prints, Escape cancels, Left/Right page.
- [ ] **Step 3: Verify on screen (Human check).** You: start `cargo run -- <12 pictures, including a portrait phone photo>` in the background, bring it forward with `wmctrl -a "Print Pictures"`, capture it with `xfce4-screenshooter -w -s <file.png>` and compare with the spec sketch. Your human partner: check every layout, both fit states, paging, copies = 3, printing to `PDF` (preview must match the file in `~/PDF`), a run with one bad file, and a run with 200+ pictures (window visible in about a second).
- [ ] **Step 4:** Ask your human partner to try it and say what differs from what they remember.
- [ ] **Step 5:** Commit `feat: print pictures window`.

---

### Task 8: Options and Printer Properties

**Files:** Create `src/ui/options.rs`; modify `src/ui/window.rs`, `src/ui/model.rs`.

**Interfaces — Consumes:** `Model::{sharpen, set_sharpen, advanced_options, option_value, set_option}`.
**Produces:** `pub fn show_print_settings(parent: &gtk4::Window, model: Rc<RefCell<Model>>, on_change: Rc<dyn Fn()>);`

- [ ] **Step 1:** Write a failing model test: with the fixture caps, `advanced_options()` contains `ColorModel` but neither `MediaType` nor `cupsPrintQuality`; setting `ColorModel` shows in `job()`. Make it pass.
- [ ] **Step 2:** "Options..." opens modal "Print Settings" with "Sharpen for printing" and "Printer Properties..." (a second modal listing each advanced option as label + dropdown). OK applies; Cancel and Escape discard.
- [ ] **Step 3 (Human check):** Verify with `PDF`: change a property, OK, reopen (kept); Cancel discards; after printing and restarting it is remembered.
- [ ] **Step 4:** Commit `feat: print settings`.

---

### Task 9: Thunar right-click entry and install

**Files:** Create `src/integration.rs`, `README.md`; modify `src/main.rs`.

**Interfaces — Produces:**
```rust
pub struct InstallError(pub String);                                          // thiserror
pub fn add_action(uca_xml: &str, exe: &Path) -> Result<String, InstallError>; // replaces our own previous entry
pub fn remove_action(uca_xml: &str) -> String;
pub fn desktop_entry(exe: &Path) -> String;
pub fn install(home: &Path, exe: &Path, system_uca: &Path) -> Result<(), InstallError>;
pub fn uninstall(home: &Path) -> Result<(), InstallError>;
```
Action: `unique-id` `print-pictures-1`, `icon` `printer`, `name` `Print`, `command` `"<exe>" %F`, `patterns` `*`, only `<image-files/>`. Binary goes to `~/.local/bin/print-pictures`; desktop entry (MimeType for the spec's formats, `Exec="<exe>" %F`) to `~/.local/share/applications/print-pictures.desktop`. Backup: `uca.xml.print-pictures.bak`. Whenever `install` or `uninstall` would change `uca.xml`, first copy the current file to the backup (nothing to copy if there is no user file yet). If the new text equals the current text, write neither file, so a second `install` leaves the first backup alone.

- [ ] **Step 1: Write the failing tests** with a two-action fixture.
  - `add_action` keeps both existing actions byte for byte and adds ours with `<image-files/>`.
  - Adding twice equals adding once; `remove_action` restores the original.
  - An exe path containing a space and `&` is quoted and XML-escaped.
  - XML without `</actions>` → error, and `install` writes nothing.
  - `install` into a temporary home: binary copied, backup equals the original; with no user `uca.xml` it starts from `system_uca`.
  - Running `install` twice: the backup still equals the original, pre-install `uca.xml`.
- [ ] **Step 2:** `cargo test` → FAIL. **Step 3:** Implement (text insertion before `</actions>`) and the `install` / `uninstall` commands. **Step 4:** `cargo test` → PASS.
- [ ] **Step 5:** README: required packages, `cargo run --release -- install`, `thunar -q`, uninstall.
- [ ] **Step 6: Acceptance (Human check).** Ask first: installing changes their Thunar menu and `thunar -q` closes their open Thunar windows. You: install and run `thunar -q`. Your human partner: reopen Thunar and do the rest. Select three pictures with spaces in their names → right-click shows "Print" → window lists all three → print to `PDF` → file appears in `~/PDF`. "Print" is absent for a text file. Existing Thunar actions still work. Try a HEIC photo if one is available and record the result.
- [ ] **Step 7:** Commit `feat: Thunar integration`.

---

## After this plan: the real printer

When the work printer is connected: run `print-pictures printers`, check paper types, quality and borderless sizes appear, print 10 x 15 and Wallet, and measure them with a ruler.
