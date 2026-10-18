//! Recently opened PaperCraft and legacy `.crumbs` files shown on the dashboard.
//!
//! The list is a small local JSON file. It stores paths only; documents stay where the
//! user saved them. A missing or invalid list is treated as empty and never blocks editing.

use crate::document::atomic_write;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

pub const MAX_RECENTS: usize = 24;
const MAX_STORE_BYTES: u64 = 256 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    version: u32,
    files: Vec<PathBuf>,
}

#[derive(Debug, Default)]
pub struct Recents {
    files: Vec<PathBuf>,
    store: Option<PathBuf>,
}

impl Recents {
    /// A list that is never written to disk.
    pub fn in_memory() -> Self {
        Self::default()
    }

    /// Loads the list stored at `store`. Unreadable or invalid lists become empty.
    pub fn load(store: PathBuf) -> Self {
        let mut data = Vec::new();
        let stored = std::fs::File::open(&store)
            .and_then(|f| f.take(MAX_STORE_BYTES + 1).read_to_end(&mut data))
            .ok()
            .filter(|&n| (n as u64) <= MAX_STORE_BYTES)
            .and_then(|_| serde_json::from_slice::<Stored>(&data).ok())
            .filter(|s| s.version == 1);
        let mut recents = Self {
            files: Vec::new(),
            store: Some(store),
        };
        for file in stored.map(|s| s.files).unwrap_or_default() {
            if file.is_absolute()
                && !recents.files.contains(&file)
                && recents.files.len() < MAX_RECENTS
            {
                recents.files.push(file);
            }
        }
        recents
    }

    /// Legacy platform location, retained so both application versions share recent files.
    pub fn default_store() -> Option<PathBuf> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        if cfg!(target_os = "macos") {
            return home.map(|h| h.join("Library/Application Support/Crumbs/recents.json"));
        }
        if cfg!(target_os = "windows") {
            return std::env::var_os("APPDATA")
                .map(|d| PathBuf::from(d).join("Crumbs").join("recents.json"));
        }
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .or_else(|| home.map(|h| h.join(".config")))
            .map(|d| d.join("crumbs").join("recents.json"))
    }

    pub fn files(&self) -> &[PathBuf] {
        &self.files
    }

    /// Moves `path` to the front of the list and persists it.
    pub fn touch(&mut self, path: &Path) -> Result<(), String> {
        let path = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
        self.files.retain(|p| p != &path);
        self.files.insert(0, path);
        self.files.truncate(MAX_RECENTS);
        self.persist()
    }

    pub fn remove(&mut self, path: &Path) -> Result<(), String> {
        let count = self.files.len();
        self.files.retain(|p| p != path);
        if count == self.files.len() {
            return Ok(());
        }
        self.persist()
    }

    fn persist(&self) -> Result<(), String> {
        let Some(store) = &self.store else {
            return Ok(());
        };
        if let Some(parent) = store.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Recent files could not be saved: {e}"))?;
        }
        let data = serde_json::to_vec_pretty(&Stored {
            version: 1,
            files: self.files.clone(),
        })
        .map_err(|e| format!("Recent files could not be saved: {e}"))?;
        atomic_write(store, &data).map_err(|e| format!("Recent files could not be saved. {e}"))
    }
}

/// Dashboard subtitle for a file modified at `modified`, relative to `now`.
pub fn edited_label(modified: SystemTime, now: SystemTime) -> String {
    let seconds = now
        .duration_since(modified)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let unit = |n: u64, name: &str| {
        if n == 1 {
            format!("Edited 1 {name} ago")
        } else {
            format!("Edited {n} {name}s ago")
        }
    };
    match seconds {
        0..60 => "Edited just now".into(),
        60..3_600 => unit(seconds / 60, "minute"),
        3_600..86_400 => unit(seconds / 3_600, "hour"),
        86_400..2_592_000 => unit(seconds / 86_400, "day"),
        2_592_000..31_536_000 => unit(seconds / 2_592_000, "month"),
        _ => unit(seconds / 31_536_000, "year"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn recent_files_are_ordered_deduplicated_capped_and_persisted() {
        let directory = tempfile::tempdir().unwrap();
        let store = directory.path().join("config").join("recents.json");
        let mut recents = Recents::load(store.clone());
        assert!(recents.files().is_empty());
        let a = directory.path().join("a.crumbs");
        let b = directory.path().join("b.crumbs");
        recents.touch(&a).unwrap();
        recents.touch(&b).unwrap();
        recents.touch(&a).unwrap();
        assert_eq!(recents.files(), [a.clone(), b.clone()]);
        assert_eq!(Recents::load(store.clone()).files(), [a.clone(), b.clone()]);
        recents.remove(&a).unwrap();
        assert_eq!(
            Recents::load(store.clone()).files(),
            std::slice::from_ref(&b)
        );
        for i in 0..MAX_RECENTS + 5 {
            recents
                .touch(&directory.path().join(format!("{i}.crumbs")))
                .unwrap();
        }
        assert_eq!(recents.files().len(), MAX_RECENTS);
        assert_eq!(
            recents.files()[0],
            directory.path().join(format!("{}.crumbs", MAX_RECENTS + 4))
        );
        // Invalid, relative or future-version lists are ignored rather than trusted.
        std::fs::write(&store, b"{not json").unwrap();
        assert!(Recents::load(store.clone()).files().is_empty());
        std::fs::write(
            &store,
            serde_json::to_vec(&serde_json::json!({
                "version": 1, "files": ["relative.crumbs", a]
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            Recents::load(store.clone()).files(),
            std::slice::from_ref(&a)
        );
        std::fs::write(
            &store,
            serde_json::to_vec(&serde_json::json!({"version": 2, "files": [a]})).unwrap(),
        )
        .unwrap();
        assert!(Recents::load(store).files().is_empty());
    }

    #[test]
    fn edited_labels_use_the_largest_whole_unit() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(100_000_000);
        let ago = |s| edited_label(now - Duration::from_secs(s), now);
        assert_eq!(ago(5), "Edited just now");
        assert_eq!(ago(60), "Edited 1 minute ago");
        assert_eq!(ago(3_599), "Edited 59 minutes ago");
        assert_eq!(ago(3_600), "Edited 1 hour ago");
        assert_eq!(ago(22 * 3_600 + 50), "Edited 22 hours ago");
        assert_eq!(ago(2 * 86_400), "Edited 2 days ago");
        assert_eq!(ago(65 * 86_400), "Edited 2 months ago");
        assert_eq!(ago(800 * 86_400), "Edited 2 years ago");
        // A modification time in the future (clock skew) is not shown as negative.
        assert_eq!(
            edited_label(now + Duration::from_secs(90), now),
            "Edited just now"
        );
    }
}
