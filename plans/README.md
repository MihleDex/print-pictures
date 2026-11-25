# Improvement plans

Written by the improve skill on 2026-11-25. These amend the build plan in
`docs/superpowers/plans/`; they do not replace it. Apply them first, then build
from the amended build plan.

## Execution order and status

| Plan | Title | Priority | Effort | Depends on | Status |
|------|-------|----------|--------|------------|--------|
| [001](001-amend-implementation-plan.md) | Amend the implementation plan | P1 | S | — | DONE |

Status values: TODO | IN PROGRESS | DONE | BLOCKED (reason) | REJECTED (reason)

## Before building (the user's own steps)

- Done: `sudo apt install libgtk-4-dev liblcms2-dev printer-driver-cups-pdf` (checked
  installed on 2026-11-25).

## Findings not planned

- Embed JPEG data to shrink big jobs: rejected by the user, 2026-11-25.
- Detect cut-off picture files: gdk-pixbuf loads most of them without an error.
- Work printer make and model: answered, the app must work on any printer.

## Checked and fine

- The planned Rust dependency versions resolve with cargo 1.100.0.
- The user's Thunar `uca.xml` has the structure Task 9 expects.
- An embedded JPEG colour profile survives a reduced-size load.
