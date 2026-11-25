# Checks still open before the first release

Tick each one after doing it. None of these can be done by the tests.

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
- [ ] Select pictures → right-click → "Print" appears (not for a text file); a
      name with spaces works.
- [ ] The window opens; a PDF printer asks where to save.
- [ ] Remove the package; "Print" is gone after reopening the file manager.

## Release

- [ ] `git tag v0.1.0 && git push origin v0.1.0`; the release page shows the
      `.deb` and `SHA256SUMS`, marked pre-release; download both and run
      `sha256sum -c SHA256SUMS`.
