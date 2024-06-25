use anyhow::{bail, ensure, Context, Error, Result};

use ignore::Walk;
use maplit::hashset;
use package_json_schema::PackageJson;
use std::{
  fmt::{self, Debug},
  fs,
  hash::Hash,
  path::{Path, PathBuf},
  str::FromStr,
  sync::{Arc, OnceLock, RwLock, RwLockReadGuard},
};

use crate::{shareable, workspace::process::Process};

use super::{dep_graph::DepGraph, Workspace};

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

#[derive(Copy, Clone, clap::ValueEnum, serde::Serialize, serde::Deserialize)]
pub enum Target {
  #[serde(rename = "lib")]
  Lib,
  #[serde(rename = "site")]
  Site,
  #[serde(rename = "script")]
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

impl fmt::Display for PackageName {
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
#[serde(rename_all = "kebab-case")]
pub struct PackageDepotConfig {
  pub platform: Platform,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub target: Option<Target>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub no_server: Option<bool>,
}

pub struct PackageManifest {
  pub manifest: PackageJson,
  pub config: PackageDepotConfig,
}

impl PackageManifest {
  pub fn load(path: &Path) -> Result<Self> {
    let contents = fs::read_to_string(path)
      .with_context(|| format!("Package does not have manifest at: `{}`", path.display()))?;
    let manifest = PackageJson::try_from(contents)?;
    Self::from_json(manifest, path)
  }

  pub fn from_json(mut manifest: PackageJson, path: &Path) -> Result<Self> {
    let error_msg = || format!("Missing \"depot\" key from manifest: `{}`", path.display());
    let other = manifest.other.as_mut().with_context(error_msg)?;
    let config_value = other.remove("depot").with_context(error_msg)?;
    let config: PackageDepotConfig = serde_json::from_value(config_value)?;
    Ok(PackageManifest { manifest, config })
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
  pub index: PackageIndex,

  // Internals
  ws: OnceLock<Workspace>,
  processes: RwLock<Vec<Arc<Process>>>,
}

shareable!(Package, PackageInner);

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
    target: Target,
  ) -> Result<Self> {
    let platform = manifest.config.platform;
    let name_str = manifest
      .manifest
      .name
      .as_deref()
      .unwrap_or_else(|| root.file_name().unwrap().to_str().unwrap());
    let name = PackageName::from_str(name_str)?;

    Ok(Package::new(PackageInner {
      root,
      manifest,
      target,
      platform,
      name,
      index,
      ws: OnceLock::default(),
      processes: RwLock::default(),
    }))
  }

  fn infer_target(root: &Path, manifest: &PackageManifest) -> Result<Target> {
    if let Some(target) = manifest.config.target {
      Ok(target)
    } else if Self::find_source_file(root, "lib").is_some() {
      Ok(Target::Lib)
    } else if Self::find_source_file(root, "main").is_some() {
      Ok(Target::Script)
    } else if Self::find_source_file(root, "index").is_some() {
      Ok(Target::Site)
    } else {
      bail!(
        "Could not infer target. Consider adding a \"target\" entry under \"depot\" to: {}",
        root.join("package.json").display()
      )
    }
  }

  pub fn load(root: &Path, index: PackageIndex) -> Result<Self> {
    let root = root
      .canonicalize()
      .with_context(|| format!("Could not find package root: `{}`", root.display()))?;
    let manifest_path = root.join("package.json");
    let manifest = PackageManifest::load(&manifest_path)?;
    let target = Self::infer_target(&root, &manifest)?;
    Self::from_parts(root, manifest, index, target)
  }

  pub fn start_process(
    &self,
    script: &'static str,
    configure: impl FnOnce(&mut tokio::process::Command),
  ) -> Result<Arc<Process>> {
    let process = self.workspace().start_process(script, |cmd| {
      cmd.current_dir(&self.root);
      configure(cmd);
    })?;
    self.processes.write().unwrap().push(process.clone());
    Ok(process)
  }

  pub async fn exec(
    &self,
    script: &'static str,
    configure: impl FnOnce(&mut tokio::process::Command),
  ) -> Result<()> {
    self
      .start_process(script, configure)?
      .wait_for_success()
      .await
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

  fn iter_files(&self, rel_path: impl AsRef<Path>) -> impl Iterator<Item = PathBuf> {
    Walk::new(self.root.join(rel_path)).filter_map(|entry| {
      let entry = entry.ok()?;
      let is_file = match entry.file_type() {
        Some(file_type) => file_type.is_file(),
        None => false,
      };
      is_file.then(|| entry.into_path())
    })
  }

  pub fn asset_files(&self) -> impl Iterator<Item = PathBuf> {
    // TODO: make this configurable
    let asset_extensions = hashset! { "scss", "css", "jpeg", "jpg", "png", "svg" };

    self.iter_files("src").filter_map(move |path| {
      let ext = path.extension()?;
      asset_extensions
        .contains(ext.to_str().unwrap())
        .then_some(path)
    })
  }

  pub fn source_files(&self) -> impl Iterator<Item = PathBuf> + '_ {
    // TODO: make this configurable
    let source_extensions = hashset! { "ts", "tsx", "html" };

    ["src", "tests"]
      .into_iter()
      .flat_map(|dir| self.iter_files(dir))
      .filter_map(move |path| {
        let ext = path.extension()?;
        source_extensions
          .contains(ext.to_str().unwrap())
          .then_some(path)
      })
  }

  pub fn all_files(&self) -> impl Iterator<Item = PathBuf> + '_ {
    self.iter_files("src")
  }
}

impl Debug for Package {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.name)
  }
}

pub type PackageGraph = DepGraph<Package>;

pub fn build_package_graph(packages: &[Package], roots: &[Package]) -> Result<PackageGraph> {
  DepGraph::build(
    roots.to_vec(),
    |pkg| pkg.name.to_string(),
    |pkg| {
      pkg
        .all_dependencies()
        .filter_map(|name| packages.iter().find(|other_pkg| other_pkg.name == name))
        .cloned()
        .collect()
    },
  )
}

#[cfg(test)]
mod test {
  use super::*;
  use maplit::hashset;

  use std::collections::HashSet;

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

  #[test]
  fn test_package_graph() {
    let pkgs = crate::test_packages! [
      {"name": "a", "dependencies": {"b": "0.1.0"}},
      {"name": "b", "dependencies": {"c": "0.1.0"}},
      {"name": "c"}
    ];

    let [a, b, c] = &pkgs;

    let dg = build_package_graph(&pkgs, &pkgs).unwrap();
    let deps_for = |p| dg.all_deps_for(p).collect::<HashSet<_>>();
    assert_eq!(deps_for(a), hashset! {b, c});
    assert_eq!(deps_for(b), hashset! {c});
    assert_eq!(deps_for(c), hashset! {});

    let imm_deps_for = |p| dg.immediate_deps_for(p).collect::<HashSet<_>>();
    assert_eq!(imm_deps_for(a), hashset! {b});
    assert_eq!(imm_deps_for(b), hashset! {c});
    assert_eq!(imm_deps_for(c), hashset! {});

    assert!(dg.is_dependent_on(a, b));
    assert!(dg.is_dependent_on(a, c));
    assert!(!dg.is_dependent_on(b, a));
  }
}
