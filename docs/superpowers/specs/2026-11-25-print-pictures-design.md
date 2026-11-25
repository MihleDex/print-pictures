# Print Pictures for Linux: research and design

Date: 2026-11-25

## Intent

The user prints many pictures at work and misses the convenience of the Windows
"Print Pictures" window since moving to Linux. Their words: "i just want the
closest thing possible to what i had".

**Success:** select pictures in Thunar → right-click → Print → pick a layout →
Print.

**Non-goals:** photo editing, a photo library, new layouts or options Windows
did not have, packaging for other distributions.

**Anti-goals:** any extra step compared with Windows; having to set the same
things again on every job; prints that come out at the wrong size.

## The Windows tool (research)

Seen in a screenshot of the Windows 10 dialog (Dutch language, A3, Adobe PDF):

- Title "Print Pictures", header "How do you want to print your pictures?"
- A row of labelled dropdowns: Printer, Paper size, Quality. Paper type is
  absent for a PDF printer.
- Large page preview, "Page 1 of 1" with previous/next buttons under it.
- Right-hand scrolling list of layouts, each a thumbnail with a caption; order
  Full page photo, 20 x 25 cm, 13 x 18 cm, ...
- Bottom: "Copies of each picture" number box, "Fit picture to frame"
  checkbox (ticked), "Options..." link, Print and Cancel.

Documented elsewhere:

- A4 layouts and counts: Full page photo (1), 20 x 25 cm (1), 13 x 18 cm (2),
  10 x 15 cm (2), 9 x 13 cm (4), Wallet (9), Contact sheet (35).
- Counts change with paper size (13 x 18 cm shows 4 on A3).
- "Fit picture to frame" enlarges the picture to fill the frame, cutting off
  what overhangs.
- Options include "Sharpen for printing" and "Color management"; printer
  preferences can be opened from the window.
- Contact sheets normally show file names.

Sources:
[Microsoft Learn](https://learn.microsoft.com/en-us/windows/win32/lwef/photo-printing-wizard) (XP/Vista only) ·
[Microsoft Q&A](https://learn.microsoft.com/en-us/answers/a/2729646) ·
[Omnivoor, with screenshots](https://www.omnivoor.nl/blog/2020/05/microsoft-afbeeldingen-afdrukken-functie) ·
[Windows Tips](https://windows.tips.net/T013713_Quick_Photo_Printing.html)

## This computer (checked)

Linux Mint 22.3, XFCE on X11, Thunar 4.18.8 (custom actions in
`~/.config/Thunar/uca.xml`), CUPS 2.4.7 with **no printers**, A4, locale
en_ZA, Rust 1.100.0, GTK 4.14.5 runtime without development packages,
gdk-pixbuf with HEIC support installed, `pdfinfo`/`pdftoppm` available.

Needed once:
`sudo apt install libgtk-4-dev liblcms2-dev printer-driver-cups-pdf`

`printer-driver-cups-pdf` creates a printer named `PDF` that writes to `~/PDF`.
Its definition has no margins, A4 default, a resolution option and no paper
type.

## Target behaviour

```
┌ Print Pictures ─────────────────────────────────────────┐
│ How do you want to print your pictures?                 │
│ Printer:       Paper size:   Quality:    Paper type:    │
│ [PDF      ▾]   [A4     ▾]    [300dpi ▾]  [hidden if n/a]│
│ ┌───────────────────────────────────────┬─────────────┐ │
│ │                                       │ [▣] Full    │ │
│ │            page preview               │ [▣] 20x25(1)│ │
│ │                                       │ [▤] 13x18(2)│ │
│ │         ◀  Page 1 of 3  ▶             │ ...scroll   │ │
│ └───────────────────────────────────────┴─────────────┘ │
│ Copies of each picture: [1]  ☑ Fit picture to frame     │
│                                              Options... │
│                                       [Print] [Cancel]  │
└─────────────────────────────────────────────────────────┘
```

### Layouts

| Caption | Picture size (mm) | Per A4 page |
|---|---|---|
| Full page photo | printable area | 1 |
| 20 x 25 cm | 200 × 250 | 1 |
| 13 x 18 cm | 130 × 180 | 2 |
| 10 x 15 cm | 100 × 150 | 2 |
| 9 x 13 cm | 90 × 130 | 4 |
| Wallet | 57.15 × 82.55 | 9 |
| Contact sheet | 5 × 7 grid with file names | 35 |

- Counts are worked out from the paper (5 mm nominal margin, 4 mm gap), so
  they follow paper size. A printer's larger margins shrink the whole page of
  pictures slightly and never change the count.
- A layout that cannot fit the paper is hidden. A picture size matching the
  paper itself (10 x 15 on 10 x 15 paper) is offered, one per page.

### Rules

- Pictures are turned 90° when that matches the frame's orientation.
- Fit ticked: fill the frame, crop the overhang equally. Unticked: show the
  whole picture inside the frame.
- Copies of a picture are consecutive (A A B B). Copies reset to 1 each time.
- Phone orientation data is honoured; transparency prints as white; embedded
  colour profiles are converted to sRGB; originals are never modified.
- JPEG, PNG, BMP, GIF (first frame), TIFF, WebP, HEIC.
- Remembered: printer, layout, fit, sharpen, and per printer its paper size
  and options.
- A file that cannot be read is skipped and named; the others still print.
- Options → "Print Settings": "Sharpen for printing" (on by default),
  "Printer Properties..." listing the printer's other settings.
- Thunar shows "Print" when only pictures are selected.

## Where it differs from Windows

- The test PDF printer saves to `~/PDF` without asking for a file name.
- Layout names are metric only.
- No "Only show options that are compatible with my printer", no "Color
  management" button, no help button.
- Printer Properties is a plain list from the driver, not the maker's dialog.
- It follows your GTK theme, not Windows styling.

## Approaches (builder's calls; not discussed with the user)

1. **GTK4 window, pages drawn with cairo to PDF, sent with `lp`** (chosen).
   The preview and the print share one drawing routine, physical sizes are
   exact, and every step can be tested without a printer.
2. **GTK's own print system** (`GtkPrintOperation`). Less code for sending
   jobs, but it does not let us build the Windows window around printer and
   paper choices in a testable way.
3. **egui or another pure-Rust toolkit.** No system development packages, but
   looks foreign on XFCE and needs more code for fonts and image formats.

GTK3 would match XFCE more closely, but its Rust bindings are no longer
maintained.

## Confirmed by the user (2026-11-25)

- After being given rendered sample pages: "The layouts are fine" (picture
  sizes, metric captions, file names on contact sheets, sharpening on by
  default).
- Pictures stay losslessly embedded in the PDF. No JPEG re-compression, although
  200 full A4 pages take about 7 minutes and 1.3 GB.
- "This app should be able to work on any printer." Nothing may be tailored to
  one printer.

## Not yet confirmed

- Nothing has been printed on paper, so sizes have not been measured there.
- Each printer's driver decides paper types, quality choices, margins and
  borderless sizes.
