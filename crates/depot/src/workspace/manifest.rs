use anyhow::{Context, Result};
use std::{fs, path::Path};

use package_json_schema::PackageJson;
use serde::de::DeserializeOwned;

pub struct DepotManifest<Config> {
    pub manifest: PackageJson,
    pub config: Config,
}

impl<Config: DeserializeOwned> DepotManifest<Config> {
    pub fn load(path: &Path) -> Result<Self> {
        let contents = fs::read_to_string(path)
            .with_context(|| format!("Missing manifest at: `{}`", path.display()))?;
        let manifest = PackageJson::try_from(contents)?;
        Self::from_json(manifest, path)
    }

    pub fn from_json(mut manifest: PackageJson, path: &Path) -> Result<Self> {
        let error_msg = || format!("Missing \"depot\" key from manifest: `{}`", path.display());
        let other = manifest.other.as_mut().with_context(error_msg)?;
        let config_value = other.shift_remove("depot").with_context(error_msg)?;
        let config: Config = serde_json::from_value(config_value)?;
        Ok(DepotManifest { manifest, config })
    }
}
