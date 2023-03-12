use anyhow::{bail, ensure, Context, Error, Result};
use package_json_schema::PackageJson;
use std::{
  fmt::Display,
  fs,
  path::{Path, PathBuf},
  str::FromStr,
};

#[derive(Copy, Clone, clap::ValueEnum, serde::Deserialize)]
pub enum Platform {
  Browser,
  Node,
}

#[derive(Copy, Clone, clap::ValueEnum)]
pub enum Target {
  Bin,
  Lib,
  Site,
}

#[derive(Clone)]
pub struct PackageName {
  pub name: String,
  pub scope: Option<String>,
}

impl Display for PackageName {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match &self.scope {
      Some(scope) => write!(f, "@{}/{}", scope, self.name),
      None => write!(f, "{}", self.name),
    }
  }
}

impl FromStr for PackageName {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self> {
    match s.strip_prefix('@') {
      Some(rest) => {
        let components = rest.split("/").collect::<Vec<_>>();
        ensure!(components.len() == 2, "Invalid package name");

        Ok(PackageName {
          name: components[0].to_string(),
          scope: Some(components[1].to_string()),
        })
      }
      None => Ok(PackageName {
        name: s.to_string(),
        scope: None,
      }),
    }
  }
}

#[derive(Default, serde::Deserialize)]
pub struct GracoConfig {
  platform: Option<Platform>,
}

pub struct Manifest {
  manifest: PackageJson,
  config: GracoConfig,
}

impl Manifest {
  pub fn load(contents: String) -> Result<Self> {
    let mut manifest = PackageJson::try_from(contents)?;
    let config = match &mut manifest.other {
      Some(other) => match other.remove("graco") {
        Some(value) => serde_json::from_value(value)?,
        None => GracoConfig::default(),
      },
      None => GracoConfig::default(),
    };
    Ok(Manifest { manifest, config })
  }
}
pub struct Package {
  pub manifest: Manifest,
  pub platform: Platform,
  pub target: Target,
  pub name: PackageName,
  pub entry_point: PathBuf,
}

impl Package {
  fn find_source_file(root: &Path, base: &str) -> Option<PathBuf> {
    ["tsx", "ts", "js"]
      .into_iter()
      .map(|ext| root.join("src").join(format!("{base}.{ext}")))
      .find(|path| path.exists())
  }

  pub fn load(root: &Path) -> Result<Self> {
    let manifest_path = root.join("package.json");
    let manifest_str = fs::read_to_string(&manifest_path)
      .with_context(|| format!("Package does not have package.json: {}", root.display()))?;
    let manifest = Manifest::load(manifest_str)?;

    let (entry_point, target) = if let Some(entry_point) = Package::find_source_file(root, "lib") {
      (entry_point, Target::Lib)
    } else if let Some(entry_point) = Package::find_source_file(root, "main") {
      (entry_point, Target::Bin)
    } else if let Some(entry_point) = Package::find_source_file(root, "index") {
      (entry_point, Target::Site)
    } else {
      bail!(
        "Could not find entry point to package in directory: {}",
        root.display()
      )
    };

    let platform = manifest.config.platform.unwrap_or(Platform::Browser);
    let name_str = manifest
      .manifest
      .name
      .as_ref()
      .map(|s| s.as_str())
      .unwrap_or_else(|| root.file_name().unwrap().to_str().unwrap());
    let name = PackageName::from_str(name_str)?;

    Ok(Package {
      manifest,
      entry_point,
      target,
      platform,
      name,
    })
  }
}
