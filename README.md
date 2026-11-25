# print-pictures

The Windows "Print Pictures" window for Linux Mint XFCE: select pictures in
Thunar, right-click, **Print**, pick a layout, **Print**. It works with any
printer that is set up in, or found by, CUPS.

## Install

Needs Rust and:

```sh
sudo apt install libgtk-4-dev liblcms2-dev
```

(`printer-driver-cups-pdf` adds a test printer, `PDF`, that writes to `~/PDF`.)

```sh
cargo run --release -- install
thunar -q          # then open Thunar again
```

This copies the program to `~/.local/bin`, adds "Print" to Thunar's right-click
menu for pictures, and adds Print Pictures to "Open With". Your Thunar actions
as they were are kept in `~/.config/Thunar/uca.xml.print-pictures.bak`.

To remove it:

```sh
~/.local/bin/print-pictures uninstall
thunar -q
```

## Command line

```sh
print-pictures FILES...                        # open the window
print-pictures printers                        # printers, paper sizes, options
print-pictures print --printer PDF --layout 10x15 FILES...
print-pictures render --layout wallet -o out.pdf FILES...
```

Layouts: `full`, `20x25`, `13x18`, `10x15`, `9x13`, `wallet`, `contact`.

## Good to know

- Choosing a printer that makes PDF files (such as `PDF`) asks where to save
  when you press Print. The `print` command still saves to `~/PDF`.
- A picture file that was cut off (a half-copied file) can still open and will
  then print with part of it missing.
- Large jobs take a while: 200 full A4 pages need about 7 minutes. Closing the
  window while it says "Printing..." abandons the job.
