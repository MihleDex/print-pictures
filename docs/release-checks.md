# Checks still open before the first release

Tick each one after doing it. None of these can be done by the tests.

## First

- [x] Rename the repository on GitHub to `print-pictures` (Settings → General).
      Every link in the files points there, so do this before merging into
      `main` or tagging. Then run
      `git remote set-url origin git@github.com:MihleDex/print-pictures.git`.

## On this computer (Xfce)

- [ ] Window: try every layout, "Fit picture to frame" on and off, paging, 3
      copies; include a portrait phone photo.
- [ ] A selection with one broken file: it is named, the rest print.
- [ ] 200 or more pictures: the window appears quickly.
- [ ] "Options..." → "Printer Properties...": change a setting, OK, reopen (kept);
      Cancel discards; after printing and restarting it is remembered.
- [ ] Thunar: select three pictures with spaces in their names → right-click →
      Print → all three listed → print. No "Print" for a text file. Your other
      right-click actions still work. A HEIC photo, if you have one.
- [ ] A PDF printer: Print opens "Save As"; Cancel returns to the window; Save
      writes the file there; the next time it opens in that folder. Typing an
      existing PDF's name without ".pdf" asks before replacing it.

## On a real printer

- [ ] `print-pictures printers` shows its paper types, quality and borderless
      sizes.
- [ ] Print 10 x 15 cm (or 4 x 6 in) and Wallet; measure with a ruler: within
      1 mm.

## Virtual machines

Needs `sudo apt install virt-manager` and disk images of Linux Mint Cinnamon,
Ubuntu (or Zorin OS) and Kubuntu. In each:

- [ ] Double-click the `.deb` and install it.
- [ ] Select pictures → right-click → "Print" appears (not for a text file).
      Include a file named like `-a "b" 100% Ü.jpg` (try Nemo and Dolphin).
- [ ] The window opens; a PDF printer asks where to save.
- [ ] Remove the package; "Print" is gone after reopening the file manager.
- [ ] On Ubuntu 26.04: a portrait phone photo comes out upright and a camera
      photo's colours look right (its picture loader is newer than the one tested).

## Release

- [ ] Set the 0.1.0 date in `data/io.github.mihledex.PrintPictures.metainfo.xml`
      and `CHANGELOG.md`.
- [ ] `git tag v0.1.0 && git push origin v0.1.0`; the release page shows the
      `.deb` and `SHA256SUMS`, marked pre-release; download both and run
      `sha256sum -c SHA256SUMS`.
