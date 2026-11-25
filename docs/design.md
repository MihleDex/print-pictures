# Design note

## What it is for

People who printed photos with the Windows "Print Pictures" window and now use
Linux. Success: select pictures → right-click → Print → pick a layout → Print,
with no extra steps compared with Windows, no settings to redo for each job, and
nothing printed at the wrong size.

Out of scope: photo editing, a photo library, layouts or options Windows did not
have.

## Layouts

Counts are per A4 or Letter page.

| Caption | Size (mm) | Per page |
|---|---|---|
| Full page photo | printable area | 1 |
| 20 x 25 cm / 8 x 10 in | 200 × 250 / 203.2 × 254 | 1 |
| 13 x 18 cm / 5 x 7 in | 130 × 180 / 127 × 177.8 | 2 |
| 10 x 15 cm / 4 x 6 in | 100 × 150 / 101.6 × 152.4 | 2 |
| 9 x 13 cm / 3.5 x 5 in | 90 × 130 / 88.9 × 127 | 4 |
| Wallet | 57.15 × 82.55 | 9 |
| Contact sheet | 5 × 7 grid with file names | 35 |

Inch sizes appear where the system's paper setting is Letter. Counts come from
the paper with a nominal 5 mm margin and 4 mm gap, so they follow the paper size.
A printer's larger margins shrink the whole page of pictures slightly and never
change the count. A size matching the paper itself is offered, one per page.

## Pictures

Turned to match the frame; "Fit picture to frame" crops the overhang; phone
orientation honoured; transparency prints white; embedded colour profiles are
converted to sRGB; originals are never changed. Pictures are stored losslessly in
the print PDF (no JPEG re-compression).

## Printing

The preview and the print share one drawing routine. The PDF is made at 300 dpi
and sent with `lp` with scaling off. Printer settings come from the CUPS PPD, so
any CUPS printer works; a network printer that is found but not set up gets its
queue from `lpoptions -p NAME -l` first. A printer on the `cups-pdf:` device gets
no driver settings: the window asks where to save and writes the PDF itself.

## Getting "Print" into file managers

The package installs a Nemo action, a Dolphin service menu and small extensions
for Files (Nautilus) and Caja. Thunar has no place for a package to add one, so
the app adds it from its welcome window, keeping a backup of the user's actions.
Any other file manager has "Open With".

## Known limits

- Many cut-off picture files open without an error and print with part missing.
- A 200-page job takes about 7 minutes and shows only "Printing...".
- Many HEIC pictures may keep the window waiting while they are checked.
