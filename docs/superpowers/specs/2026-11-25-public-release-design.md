# Making print-pictures available to others

Date: 2026-11-25. Extends `2026-11-25-print-pictures-design.md` and replaces
its "no packaging for other distributions" non-goal.

## Intent

People who moved from Windows to Linux and print many photos should get this
without developer tools and use it as they used Windows' Print Pictures.

**Success:** download one file → double-click → Install → select photos →
right-click → Print.

**Non-goals (first release):** Fedora, Arch, Flatpak; processors other than
64-bit PC; translations; systems older than those below; photo editing.

**Anti-goals:** a stranger's print at the wrong size; needing a terminal on the
normal path; damaging anyone's file-manager settings; any network access beyond
the local print server.

## Constraints

- Needs GTK 4.14 and glibc 2.39: Linux Mint 22, Zorin OS 18, the Ubuntu family
  24.04, Debian 13, or newer.
- Prints through the CUPS tools `lp`, `lpstat` and `lpoptions`.
- The repository is public and GPL version 3.
- It must still work on any printer.

## Approaches

1. **A `.deb` download from GitHub releases (chosen by the user).** Keeps
   right-click → Print and reuses what is built. Fedora comes later; updating
   means downloading again.
2. **Flatpak.** Wider reach and automatic updates, but the sandbox would not let
   us add the right-click entry the same way, and printing would likely need
   rework.

## Agreed

Chosen: approach 1. Proposed with it and not objected to: a friendlier GitHub
page (screenshots, three-step install, notes for Windows users, help), downloads
built automatically for each release, inch sizes where Letter paper is used,
planning notes removed from the public tree, and real prints measured before the
first release.

## Design

Everything below is the builder's proposal unless listed under "Agreed".

### Right-click "Print"

| File manager | Typical systems | How it is added |
|---|---|---|
| Nemo | Mint Cinnamon | action file installed by the package |
| Files (Nautilus) | Ubuntu, Zorin | small Python extension (`python3-nautilus`) |
| Dolphin | Kubuntu | service menu installed by the package |
| Caja | Mint MATE | small Python extension (`python3-caja`) |
| Thunar | Mint Xfce, Xubuntu | added per user by the app (existing code) |
| any other | | "Open With → Print Pictures" |

- Shown only when pictures are selected.
- Installing must not pull in a file manager the person does not use.
- Thunar has no system-wide drop-in. Started from the application menu with no
  pictures, the app shows a short welcome that explains use and, where Thunar is
  installed, offers "Add Print to the right-click menu" and "Remove". This is one
  extra step, once, for Xfce users only. It also replaces today's empty window.
- Removing the package cannot edit a person's Thunar settings; the help says to
  press "Remove" first.

### Inch sizes

Where the system's paper setting is Letter, the layouts are 3.5 x 5, 4 x 6,
5 x 7 and 8 x 10 in, plus Full page, Wallet and Contact sheet. On Letter paper
that gives 4, 2, 2, 1 per page, using the same layout rules. A size matching the
paper (4 x 6 in on 4 x 6 paper) fills it exactly. Elsewhere nothing changes.

### Package

- `print-pictures`: program in `/usr/bin`, menu entry, own icon, store
  description with a screenshot, and the right-click files above.
- Requires GTK 4.14, Little CMS and `cups-client`; recommends the HEIC and WebP
  picture loaders.
- Application id `io.github.mihledex.PrintPictures` (the current `local.` id is
  not suitable to publish).
- Respect `XDG_CONFIG_HOME` and `XDG_DATA_HOME`.
- The `install` / `uninstall` commands stay, for people who build from source.

### Releases

- On every push: build, lint and test on Ubuntu 24.04.
- On a version tag: build the `.deb`, check it installs in clean Debian 13 and
  current Ubuntu containers, attach it to a GitHub release with checksums.
- Versions stay 0.x, marked "preview", until the release checks below pass.

### GitHub page

- README for a non-developer: what it does, a screenshot, download and install,
  how to use it, "Coming from Windows", help, uninstall, then building from
  source and licence. The help gives a one-line terminal install for a system
  where double-click does not work.
- Screenshots use pictures we may publish, not system wallpapers or the user's
  photos.
- A change log, a short contributing note, and a problem report form asking for
  system, file manager, printer and the output of `print-pictures printers`.
- Planning notes (`docs/superpowers/`, `plans/`) leave the tree, replaced by a
  short design note. `CLAUDE.md` keeps its rules, without quotes. All remain in
  the history.

### Release checks (before announcing)

- 10 x 15 (or 4 x 6) and Wallet printed on a real printer and measured.
- The hands-on checks still open from the earlier plans.
- Installed from the `.deb` in virtual machines of Mint Cinnamon, Ubuntu (or
  Zorin) and Kubuntu: double-click install, right-click Print, one print, remove.
  This computer can run virtual machines but has no software for it yet.

## Order of work

1. File managers, welcome window, folder settings.
2. Inch sizes.
3. Package and automatic builds.
4. GitHub page.
5. Virtual machine tests and the first release.

Each stage leaves a working program.

## Left to the builder

Icon art, packaging tool, the exact content of each file-manager file (checked
against that file manager), and dropping Caja to "Open With" if it cannot be done
cleanly.

## Open for the user

- Rename the repository? "LinuxPPT" reads as PowerPoint to many Windows users.
- Licence: GPL version 3 only, or "version 3 or any later version"?
