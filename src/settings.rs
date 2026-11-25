//! Choices remembered between runs.

use crate::layout::LayoutId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrinterPrefs {
    pub paper: Option<String>,
    pub options: BTreeMap<String, String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub printer: Option<String>,
    pub layout: LayoutId,
    pub fit_to_frame: bool,
    pub sharpen: bool,
    /// Where a PDF was last saved.
    pub pdf_folder: Option<PathBuf>,
    // Last, because TOML writes tables after plain values.
    pub printers: BTreeMap<String, PrinterPrefs>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            printer: None,
            layout: LayoutId::default(),
            fit_to_frame: true,
            sharpen: true,
            pdf_folder: None,
            printers: BTreeMap::new(),
        }
    }
}

impl Settings {
    /// A missing or unreadable file gives the defaults.
    pub fn load_from(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|text| toml::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        let dir = path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string(self).map_err(io::Error::other)?;
        // Replaced in one step, because two windows may save at once.
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        file.write_all(text.as_bytes())?;
        file.persist(path).map_err(|e| e.error)?;
        Ok(())
    }

    pub fn default_path() -> PathBuf {
        config_file(
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("HOME"),
        )
    }
}

fn config_file(xdg: Option<OsString>, home: Option<OsString>) -> PathBuf {
    xdg.map(PathBuf::from)
        .filter(|dir| dir.is_absolute())
        .unwrap_or_else(|| PathBuf::from(home.unwrap_or_default()).join(".config"))
        .join("print-pictures/settings.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deeper/settings.toml");
        let mut settings = Settings {
            printer: Some("Büro \"2\"".into()),
            layout: LayoutId::Wallet,
            fit_to_frame: false,
            sharpen: false,
            pdf_folder: Some("/home/u/Documents".into()),
            printers: BTreeMap::new(),
        };
        settings.printers.insert(
            "Büro \"2\"".into(),
            PrinterPrefs {
                paper: Some("4x6.Borderless".into()),
                options: [("MediaType".into(), "Glossy".into())].into(),
            },
        );
        settings.save_to(&path).unwrap();
        assert_eq!(Settings::load_from(&path), settings);
    }

    #[test]
    fn a_missing_or_corrupt_file_gives_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.toml");
        let defaults = Settings::load_from(&path);
        assert_eq!(defaults, Settings::default());
        assert!(defaults.fit_to_frame && defaults.sharpen);
        assert_eq!(defaults.layout, LayoutId::FullPage);

        std::fs::write(&path, "layout = [[[ not toml").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
    }

    #[test]
    fn the_file_lives_in_the_config_directory() {
        let os = |s: &str| Some(std::ffi::OsString::from(s));
        let expected = |base: &str| PathBuf::from(base).join("print-pictures/settings.toml");
        assert_eq!(config_file(os("/x/cfg"), os("/home/u")), expected("/x/cfg"));
        assert_eq!(
            config_file(None, os("/home/u")),
            expected("/home/u/.config")
        );
        // An empty or relative XDG_CONFIG_HOME is to be ignored.
        assert_eq!(
            config_file(os(""), os("/home/u")),
            expected("/home/u/.config")
        );
    }
}
