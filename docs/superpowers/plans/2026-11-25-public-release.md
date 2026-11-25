# Public Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Anyone on the Mint/Ubuntu/Debian family can download one `.deb`, install it by double-click, and right-click pictures → Print; the GitHub page tells them how.

**Architecture:** The app barely changes: inch layouts, a welcome window when started without pictures, and XDG folders. Per-file-manager menu files live in `data/` and are shipped by `cargo-deb`; GitHub Actions builds, tests and releases.

**Tech Stack:** as before, plus `cargo-deb`, GitHub Actions, Python 3 for the Nautilus and Caja extensions. No new Rust dependencies.

**Spec:** `docs/superpowers/specs/2026-11-25-public-release-design.md`

## Global Constraints

- Targets: Linux Mint 22, Zorin OS 18, Ubuntu family 24.04, Debian 13, or newer; amd64 only.
- Package `print-pictures`; application id `io.github.mihledex.PrintPictures`; licence `GPL-3.0-or-later`.
- The repository is public: nothing private in a commit; keep the configured no-reply commit address; no quotes from the user in new files.
- Pushing, tagging, renaming and deleting anything on GitHub need the user's OK each time.
- Installing must not pull in a file manager the person does not use.
- Every task ends with `cargo test` passing and `cargo clippy --all-targets -- -D warnings` clean.

## Review Focus

1. A remembered metric layout on an inch system (or the reverse) → the matching size, not Full page (Task 3).
2. Names with spaces, quotes, non-ASCII or a leading `-` chosen in Files or Caja → each reaches the app as one intact path (Task 4).
3. A selection with a non-picture, or a file with no local path → no "Print" item (Task 4).
4. Thunar's actions file is broken → the welcome window shows the error and leaves the file alone (Task 2).
5. Someone who installed from source and then installs the package: the old `~/.local/bin` copy runs instead → the help says to remove it first (Task 6; no test).

## File Structure

```
src/integration.rs        folders, Thunar action (changed)
src/ui/welcome.rs         window shown when started without pictures (new)
src/layout.rs             units and inch layouts (changed)
data/                     menu entry, store description, icon (new)
data/menus/               one file per file manager, and their test (new)
tests/packaging.rs        checks the data files (new)
.github/workflows/        ci.yml, release.yml (new)
```

---

### Task 1: User folders and the Thunar action on its own

**Files:** Modify `src/integration.rs`, `src/main.rs`, `src/settings.rs`.

**Interfaces — Produces:**
```rust
pub struct Folders { pub config: PathBuf, pub data: PathBuf, pub bin: PathBuf }
impl Folders { pub fn new(home: &Path, xdg_config: Option<OsString>, xdg_data: Option<OsString>) -> Folders; }
pub fn add_thunar(config: &Path, exe: &Path, system_uca: &Path) -> Result<(), InstallError>; // changes only uca.xml
pub fn remove_thunar(config: &Path) -> Result<(), InstallError>;
pub fn has_thunar(config: &Path) -> bool; // our action is present
pub fn install(folders: &Folders, exe: &Path, system_uca: &Path) -> Result<(), InstallError>;
pub fn uninstall(folders: &Folders) -> Result<(), InstallError>;
```
`Settings::default_path` takes its config folder from `Folders`, so the XDG rule exists once. A program under `/usr` (the package) is never copied: `install` then only adds the Thunar action pointing at it, and `uninstall` only removes that action. The `install` help says: Add "Print" to Thunar's right-click menu; from a source build, also install the program for this user.
`config` is `$XDG_CONFIG_HOME` (else `~/.config`), `data` is `$XDG_DATA_HOME` (else `~/.local/share`), a relative or empty value being ignored; `bin` is `~/.local/bin`.

- [x] **Step 1: Write the failing tests** (adapt the existing `Home` helper to `Folders`):
  - `folders_follow_xdg`: `new("/home/u", Some("/x/c"), None)` → config `/x/c`, data `/home/u/.local/share`, bin `/home/u/.local/bin`; `Some("rel")` and `Some("")` give the defaults.
  - `thunar_alone`: `add_thunar` creates or changes only `Thunar/uca.xml` (no binary, no desktop entry); `has_thunar` false → true → false across add and `remove_thunar`; the backup rule of the existing tests still holds.
  - `a_packaged_program_is_not_copied`: `install` with exe `/usr/bin/print-pictures` writes only `uca.xml`, whose command is that path; there is no copy in `bin` and no desktop entry; `uninstall` with the same folders removes the action and leaves any file already in `bin`.
- [x] **Step 2:** `cargo test integration` → FAIL.
- [x] **Step 3: Implement.** `install` and `uninstall` call `add_thunar` / `remove_thunar`; remove the `shortcut:` comment about XDG.
- [x] **Step 4:** `cargo test` and clippy → PASS.
- [x] **Step 5:** Commit `feat: honour user folders and add the Thunar action on its own`.

---

### Task 2: Welcome window

**Files:** Create `src/ui/welcome.rs`; modify `src/ui/mod.rs`.

**Interfaces — Consumes:** Task 1's `add_thunar`, `remove_thunar`, `has_thunar`.
**Produces:** `pub(super) fn show(app: &gtk4::Application, config: PathBuf, exe: PathBuf, thunar_installed: bool) -> gtk4::ApplicationWindow;`

`ui::run` with no files opens this instead of the print window. `thunar_installed` is true when a `thunar` program is on `PATH`; `exe` is `std::env::current_exe()`.

Exact strings: title "Print Pictures"; text "Select pictures in your file manager, right-click them and choose Print."; button "Add Print to the right-click menu", or "Remove Print from the right-click menu" when it is there; after a change "Done. Close and reopen your file manager."; button "Close". Without Thunar there is no add/remove button.

- [x] **Step 1: Write the failing tests** (`#[gtk4::test] #[ignore = "needs a display"]`, finding widgets as `ui/options.rs` tests do, with a temporary `config`):
  - `welcome_adds_and_removes`: pressing "Add…" makes `has_thunar` true, shows the "Done." line and relabels the button to "Remove…"; pressing it again reverses this.
  - `welcome_without_thunar`: no button with either label.
  - `welcome_reports_a_broken_file`: with `uca.xml` = `<broken>`, pressing "Add…" shows a message containing "uca.xml" and the file still reads `<broken>`.
- [x] **Step 2:** `cargo test --lib welcome -- --ignored` → FAIL.
- [x] **Step 3: Implement.** The system file is `/etc/xdg/Thunar/uca.xml`.
- [x] **Step 4:** `cargo test --lib welcome -- --ignored`, `cargo test` and clippy → PASS.
- [x] **Step 5:** Commit `feat: welcome window`.

---

### Task 3: Inch sizes

**Files:** Modify `src/layout.rs`, `src/ui/model.rs`, `src/ui/window.rs`, `src/main.rs`.

**Interfaces — Produces:**
```rust
pub enum Units { Metric, Inch }                       // layout.rs
// LayoutId gains P8x10in, P5x7in, P4x6in, P35x5in with ids "8x10in", "5x7in", "4x6in", "3.5x5in"
impl LayoutId {
    pub fn offered(units: Units) -> [LayoutId; 7];
    pub fn in_units(self, units: Units) -> LayoutId;  // 20x25↔8x10, 13x18↔5x7, 10x15↔4x6, 9x13↔3.5x5
}
pub fn units_from_locale(locale_paper: &str) -> Units; // output of `locale -k LC_PAPER`
pub fn system_units() -> Units;                        // runs it; Metric if that fails
impl Model { pub fn with_units(self, units: Units) -> Model; } // default Metric
```
Sizes in mm: 203.2×254, 127×177.8, 101.6×152.4, 88.9×127. Captions "8 x 10 in", "5 x 7 in", "4 x 6 in", "3.5 x 5 in", with the count as for the others. `Inch` means the locale paper is 216 × 279.

- [x] **Step 1: Write the failing tests.**
  - Letter (215.9×279.4): counts over `offered(Inch)` are `[1, 1, 2, 2, 4, 9, 35]`; ids are `full, 8x10in, 5x7in, 4x6in, 3.5x5in, wallet, contact`.
  - Paper 101.6×152.4: `P4x6in` has one slot, 101.6×152.4 within 1e-6.
  - `label(P4x6in, letter) == "4 x 6 in (2)"`.
  - `P10x15.in_units(Inch) == P4x6in`, back again with `Metric`; `Wallet` unchanged.
  - `units_from_locale("height=279\nwidth=216\n") == Inch`; `"height=297\nwidth=210\n"` and `""` give `Metric`.
  - A model with `Inch` units and remembered `P13x18`: `layout() == P5x7in` and `layouts()` holds only offered ids. A `Metric` model with remembered `P4x6in` gives `P10x15`.
  - Existing tests that loop over `LayoutId::ALL` for the seven Windows layouts use `offered(Metric)`; `ALL` now holds all eleven.
- [x] **Step 2:** `cargo test` → FAIL.
- [x] **Step 3: Implement.** `Model::layout()` returns the remembered layout `in_units`; the window builds its model `with_units(system_units())`, read off the UI thread with the printers. Add the new ids to the `--layout` help.
- [x] **Step 4:** `cargo test` and clippy → PASS.
- [x] **Step 5:** `LC_PAPER=en_US.UTF-8 cargo run -- FILE` lists inch layouts (if that locale is not generated, say so and rely on the tests).
- [x] **Step 6:** Commit `feat: inch sizes for Letter-paper systems`.

---

### Task 4: Menu files and the package

**Files:** Create `data/io.github.mihledex.PrintPictures.{desktop,metainfo.xml,svg}`, `data/menus/{print-pictures.nemo_action,print-pictures-dolphin.desktop,nautilus_print_pictures.py,caja_print_pictures.py,test_menus.py}`, `tests/packaging.rs`; modify `Cargo.toml`, `src/ui/mod.rs`.

**Interfaces — Produces:** `pub const APPLICATION_ID: &str = "io.github.mihledex.PrintPictures";` in `ui` (made public for the test).

`data/menus/print-pictures.nemo_action`:
```ini
[Nemo Action]
Name=Print
Comment=Print the selected pictures
Exec=print-pictures %F
Icon-Name=printer
Selection=notnone
Mimetypes=image/*;
```
`data/menus/print-pictures-dolphin.desktop`:
```ini
[Desktop Entry]
Type=Service
MimeType=image/*;
Actions=printPictures;
X-KDE-Priority=TopLevel

[Desktop Action printPictures]
Name=Print
Icon=printer
Exec=print-pictures %F
```
`data/menus/nautilus_print_pictures.py`:
```python
from gi.repository import GLib, GObject, Nautilus


class PrintPictures(GObject.GObject, Nautilus.MenuProvider):
    def get_file_items(self, files):
        paths = [f.get_location().get_path() for f in files]
        if not paths or None in paths or not all(f.get_mime_type().startswith("image/") for f in files):
            return []
        item = Nautilus.MenuItem(name="PrintPictures::print", label="Print", tip="Print these pictures")
        item.connect("activate", lambda _item: GLib.spawn_async(
            ["print-pictures", *paths], flags=GLib.SpawnFlags.SEARCH_PATH))
        return [item]
```
`caja_print_pictures.py` is the same with `Caja` for `Nautilus` and the signature `get_file_items(self, window, files)`.

`[package]` gains `license = "GPL-3.0-or-later"`, `description = "Print photos in standard sizes, like Windows' Print Pictures"`, `repository = "https://github.com/MihleDex/print-pictures"`. `[package.metadata.deb]`:
```toml
maintainer = "MihleDex <52197701+MihleDex@users.noreply.github.com>"
copyright = "2026 MihleDex"
section = "graphics"
priority = "optional"
depends = "$auto, cups-client"
recommends = "heif-gdk-pixbuf, webp-pixbuf-loader, python3-nautilus, python3-caja"
assets = [
  ["target/release/print-pictures", "usr/bin/", "755"],
  ["data/io.github.mihledex.PrintPictures.desktop", "usr/share/applications/", "644"],
  ["data/io.github.mihledex.PrintPictures.metainfo.xml", "usr/share/metainfo/", "644"],
  ["data/io.github.mihledex.PrintPictures.svg", "usr/share/icons/hicolor/scalable/apps/", "644"],
  ["data/menus/print-pictures.nemo_action", "usr/share/nemo/actions/", "644"],
  ["data/menus/print-pictures-dolphin.desktop", "usr/share/kio/servicemenus/print-pictures.desktop", "644"],
  ["data/menus/nautilus_print_pictures.py", "usr/share/nautilus-python/extensions/print_pictures.py", "644"],
  ["data/menus/caja_print_pictures.py", "usr/share/caja-python/extensions/print_pictures.py", "644"],
]
```
The `.desktop` entry: `Name=Print Pictures`, `Exec=print-pictures %F`, `Icon=io.github.mihledex.PrintPictures`, `Categories=Graphics;`, the MIME types of `integration.rs`. The metainfo: same id, `metadata_license` CC0-1.0, `project_license` GPL-3.0-or-later, a screenshot at `https://raw.githubusercontent.com/MihleDex/print-pictures/main/docs/screenshot.png`, release 0.1.0. The icon is the implementer's own simple drawing of a printer with a photo.

- [ ] **Step 1: Ask the user** to rename the repository on GitHub (Settings → General → Repository name → `print-pictures`). Then `git remote set-url origin git@github.com:MihleDex/print-pictures.git`; `git ls-remote origin` succeeds.
- [x] **Step 2: Write the failing tests.**
  - `data/menus/test_menus.py` (`unittest`; load each extension with a stub `gi.repository` that records `spawn_async`), for both: images with paths `/p/-a "b" 100% Ü.jpg` and `/p/c d.png` → one item labelled "Print", and activating it spawns exactly `["print-pictures", <those two>]`; an empty selection, a `text/plain` among images, or a `None` path → `[]`.
  - `tests/packaging.rs`: runs `python3 data/menus/test_menus.py`; every menu file contains `print-pictures`; Nemo and Dolphin files contain `image/*` and `%F`; `APPLICATION_ID` equals the data file names.
- [x] **Step 3:** `cargo test --test packaging` → FAIL.
- [x] **Step 4: Create the files**, set the application id, and set `rust-version` to the oldest toolchain for which `cargo +<version> build` succeeds.
- [x] **Step 5:** `cargo test`, clippy → PASS. `desktop-file-validate data/*.desktop` and `appstreamcli validate --no-net data/*.metainfo.xml` → no errors.
- [x] **Step 6:** `cargo install cargo-deb --locked && cargo deb` → `dpkg-deb -c target/debian/*.deb` lists the eight assets; `dpkg-deb -f target/debian/*.deb Depends` contains `libgtk-4-1`, `liblcms2-2`, `cups-client`.
- [x] **Step 7:** Commit `feat: package for the Mint and Ubuntu family`.

---

### Task 5: Automatic builds and releases

**Files:** Create `.github/workflows/ci.yml`, `.github/workflows/release.yml`.

- [x] **Step 1: `ci.yml`** on push and pull request, `runs-on: ubuntu-24.04`: install `libgtk-4-dev liblcms2-dev poppler-utils cups cups-client desktop-file-utils appstream`; `sudo systemctl start cups` (two tests need it); stable Rust; `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test`; the two validators from Task 4.
- [x] **Step 2: `release.yml`** on tags `v*`, `permissions: contents: write`: same setup and tests; `cargo install cargo-deb --locked`; `cargo deb`; for each of `debian:13` and `ubuntu:26.04`, `docker run --rm -v "$PWD/target/debian:/d" IMAGE sh -c "apt-get update && apt-get install -y /d/*.deb && print-pictures --version && ! dpkg -l caja nautilus nemo dolphin thunar 2>/dev/null | grep -q '^ii'"` (the last part fails if a file manager was pulled in); `cd target/debian && sha256sum *.deb > SHA256SUMS`, so the file names carry no folder; `gh release create "$GITHUB_REF_NAME" target/debian/*.deb target/debian/SHA256SUMS --prerelease --generate-notes` with `GH_TOKEN: ${{ github.token }}`.
- [x] **Step 3:** Commit `ci: build, test and release`.
- [ ] **Step 4:** With the user's OK, push; the CI run on GitHub is green. If it is not, fix the cause and push again.

---

### Task 6: The GitHub page

**Files:** Rewrite `README.md`; create `docs/screenshot.png`, `CHANGELOG.md`, `CONTRIBUTING.md`, `.github/ISSUE_TEMPLATE/problem.yml`.

README headings, in order: "What it does", "Download and install", "How to use it", "Coming from Windows", "Help", "Remove it", "Build from source", "Licence". Written for someone who does not use a terminal. "Licence" says GPL version 3 or any later version, see `LICENSE`.

- Download link: `https://github.com/MihleDex/print-pictures/releases` (not `/latest`, which skips preview releases).
- "Help" covers: no "Print" in the right-click menu (reopen the file manager; Xfce: open Print Pictures from the menu and press "Add Print to the right-click menu"; otherwise "Open With"); double-click does not install (`sudo apt install ./print-pictures_*.deb`); installed from source before (`~/.local/bin/print-pictures uninstall` first); wrong size on paper; printer not listed (add it in the system's printer settings).
- "Remove it": on Xfce press "Remove Print from the right-click menu" before removing the package.
- The problem form requires: system and version, file manager, printer model, output of `print-pictures printers`.

- [x] **Step 1: Screenshot.** Make sample pictures by script (gradients and shapes; no system wallpapers, no one's photos), open the app on them, capture only its window.
- [x] **Step 2:** Write the files. State plainly that releases are previews until tested on more printers.
- [x] **Step 3:** Verify: `grep -c '^## ' README.md` → 8; every relative link and image path in `README.md` exists.
- [x] **Step 4:** Commit `docs: a page for people who just want to print`.

---

### Task 7: Tidy the public repository

**Files:** Create `docs/design.md`, `docs/release-checks.md`; modify `CLAUDE.md`; delete `docs/superpowers/`, `plans/`.

- [ ] **Step 1:** `docs/design.md`: under 80 lines — what the app is for, the layout table, the printing and file-manager decisions, known limits. No quotes.
- [ ] **Step 2:** `CLAUDE.md`: keep commands, architecture, rules and cases; paraphrase quotes; point to `docs/design.md`.
- [ ] **Step 3:** Write `docs/release-checks.md` with every check still open, so deleting the plans loses none: the window checks (first plan Task 7 Steps 3–4), the Options check (Task 8 Step 3), the Thunar check (Task 9 Step 6), measuring on a real printer, the Save As check (save-PDF plan Task 2 Step 8) and Task 8 below, each spelled out. Then `git rm -r docs/superpowers plans` (they remain in history).
- [ ] **Step 4:** Verify: `git grep -n 'superpowers\|"i just want\|This app should\|when printng' -- .` → nothing.
- [ ] **Step 5:** Commit `docs: remove planning notes from the tree`.
- [ ] **Step 6:** Ask the user: push; delete the old `print-pictures` branch on GitHub; set the description "Print photos in standard sizes on Linux, like Windows' Print Pictures" and topics `linux gtk4 printing photos linux-mint ubuntu` on the website.

---

### Task 8: Release checks and the first preview

- [ ] **Step 1 (Human check):** On the user's printer, print 10 x 15 (or 4 x 6) and Wallet and measure with a ruler: within 1 mm.
- [ ] **Step 2 (Human check):** Finish every check in `docs/release-checks.md`.
- [ ] **Step 3 (Human check):** The user installs virtual machine software (`sudo apt install virt-manager`) and, in Linux Mint Cinnamon, Ubuntu (or Zorin) and Kubuntu: double-click the `.deb`; select pictures → right-click → "Print" appears (not for a text file); a name with spaces works; "Save As" for a PDF printer; remove the package. Record each result; fix failures before Step 4.
- [ ] **Step 4:** With the user's OK: `git tag v0.1.0 && git push origin v0.1.0`. The release page shows the `.deb` and `SHA256SUMS`, marked pre-release; download it and check `sha256sum -c SHA256SUMS`.
