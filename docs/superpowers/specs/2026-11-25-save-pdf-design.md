# Ask where to save when the printer makes PDF files

Date: 2026-11-25. Small change to `2026-11-25-print-pictures-design.md`.

## Intent

The user: "when printng pdf it doesnt ask me where to save it". Windows asked.
The `PDF` printer (`printer-driver-cups-pdf`) runs in the background and always
writes to `~/PDF`, so the window has to ask itself.

## Approaches

- **A (chosen by the user):** keep the `PDF` entry in the printer list; Print
  asks where to save and the app writes the file. No new entry; needs that
  printer to be installed.
- **B:** a separate "Save as PDF" entry. Works with no printers, but gives two
  similar entries and leaves `PDF` silent.

## Behaviour the user approved

- A printer that makes files (CUPS device `cups-pdf:`) → Print opens "Save As".
- Suggested name: the first picture's name with `.pdf`. Folder: the one last
  saved to, or the first picture's folder the first time.
- Save writes the file and closes the window, as printing does. Cancel returns
  to the window; nothing is written or remembered.
- The app writes its own print PDF, so the file is the previewed page.
- Other printers behave as before. The `print` command is unchanged and still
  sends to `~/PDF`.

## Builder's calls (not discussed)

- Quality, Paper type and "Printer Properties..." are hidden or disabled for
  such a printer: they cannot change a file the app writes itself. Saved values
  are kept.
- A name without `.pdf` gets it added, so a picture can never be overwritten. If
  that file already exists, "Save As" opens again on it, so the overwrite is
  confirmed.
- The saved file has ordinary permissions (readable by others, like a document).
- A location that is not a normal folder is refused with a message.

## Done when

Choosing `PDF` and pressing Print asks for a name, the file appears there, and
nothing new appears in `~/PDF`.
