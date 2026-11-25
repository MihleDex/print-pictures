# Plan 001: Amend the Print Pictures implementation plan before anyone builds from it

> **Executor instructions**: This plan edits ONE markdown document. It writes no
> code. Apply the edits in order, then run the done criteria. If anything in
> "STOP conditions" occurs, stop and report. When done, update this plan's status
> row in `plans/README.md`.
>
> **Drift check (run first)**: there is no git repository, so compare a checksum.
> `sha256sum docs/superpowers/plans/2026-11-25-print-pictures.md` must print
> `e5e4cbef3bbd993f71f1dc5cc411c27d70e9b9f3a7f21eaf58a5a4bba71f2b1a`.
> If it differs, check that every "Replace" block below still appears exactly
> once in the file. If any does not, STOP.

## Status

- **Priority**: P1 (apply before Task 1 of the implementation plan starts)
- **Effort**: S
- **Risk**: LOW
- **Depends on**: none
- **Category**: dx
- **Planned at**: no commit (not a git repository), 2026-11-25

## Why this matters

`print-pictures` is a planned Rust/GTK4 copy of the Windows "Print Pictures"
window for Linux Mint. No code exists yet. Its implementation plan has six
problems that would cost an executor time or produce wrong behaviour:

1. A colour test asks for a PNG that gdk-pixbuf 2.42.10 refuses to save.
2. A picture that passes the header check but fails to load aborts the whole
   job, although the spec says "A file that cannot be read is skipped and named;
   the others still print."
3. Three on-screen checks need clicking and typing, which an agent cannot do
   here (`xdotool` is not installed).
4. Nothing stops a second `install` overwriting the backup of the user's Thunar
   actions.
5. Large jobs may produce very large PDFs (estimated about 12 MB per full A4
   page); nobody is told to measure it.
6. The user sees no output until Task 7 of 9, although picture sizes and captions
   are unconfirmed.

## Current state

- `docs/superpowers/plans/2026-11-25-print-pictures.md` — nine-task
  implementation plan. The only file you edit.
- `docs/superpowers/specs/2026-11-25-print-pictures-design.md` — the spec. Read
  only.

Facts measured on this machine (gdk-pixbuf 2.42.10), which the new text relies on:

- Saving a PNG with an RGB `icc-profile` option fails; saving a JPEG with it
  works, and the profile survives `new_from_file_at_scale`.
- A PNG whose body after the start of `IDAT` is zeroed passes
  `Pixbuf::file_info` but `new_from_file_at_scale` returns an error.
- A file that is merely cut off (PNG, JPEG, BMP) loads **without** an error
  through `new_from_file_at_scale`. This plan records that as a known limit; it
  does not try to detect it.

## Commands you will need

| Purpose  | Command | Expected on success |
|----------|---------|---------------------|
| Checksum | `sha256sum docs/superpowers/plans/2026-11-25-print-pictures.md` | value above |
| Backup   | `cp docs/superpowers/plans/2026-11-25-print-pictures.md plans/001-before.md` | exit 0 |
| Review   | `diff plans/001-before.md docs/superpowers/plans/2026-11-25-print-pictures.md` | only the edits below |

## Scope

**In scope**: `docs/superpowers/plans/2026-11-25-print-pictures.md`, and the
status row in `plans/README.md`.

**Out of scope**: the spec, `CLAUDE.md`, any code, `~/.config`. Do not run
`git init` or commit anything. Do not tick any checkbox in the implementation
plan, and do not carry out any of its tasks.

## Steps

First run the checksum, then the backup command.

Each edit below gives text to find and text to put in its place. Match the text
exactly, including leading spaces. Each "Replace" block appears exactly once.

### Edit 1: Say who does on-screen checks

**Replace:**
````text
- Every task ends with `cargo test` passing and `cargo clippy --all-targets -- -D warnings` clean.
````
**With:**
````text
- Every task ends with `cargo test` passing and `cargo clippy --all-targets -- -D warnings` clean.
- Checks that need clicking or typing in a window are done by your human partner: there is no input-automation tool here (`xdotool` is not installed). Prepare the files and a numbered list, and tick the box only after they report the result.
````

### Edit 2: Record what "skipped" covers

**Replace:**
````text
none usable → message, nothing sent (Task 3).
````
**With:**
````text
none usable → message, nothing sent (Task 3). This includes a picture that passes the header check and then fails to load (Tasks 3, 7). Known limit: gdk-pixbuf loads many cut-off files without an error, so those print with part missing.
````

### Edit 3: Check the git identity up front

**Replace:**
````text
Verify: `pkg-config --modversion gtk4` prints `4.14.x` and `lpstat -v` lists `PDF`.
````
**With:**
````text
Verify: `pkg-config --modversion gtk4` prints `4.14.x` and `lpstat -v` lists `PDF`. Also verify `git var GIT_AUTHOR_IDENT` exits 0; if not, ask them to set `user.name` and `user.email` (never choose these yourself).
````

### Edit 4: Commit the documents first

**Replace:**
````text
- [ ] **Step 2:** `git init`; create the package
````
**With:**
````text
- [ ] **Step 2:** `git init` (skip if already a repository) and commit `CLAUDE.md`, `docs/` and `plans/` as `docs: print pictures spec and plan`; then create the package
````

### Edit 5: Make the colour test possible

**Replace:**
````text
save a pure-red PNG carrying it (pixbuf save option `icc-profile`, base64) → loaded pixel has b > 200 and r < 60.
````
**With:**
````text
save a pure-red JPEG carrying it (pixbuf `savev` options `quality` = `95` and `icc-profile` = base64 of the profile) → loaded pixel has b > 200 and r < 60. It must be JPEG: gdk-pixbuf 2.42.10 refuses to save a PNG with an RGB profile.
````

### Edit 6: Let `build_pdf` report skipped pictures

**Replace:**
````text
pub fn build_pdf(spec: &JobSpec, out: &Path) -> Result<usize, JobError>;  // page count
pub enum JobError { NothingToPrint, LayoutUnavailable, Picture { path: PathBuf, source: LoadError }, Pdf(String) }
````
**With:**
````text
pub struct Built { pub pages: usize, pub skipped: Vec<(PathBuf, String)> } // skipped = passed the check, failed to load
pub fn build_pdf(spec: &JobSpec, out: &Path) -> Result<Built, JobError>;
pub enum JobError { NothingToPrint(Vec<(PathBuf, String)>), LayoutUnavailable, Pdf(String) } // the Vec lists skipped pictures
````

### Edit 7: `render` lists skipped files

**Replace:**
````text
(zero margins; rejected files listed on stderr).
````
**With:**
````text
(zero margins; rejected and skipped files listed on stderr).
````

### Edit 8: Page count assertion

**Replace:**
````text
3 solid pictures, 13x18, A4 → returns 2;
````
**With:**
````text
3 solid pictures, 13x18, A4 → returns `pages == 2`;
````

### Edit 9: Add the damaged-picture test

**Replace:**
````text
`build_pdf` with no files → `NothingToPrint` and no output file.
````
**With:**
````text
`build_pdf` with no files → `NothingToPrint` with an empty list and no output file.
  - `damaged_body_is_skipped`: write a 200×200 PNG with varied pixels; keep it up to and including the 16 bytes that follow `IDAT`, and set every later byte to zero. Assert first that `check_files` lists it as usable and `load` returns an error. Then `build_pdf` of `[good, damaged, good]` at 13x18 → `pages == 1`, `skipped` names only the damaged file, output exists. `build_pdf` of `[damaged]` → `NothingToPrint` listing it, and no output file.
````

### Edit 10: Say how skipping works

**Replace:**
````text
`check_files` uses `probe`.
````
**With:**
````text
`check_files` uses `probe`. When a picture fails to load, `build_pdf` removes that file from its list, records it in `skipped` and builds again from the start, so no slot is left blank (add `// shortcut: one rebuild per damaged picture; fine while they are rare`). If none remain, return `NothingToPrint(skipped)`.
````

### Edit 11: Add the early look and the size measurement to Task 3

**Replace:**
````text
- [ ] **Step 5:** `cargo test` → PASS. Run `cargo run -- render --layout wallet -o /tmp/w.pdf <a few pictures>` and look at the result.
- [ ] **Step 6:** Commit `feat: render pictures to PDF`.
````
**With:**
````text
- [ ] **Step 5:** `cargo test` → PASS.
- [ ] **Step 6: Early look (Human check).** Ask your human partner for a few of their own pictures, then render every layout:
  `mkdir -p target/layout-check && for l in full 20x25 13x18 10x15 9x13 wallet contact; do cargo run -- render --layout $l -o target/layout-check/$l.pdf <pictures>; done`
  Also render one layout with `--no-sharpen`. Ask them to compare with what they remember (holding a printed page against a frame if they can): picture sizes, metric captions such as "13 x 18 cm (2)", file names on the contact sheet, sharpening on by default. Record each answer under "Not yet confirmed" in the spec. If a size or caption changes, update the spec's layout table and Task 1's tests before going on.
- [ ] **Step 7: Measure a big job.** Run:
  ```sh
  mkdir -p target/timing
  i=0; while [ $i -lt 200 ]; do for f in /usr/share/backgrounds/*/*.jpg; do [ $i -lt 200 ] || break; i=$((i+1)); ln -sf "$f" target/timing/$i.jpg; done; done
  cargo build --release
  time target/release/print-pictures render --layout full -o target/timing/big.pdf target/timing/*.jpg
  ls -lh target/timing/big.pdf
  ```
  Tell your human partner both numbers (the estimate beforehand was about 2.4 GB). Do not change how pictures are embedded without their decision. Then `rm -r target/timing`.
- [ ] **Step 8:** Commit `feat: render pictures to PDF`.
````

### Edit 12: `print` lists skipped files

**Replace:**
````text
[--no-sharpen] [-o KEY=VALUE]... FILES...`
````
**With:**
````text
[--no-sharpen] [-o KEY=VALUE]... FILES...` (rejected and skipped files listed on stderr).
````

### Edit 13: The window names skipped pictures before closing

**Replace:**
````text
success saves settings and closes; failure shows the message and keeps the window open.
````
**With:**
````text
success saves settings and closes; failure shows the message and keeps the window open. If `build_pdf` skipped pictures, success first shows a `gtk4::AlertDialog` reading `Skipped: <file names>` and closes the window when it is dismissed. `NothingToPrint` with a non-empty list shows the same text.
````

### Edit 14: Split the Task 7 check

**Replace:**
````text
- [ ] **Step 3: Verify on screen.** `cargo run -- <12 pictures, including a portrait phone photo>`; capture with `xfce4-screenshooter -w -s <file.png>` and compare with the spec sketch. Then check every layout,
````
**With:**
````text
- [ ] **Step 3: Verify on screen (Human check).** You: start `cargo run -- <12 pictures, including a portrait phone photo>` in the background, bring it forward with `wmctrl -a "Print Pictures"`, capture it with `xfce4-screenshooter -w -s <file.png>` and compare with the spec sketch. Your human partner: check every layout,
````

### Edit 15: Mark the Task 8 check

**Replace:**
````text
- [ ] **Step 3:** Verify with `PDF`: change a property,
````
**With:**
````text
- [ ] **Step 3 (Human check):** Verify with `PDF`: change a property,
````

### Edit 16: Protect the Thunar backup

**Replace:**
````text
Backup: `uca.xml.print-pictures.bak`.
````
**With:**
````text
Backup: `uca.xml.print-pictures.bak`. Whenever `install` or `uninstall` would change `uca.xml`, first copy the current file to the backup (nothing to copy if there is no user file yet). If the new text equals the current text, write neither file, so a second `install` leaves the first backup alone.
````

### Edit 17: Test the backup rule

**Replace:**
````text
with no user `uca.xml` it starts from `system_uca`.
````
**With:**
````text
with no user `uca.xml` it starts from `system_uca`.
  - Running `install` twice: the backup still equals the original, pre-install `uca.xml`.
````

### Edit 18: Mark the Task 9 acceptance check

**Replace:**
````text
- [ ] **Step 6: Acceptance.** Install, `thunar -q`, reopen Thunar. Select three pictures
````
**With:**
````text
- [ ] **Step 6: Acceptance (Human check).** Ask first: installing changes their Thunar menu and `thunar -q` closes their open Thunar windows. You: install and run `thunar -q`. Your human partner: reopen Thunar and do the rest. Select three pictures
````

## Test plan

No code changes, so no tests. The done criteria below check the text.

## Done criteria

Run from the repository root with
`F=docs/superpowers/plans/2026-11-25-print-pictures.md`. ALL must hold:

- [ ] `grep -c 'Result<usize, JobError>' $F` prints `0`
- [ ] `grep -c 'Result<Built, JobError>' $F` prints `1`
- [ ] `grep -c 'source: LoadError }' $F` prints `0`
- [ ] `grep -c 'save a pure-red PNG' $F` prints `0`
- [ ] `grep -c 'save a pure-red JPEG' $F` prints `1`
- [ ] `grep -c 'damaged_body_is_skipped' $F` prints `1`
- [ ] `grep -c 'Human check' $F` prints `4`
- [ ] `grep -c 'big.pdf' $F` prints `2`
- [ ] `grep -c 'leaves the first backup alone' $F` prints `1`
- [ ] `grep -c '/tmp/w.pdf' $F` prints `0`
- [ ] `grep -c '^- \[x\]' $F` prints `0`
- [ ] `plans/README.md` status row updated

## STOP conditions

Stop and report if:

- A "Replace" block is missing or appears more than once.
- Any checkbox in the implementation plan is already ticked, or `src/` exists:
  building has started, and the edits must be reconciled with the code.
- A done criterion still fails after you recheck the edit it covers.

## Maintenance notes

- Delete `plans/001-before.md` once the diff has been reviewed.
- `build_pdf`'s return type is used by Task 3 (`render`), Task 5 (`print`) and
  Task 7 (window). A reviewer should check all three handle `skipped`.
- Deferred: embedding JPEG data in the PDF to shrink big jobs. Decide after the
  Task 3 measurement.
- Deferred: detecting cut-off files. gdk-pixbuf gives no error for them.
