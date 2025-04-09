use anyhow::Result;
use log::warn;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, BufWriter},
    path::{Path, PathBuf},
    time::SystemTime,
};

use serde::{Deserialize, Serialize};

use crate::utils;

/// Data structure for tracking when Depot commands were last executed.
#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Fingerprints {
    fingerprints: HashMap<String, SystemTime>,
}

impl Fingerprints {
    pub fn new() -> Self {
        Fingerprints {
            fingerprints: HashMap::new(),
        }
    }

    /// Returns true if there is a recorded timestamp for `key`, and that timestamp is
    /// later than the modified time for all `files`.
    pub fn can_skip(&self, key: &str, files: impl IntoIterator<Item = PathBuf>) -> bool {
        match self.fingerprints.get(key) {
            None => false,
            Some(stored_time) => files
                .into_iter()
                .map(|path| fs::metadata(path)?.modified())
                .filter_map(|res| match res {
                    Ok(time) => Some(time),
                    Err(e) => {
                        warn!("Could not test for staleness: {e}");
                        None
                    }
                })
                .all(|time| time <= *stored_time),
        }
    }

    /// Sets the timestamp for `key` to the current time.
    pub fn update_time(&mut self, key: String) {
        self.fingerprints.insert(key, SystemTime::now());
    }

    fn file_path(root: &Path) -> PathBuf {
        root.join("node_modules").join(".depot-fingerprints.json")
    }

    pub fn load(root: &Path) -> Result<Self> {
        let path = Self::file_path(root);
        if path.exists() {
            let f = File::open(path)?;
            let reader = BufReader::new(f);
            Ok(serde_json::from_reader(reader)?)
        } else {
            Ok(Fingerprints::new())
        }
    }

    pub fn save(&self, root: &Path) -> Result<()> {
        let path = Self::file_path(root);
        utils::create_dir_if_missing(path.parent().unwrap())?;
        let f = File::create(path)?;
        let writer = BufWriter::new(f);
        serde_json::to_writer(writer, &self)?;
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[ignore = "Flaky or system-dependent test, not passing in CI"]
    fn fingerprints() -> Result<()> {
        let dir = TempDir::new()?;
        let dir = dir.path();
        assert!(Fingerprints::load(dir)? == Fingerprints::new());

        let file = dir.join("file.txt");
        fs::write(&file, "Hello")?;

        let mut fingerprints = Fingerprints::new();
        assert!(!fingerprints.can_skip("file.txt", vec![file.clone()]));

        fingerprints.update_time("file.txt".into());
        assert!(fingerprints.can_skip("file.txt", vec![file.clone()]));

        fs::write(&file, "World")?;
        assert!(!fingerprints.can_skip("file.txt", vec![file.clone()]));

        fingerprints.save(dir)?;
        assert!(Fingerprints::load(dir)? == fingerprints);

        Ok(())
    }
}
