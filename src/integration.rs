//! The Thunar right-click action and the desktop entry.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

const OUR_ID: &str = "<unique-id>print-pictures-1</unique-id>";
const END: &str = "</actions>";
const NO_ACTIONS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n";
const BACKUP: &str = "uca.xml.print-pictures.bak";
/// What Thunar uses for someone who has no actions file of their own.
pub const SYSTEM_UCA: &str = "/etc/xdg/Thunar/uca.xml";
const MIME_TYPES: &str =
    "image/jpeg;image/png;image/bmp;image/gif;image/tiff;image/webp;image/heic;image/heif;";

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct InstallError(pub String);

/// `exe` as one quoted word of a command line, with `%` doubled.
fn quoted(exe: &Path) -> String {
    let mut word = String::from("\"");
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '\\' | '$' | '`' => word.extend(['\\', c]),
            '%' => word.push_str("%%"),
            _ => word.push(c),
        }
    }
    word.push('"');
    word
}

pub fn add_action(uca_xml: &str, exe: &Path) -> Result<String, InstallError> {
    let command = quoted(exe)
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    let action = format!(
        "<action>\n\
         \t<icon>printer</icon>\n\
         \t<name>Print</name>\n\
         \t<submenu></submenu>\n\
         \t{OUR_ID}\n\
         \t<command>{command} %F</command>\n\
         \t<description>Print pictures</description>\n\
         \t<range></range>\n\
         \t<patterns>*</patterns>\n\
         \t<image-files/>\n\
         </action>\n"
    );
    let mut xml = remove_action(uca_xml);
    let end = xml.rfind(END).ok_or_else(|| {
        InstallError(format!(
            "Thunar's uca.xml has no {END}; it was left unchanged"
        ))
    })?;
    xml.insert_str(end, &action);
    Ok(xml)
}

pub fn remove_action(uca_xml: &str) -> String {
    let mut xml = uca_xml.to_string();
    while let Some(id) = xml.find(OUR_ID) {
        let Some(start) = xml[..id].rfind("<action>") else {
            break;
        };
        let Some(length) = xml[id..].find("</action>") else {
            break;
        };
        let mut end = id + length + "</action>".len();
        if xml[end..].starts_with('\n') {
            end += 1;
        }
        xml.replace_range(start..end, "");
    }
    xml
}

pub fn desktop_entry(exe: &Path) -> String {
    // A desktop file reads backslashes once more, as string escapes.
    let exec = quoted(exe).replace('\\', "\\\\");
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=Print Pictures\n\
         Comment=Print pictures in photo layouts\n\
         Exec={exec} %F\n\
         Icon=printer\n\
         Terminal=false\n\
         Categories=Graphics;\n\
         MimeType={MIME_TYPES}\n"
    )
}

/// Where a user's own files go.
pub struct Folders {
    pub config: PathBuf,
    pub data: PathBuf,
    pub bin: PathBuf,
}

impl Folders {
    /// The XDG variables count only when they hold an absolute path.
    pub fn new(home: &Path, xdg_config: Option<OsString>, xdg_data: Option<OsString>) -> Folders {
        let or_home = |set: Option<OsString>, fallback: &str| {
            set.map(PathBuf::from)
                .filter(|dir| dir.is_absolute())
                .unwrap_or_else(|| home.join(fallback))
        };
        Folders {
            config: or_home(xdg_config, ".config"),
            data: or_home(xdg_data, ".local/share"),
            bin: home.join(".local/bin"),
        }
    }

    fn binary(&self) -> PathBuf {
        self.bin.join("print-pictures")
    }

    fn entry(&self) -> PathBuf {
        self.data.join("applications/print-pictures.desktop")
    }
}

/// True for the packaged program, which is already installed for everyone.
pub fn is_system_install(exe: &Path) -> bool {
    exe.starts_with("/usr")
}

fn uca_file(config: &Path) -> PathBuf {
    config.join("Thunar/uca.xml")
}

fn about(path: &Path) -> impl FnOnce(io::Error) -> InstallError {
    let path = path.display().to_string();
    move |e| InstallError(format!("{path}: {e}"))
}

/// Writes a file in one step, so it is never left half written. Also works for a
/// program that is running.
fn replace(path: &Path, contents: &[u8], mode: u32) -> Result<(), InstallError> {
    let write = || {
        let dir = path.parent().unwrap_or(Path::new("."));
        fs::create_dir_all(dir)?;
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(contents)?;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(mode))?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    };
    write().map_err(about(path))
}

/// `None` only if the file does not exist; an unreadable one is an error.
fn read_if_there(path: &Path) -> Result<Option<String>, InstallError> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(about(path)(e)),
    }
}

/// Changes uca.xml, first saving what was there. Unchanged text writes nothing,
/// so a second install keeps the first backup.
fn change_uca(uca: &Path, current: Option<&str>, new: &str) -> Result<(), InstallError> {
    if current == Some(new) {
        return Ok(());
    }
    // Write through a symlink rather than replacing it.
    let target = fs::canonicalize(uca).unwrap_or_else(|_| uca.to_path_buf());
    let mut mode = 0o600;
    if let Some(current) = current {
        mode = fs::metadata(&target)
            .map_err(about(&target))?
            .permissions()
            .mode();
        replace(&uca.with_file_name(BACKUP), current.as_bytes(), mode)?;
    }
    replace(&target, new.as_bytes(), mode)
}

/// A change to uca.xml that has been worked out but not yet written.
struct Planned {
    uca: PathBuf,
    current: Option<String>,
    new: String,
}

/// Fails, before anything is written, if the file is not understood.
fn plan_add(config: &Path, exe: &Path, system_uca: &Path) -> Result<Planned, InstallError> {
    let uca = uca_file(config);
    let current = read_if_there(&uca)?;
    let base = match &current {
        Some(text) => text.clone(),
        None => read_if_there(system_uca)?.unwrap_or_else(|| NO_ACTIONS.into()),
    };
    let new = add_action(&base, exe)?;
    Ok(Planned { uca, current, new })
}

/// Adds "Print" to Thunar's menu for this user, running `exe`.
pub fn add_thunar(config: &Path, exe: &Path, system_uca: &Path) -> Result<(), InstallError> {
    let planned = plan_add(config, exe, system_uca)?;
    change_uca(&planned.uca, planned.current.as_deref(), &planned.new)
}

pub fn remove_thunar(config: &Path) -> Result<(), InstallError> {
    let uca = uca_file(config);
    if let Some(current) = read_if_there(&uca)? {
        change_uca(&uca, Some(&current), &remove_action(&current))?;
    }
    Ok(())
}

pub fn has_thunar(config: &Path) -> bool {
    std::fs::read_to_string(uca_file(config)).is_ok_and(|text| text.contains(OUR_ID))
}

pub fn install(folders: &Folders, exe: &Path, system_uca: &Path) -> Result<(), InstallError> {
    if is_system_install(exe) {
        // Copying it would put a stale copy in front of the packaged one.
        return add_thunar(&folders.config, exe, system_uca);
    }
    let binary = folders.binary();
    let planned = plan_add(&folders.config, &binary, system_uca)?;
    let program = fs::read(exe).map_err(about(exe))?;
    replace(&binary, &program, 0o755)?;
    replace(&folders.entry(), desktop_entry(&binary).as_bytes(), 0o644)?;
    change_uca(&planned.uca, planned.current.as_deref(), &planned.new)
}

pub fn uninstall(folders: &Folders, exe: &Path) -> Result<(), InstallError> {
    remove_thunar(&folders.config)?;
    if is_system_install(exe) {
        return Ok(());
    }
    for path in [folders.binary(), folders.entry()] {
        match fs::remove_file(&path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(about(&path)(e)),
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const TWO_ACTIONS: &str = concat!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
        "<actions>\n",
        "<action>\n\t<icon>Terminal</icon>\n\t<name>Open in Terminal</name>\n",
        "\t<unique-id>1795614082074386-1</unique-id>\n\t<command>exo-open %f</command>\n",
        "\t<patterns>*</patterns>\n\t<directories/>\n</action>\n",
        "<action>\n\t<name>Create a Link</name>\n\t<unique-id>1795614082074980-3</unique-id>\n",
        "\t<command>ln -s %f  &apos;Link to %n&apos;</command>\n\t<other-files/>\n</action>\n",
        "</actions>\n",
    );

    fn exe() -> &'static Path {
        Path::new("/home/u/.local/bin/print-pictures")
    }

    #[test]
    fn adding_keeps_what_is_there() {
        let added = add_action(TWO_ACTIONS, exe()).unwrap();
        let (before, end) = TWO_ACTIONS.split_at(TWO_ACTIONS.find("</actions>").unwrap());
        assert!(added.starts_with(before) && added.ends_with(end));
        let ours = &added[before.len()..added.len() - end.len()];
        for part in [
            "<unique-id>print-pictures-1</unique-id>",
            "<icon>printer</icon>",
            "<name>Print</name>",
            "<command>\"/home/u/.local/bin/print-pictures\" %F</command>",
            "<patterns>*</patterns>",
            "<image-files/>",
        ] {
            assert!(ours.contains(part), "{part} missing from {ours}");
        }
        for other in [
            "<directories/>",
            "<other-files/>",
            "<video-files/>",
            "<text-files/>",
        ] {
            assert!(!ours.contains(other), "{other}");
        }
    }

    #[test]
    fn adding_twice_is_adding_once_and_removing_restores() {
        let once = add_action(TWO_ACTIONS, exe()).unwrap();
        assert_eq!(add_action(&once, exe()).unwrap(), once);
        assert_eq!(remove_action(&once), TWO_ACTIONS);
        assert_eq!(remove_action(TWO_ACTIONS), TWO_ACTIONS);
    }

    #[test]
    fn odd_install_paths_are_quoted_and_escaped() {
        let exe = Path::new("/home/a b&c/$x 5%/\"q\"/print-pictures");
        let added = add_action(TWO_ACTIONS, exe).unwrap();
        // Shell quoting first, then Thunar's %%, then XML.
        let command = "<command>\"/home/a b&amp;c/\\$x 5%%/\\\"q\\\"/print-pictures\" %F</command>";
        assert!(added.contains(command), "{added}");
        // A desktop file also reads backslashes once as string escapes.
        let entry = desktop_entry(exe);
        assert!(
            entry.contains("Exec=\"/home/a b&c/\\\\$x 5%%/\\\\\"q\\\\\"/print-pictures\" %F\n"),
            "{entry}"
        );
    }

    #[test]
    fn the_desktop_entry_lists_the_formats() {
        let entry = desktop_entry(exe());
        assert!(entry.starts_with("[Desktop Entry]\n"));
        assert!(entry.contains("Name=Print Pictures\n"));
        for format in ["jpeg", "png", "bmp", "gif", "tiff", "webp", "heic", "heif"] {
            assert!(entry.contains(&format!("image/{format};")), "{format}");
        }
    }

    struct Home {
        dir: tempfile::TempDir,
        program: PathBuf,
        system: PathBuf,
    }

    impl Home {
        fn new(user_uca: Option<&str>) -> Home {
            let dir = tempfile::tempdir().unwrap();
            let program = dir.path().join("built");
            fs::write(&program, "the program").unwrap();
            let system = dir.path().join("system-uca.xml");
            fs::write(&system, TWO_ACTIONS).unwrap();
            let home = Home {
                dir,
                program,
                system,
            };
            if let Some(text) = user_uca {
                fs::create_dir_all(home.uca().parent().unwrap()).unwrap();
                fs::write(home.uca(), text).unwrap();
            }
            home
        }

        fn at(&self, relative: &str) -> PathBuf {
            self.dir.path().join("home").join(relative)
        }

        fn uca(&self) -> PathBuf {
            self.at(".config/Thunar/uca.xml")
        }

        fn backup(&self) -> PathBuf {
            self.at(".config/Thunar/uca.xml.print-pictures.bak")
        }

        fn folders(&self) -> Folders {
            Folders::new(&self.dir.path().join("home"), None, None)
        }

        fn install(&self) -> Result<(), InstallError> {
            install(&self.folders(), &self.program, &self.system)
        }

        fn uninstall(&self) -> Result<(), InstallError> {
            uninstall(&self.folders(), &self.program)
        }
    }

    const BINARY: &str = ".local/bin/print-pictures";
    const ENTRY: &str = ".local/share/applications/print-pictures.desktop";

    #[test]
    fn install_backs_up_once_and_uninstall_restores() {
        let home = Home::new(Some(TWO_ACTIONS));
        home.install().unwrap();
        assert_eq!(fs::read_to_string(home.at(BINARY)).unwrap(), "the program");
        assert!(
            fs::read_to_string(home.at(ENTRY))
                .unwrap()
                .contains("print-pictures")
        );
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), TWO_ACTIONS);
        let installed = fs::read_to_string(home.uca()).unwrap();
        assert!(installed.contains("print-pictures-1"));

        home.install().unwrap();
        assert_eq!(fs::read_to_string(home.uca()).unwrap(), installed);
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), TWO_ACTIONS);

        home.uninstall().unwrap();
        assert_eq!(fs::read_to_string(home.uca()).unwrap(), TWO_ACTIONS);
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), installed);
        assert!(!home.at(BINARY).exists() && !home.at(ENTRY).exists());
        home.uninstall().unwrap();
    }

    #[test]
    fn without_a_user_file_it_starts_from_the_system_one() {
        let home = Home::new(None);
        home.install().unwrap();
        let installed = fs::read_to_string(home.uca()).unwrap();
        assert!(installed.contains("Open in Terminal") && installed.contains("print-pictures-1"));
        assert!(!home.backup().exists());

        let bare = Home::new(None);
        fs::remove_file(&bare.system).unwrap();
        bare.install().unwrap();
        assert!(
            fs::read_to_string(bare.uca())
                .unwrap()
                .contains("print-pictures-1")
        );
    }

    #[test]
    fn a_file_it_does_not_understand_is_left_alone() {
        assert!(add_action("<actions/>", exe()).is_err());
        let home = Home::new(Some("<broken>"));
        assert!(home.install().is_err());
        assert_eq!(fs::read_to_string(home.uca()).unwrap(), "<broken>");
        assert!(!home.at(BINARY).exists() && !home.at(ENTRY).exists() && !home.backup().exists());
    }

    #[test]
    fn installing_from_the_installed_copy_keeps_it_whole() {
        let home = Home::new(Some(TWO_ACTIONS));
        home.install().unwrap();
        let installed = home.at(BINARY);
        install(&home.folders(), &installed, &home.system).unwrap();
        assert_eq!(fs::read_to_string(installed).unwrap(), "the program");
    }

    #[test]
    fn folders_follow_xdg() {
        let os = |s: &str| Some(OsString::from(s));
        let home = Path::new("/home/u");
        let set = Folders::new(home, os("/x/c"), None);
        assert_eq!(set.config, Path::new("/x/c"));
        assert_eq!(set.data, Path::new("/home/u/.local/share"));
        assert_eq!(set.bin, Path::new("/home/u/.local/bin"));
        assert_eq!(Folders::new(home, None, os("/x/d")).data, Path::new("/x/d"));
        for ignored in ["rel", ""] {
            let folders = Folders::new(home, os(ignored), os(ignored));
            assert_eq!(folders.config, Path::new("/home/u/.config"));
            assert_eq!(folders.data, Path::new("/home/u/.local/share"));
        }
    }

    #[test]
    fn thunar_alone() {
        let home = Home::new(Some(TWO_ACTIONS));
        let config = home.folders().config;
        assert!(!has_thunar(&config));
        add_thunar(&config, exe(), &home.system).unwrap();
        assert!(has_thunar(&config));
        assert!(!home.at(BINARY).exists() && !home.at(ENTRY).exists());
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), TWO_ACTIONS);
        let added = fs::read_to_string(home.uca()).unwrap();
        add_thunar(&config, exe(), &home.system).unwrap();
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), TWO_ACTIONS);
        remove_thunar(&config).unwrap();
        assert!(!has_thunar(&config));
        assert_eq!(fs::read_to_string(home.uca()).unwrap(), TWO_ACTIONS);
        assert_eq!(fs::read_to_string(home.backup()).unwrap(), added);
        remove_thunar(&Home::new(None).folders().config).unwrap();
    }

    #[test]
    fn a_packaged_program_is_not_copied() {
        let home = Home::new(Some(TWO_ACTIONS));
        // Not read, because it is not copied.
        let packaged = Path::new("/usr/bin/print-pictures");
        assert!(is_system_install(packaged) && !is_system_install(&home.program));
        install(&home.folders(), packaged, &home.system).unwrap();
        let text = fs::read_to_string(home.uca()).unwrap();
        assert!(text.contains("<command>\"/usr/bin/print-pictures\" %F</command>"));
        assert!(!home.at(BINARY).exists() && !home.at(ENTRY).exists());

        // A copy from an earlier source build is not this program's to delete.
        fs::create_dir_all(home.at(BINARY).parent().unwrap()).unwrap();
        fs::write(home.at(BINARY), "older").unwrap();
        uninstall(&home.folders(), packaged).unwrap();
        assert_eq!(fs::read_to_string(home.uca()).unwrap(), TWO_ACTIONS);
        assert!(home.at(BINARY).exists());
    }

    #[test]
    fn a_linked_config_stays_a_link() {
        let home = Home::new(None);
        let elsewhere = home.dir.path().join("dotfiles-uca.xml");
        fs::write(&elsewhere, TWO_ACTIONS).unwrap();
        fs::create_dir_all(home.uca().parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&elsewhere, home.uca()).unwrap();
        home.install().unwrap();
        assert!(home.uca().is_symlink());
        assert!(
            fs::read_to_string(elsewhere)
                .unwrap()
                .contains("print-pictures-1")
        );
    }
}
