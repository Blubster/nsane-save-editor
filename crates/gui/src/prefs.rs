//! Settings kept between runs, in eframe's storage: a file in the user's data
//! folder on the desktop, the browser's local storage on the web.

#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Missing fields take their default, so that settings written by an older
/// version still load.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    /// The theme last picked in the app; `None` follows the system.
    pub dark: Option<bool>,
    /// On a narrow window, whether each side panel is shown.
    pub show_overview: bool,
    pub show_details: bool,
    /// Save without showing the review of the changes first.
    pub skip_save_review: bool,
    /// The folder of the last file opened or saved.
    #[cfg(not(target_arch = "wasm32"))]
    pub last_dir: Option<PathBuf>,
}

impl Prefs {
    /// The stored settings, or the defaults when there are none or they
    /// can't be read.
    pub fn load(storage: Option<&dyn eframe::Storage>) -> Self {
        storage
            .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
            .unwrap_or_default()
    }

    pub fn store(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, self);
    }
}

#[cfg(test)]
pub mod tests {
    use std::collections::HashMap;

    use eframe::Storage as _;

    use super::*;

    /// Storage in memory, standing in for the file or the browser's.
    #[derive(Default)]
    pub struct MemoryStorage(pub HashMap<String, String>);

    impl eframe::Storage for MemoryStorage {
        fn get_string(&self, key: &str) -> Option<String> {
            self.0.get(key).cloned()
        }

        fn set_string(&mut self, key: &str, value: String) {
            self.0.insert(key.to_owned(), value);
        }

        fn remove_string(&mut self, key: &str) {
            self.0.remove(key);
        }

        fn flush(&mut self) {}
    }

    #[test]
    fn round_trips() {
        let prefs = Prefs {
            dark: Some(false),
            show_overview: true,
            show_details: false,
            skip_save_review: true,
            last_dir: Some(PathBuf::from("/home/me/saves")),
        };
        let mut storage = MemoryStorage::default();
        prefs.store(&mut storage);
        assert_eq!(Prefs::load(Some(&storage)), prefs);
    }

    #[test]
    fn missing_or_unreadable_settings_give_the_defaults() {
        assert_eq!(Prefs::load(None), Prefs::default());
        let mut storage = MemoryStorage::default();
        assert_eq!(Prefs::load(Some(&storage)), Prefs::default());
        storage.set_string(eframe::APP_KEY, "not ron at all {".to_owned());
        assert_eq!(Prefs::load(Some(&storage)), Prefs::default());
        // written by a version that knew fewer settings
        storage.set_string(eframe::APP_KEY, "(dark: Some(true))".to_owned());
        let prefs = Prefs::load(Some(&storage));
        assert_eq!(prefs.dark, Some(true));
        assert!(!prefs.show_overview && !prefs.skip_save_review);
    }
}
