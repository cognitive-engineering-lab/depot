use anyhow::{bail, ensure, Context, Error, Result};

use once_cell::sync::OnceCell;
use package_json_schema::PackageJson;
use std::{
  fmt::Display,
  fs,
  ops::Deref,
  path::{Path, PathBuf},
  str::FromStr,
  sync::{Arc, RwLock, RwLockReadGuard},
};
use walkdir::WalkDir;

use crate::workspace::process::Process;

use super::Workspace;

#[derive(Copy, Clone, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
pub enum Platform {
  #[serde(rename = "browser")]
  Browser,
  #[serde(rename = "node")]
  Node,
}

#[allow(unused)]
impl Platform {
  pub fn is_browser(self) -> bool {
    matches!(self, Platform::Browser)
  }

  pub fn is_node(self) -> bool {
    matches!(self, Platform::Node)
  }
}

#[derive(Copy, Clone, clap::ValueEnum)]
pub enum Target {
  Lib,
  Site,
  Script,
}

impl Target {
  pub fn is_lib(self) -> bool {
    matches!(self, Target::Lib)
  }

  pub fn is_site(self) -> bool {
    matches!(self, Target::Site)
  }

  pub fn is_script(self) -> bool {
    matches!(self, Target::Script)
  }
}

#[derive(Clone, PartialEq, Eq, Hash, Debug, Ord, PartialOrd)]
pub struct PackageName {
  pub name: String,
  pub scope: Option<String>,
}

impl PackageName {
  pub fn as_global_var(&self) -> String {
    self
      .name
      .split('-')
      .map(|substr| {
        let mut chars = substr.chars();
        let first = chars.next().unwrap().to_uppercase().to_string();
        first + &chars.collect::<String>()
      })
      .collect::<String>()
  }
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
        let components = rest.split('/').collect::<Vec<_>>();
        ensure!(components.len() == 2, "Invalid package name");

        Ok(PackageName {
          scope: Some(components[0].to_string()),
          name: components[1].to_string(),
        })
      }
      None => Ok(PackageName {
        name: s.to_string(),
        scope: None,
      }),
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PackageGracoConfig {
  pub platform: Platform,
}

pub struct PackageManifest {
  pub manifest: PackageJson,
  pub config: PackageGracoConfig,
}

impl PackageManifest {
  pub fn load(path: &Path) -> Result<Self> {
    let contents = fs::read_to_string(path)
      .with_context(|| format!("Package does not have manifest at: `{}`", path.display()))?;
    let manifest = PackageJson::try_from(contents)?;
    Self::from_json(manifest, path)
  }

  pub fn from_json(mut manifest: PackageJson, path: &Path) -> Result<Self> {
    let error_msg = || format!("Missing \"graco\" key from manifest: `{}`", path.display());
    let other = manifest.other.as_mut().with_context(error_msg)?;
    let config_value = other.remove("graco").with_context(error_msg)?;
    let config: PackageGracoConfig = serde_json::from_value(config_value)?;
    Ok(PackageManifest { manifest, config })
  }
}

#[derive(Clone)]
pub struct Package(Arc<PackageInner>);

impl Deref for Package {
  type Target = PackageInner;

  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub type PackageIndex = usize;

pub struct PackageInner {
  // Metadata
  pub root: PathBuf,
  pub manifest: PackageManifest,
  pub platform: Platform,
  pub target: Target,
  pub name: PackageName,
  pub entry_point: PathBuf,
  pub index: PackageIndex,

  // Internals
  ws: OnceCell<Workspace>,
  processes: RwLock<Vec<Arc<Process>>>,
}

impl Package {
  fn find_source_file(root: &Path, base: &str) -> Option<PathBuf> {
    ["tsx", "ts", "js"]
      .into_iter()
      .map(|ext| root.join("src").join(format!("{base}.{ext}")))
      .find(|path| path.exists())
  }

  pub fn processes(&self) -> RwLockReadGuard<'_, Vec<Arc<Process>>> {
    self.processes.read().unwrap()
  }

  pub fn from_parts(
    root: PathBuf,
    manifest: PackageManifest,
    index: PackageIndex,
    entry_point: PathBuf,
    target: Target,
  ) -> Result<Self> {
    let platform = manifest.config.platform;
    let name_str = manifest
      .manifest
      .name
      .as_deref()
      .unwrap_or_else(|| root.file_name().unwrap().to_str().unwrap());
    let name = PackageName::from_str(name_str)?;

    Ok(Package(Arc::new(PackageInner {
      root: root.to_owned(),
      manifest,
      entry_point,
      target,
      platform,
      name,
      index,
      ws: OnceCell::default(),
      processes: Default::default(),
    })))
  }

  pub fn load(root: &Path, index: PackageIndex) -> Result<Self> {
    let root = root
      .canonicalize()
      .with_context(|| format!("Could not find package root: `{}`", root.display()))?;
    let manifest_path = root.join("package.json");
    let manifest = PackageManifest::load(&manifest_path)?;

    let (entry_point, target) = if let Some(entry_point) = Package::find_source_file(&root, "lib") {
      (entry_point, Target::Lib)
    } else if let Some(entry_point) = Package::find_source_file(&root, "main") {
      (entry_point, Target::Script)
    } else if let Some(entry_point) = Package::find_source_file(&root, "index") {
      (entry_point, Target::Site)
    } else {
      bail!(
        "Could not find entry point to package in directory: `{}`",
        root.display()
      )
    };

    Self::from_parts(root, manifest, index, entry_point, target)
  }

  pub async fn exec(
    &self,
    script: &'static str,
    configure: impl FnOnce(&mut async_process::Command),
  ) -> Result<()> {
    let ws = self.workspace();
    let pnpm_path = ws.global_config.bindir().join("pnpm");
    ensure!(
      pnpm_path.exists(),
      "Trying to execute a command before Graco is setup"
    );

    let mut cmd = async_process::Command::new(&pnpm_path);
    cmd.current_dir(&self.root);

    if script != "pnpm" {
      cmd.args(["exec", script]);
    }

    configure(&mut cmd);

    let process = Arc::new(Process::new(script.to_owned(), cmd)?);
    self.processes.write().unwrap().push(process.clone());

    process.wait().await?;

    Ok(())
  }
}

impl PackageInner {
  pub fn all_dependencies(&self) -> impl Iterator<Item = PackageName> + '_ {
    let manifest = &self.manifest.manifest;
    let manifest_deps = [
      &manifest.dependencies,
      &manifest.dev_dependencies,
      &manifest.peer_dependencies,
    ];
    manifest_deps
      .into_iter()
      .flatten()
      .flat_map(|deps| deps.keys())
      .filter_map(|s| PackageName::from_str(s).ok())
  }

  pub fn workspace(&self) -> &Workspace {
    self.ws.get().unwrap()
  }

  pub(super) fn set_workspace(&self, ws: &Workspace) {
    self
      .ws
      .set(ws.clone())
      .unwrap_or_else(|_| panic!("Called set_workspace twice!"));
  }

  pub fn source_files(&self) -> Vec<PathBuf> {
    ["src", "tests"]
      .into_iter()
      .flat_map(|dir| WalkDir::new(self.root.join(dir)))
      .filter_map(|entry| {
        let entry = entry.ok()?;
        if !entry.file_type().is_file() {
          return None;
        }

        let path = entry.path();
        let Some(ext) = path.extension() else {
          return None;
        };
        let is_src_file = ext == "ts" || ext == "tsx";
        is_src_file.then(|| path.to_owned())
      })
      .collect()
  }
}

#[cfg(test)]
mod test {
  use super::*;

  #[test]
  fn test_package_name() {
    let s = "foo";
    let name = PackageName::from_str(s).unwrap();
    assert_eq!(
      name,
      PackageName {
        name: "foo".into(),
        scope: None
      }
    );

    let s = "@foo/bar";
    let name = PackageName::from_str(s).unwrap();
    assert_eq!(
      name,
      PackageName {
        name: "bar".into(),
        scope: Some("foo".into())
      }
    );
    assert_eq!("@foo/bar", format!("{}", name));

    let s = "@what/is/this";
    assert!(PackageName::from_str(s).is_err());
  }
}
