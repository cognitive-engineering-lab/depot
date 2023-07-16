use anyhow::{Context, Result};

use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

pub fn create_dir_if_missing(p: impl AsRef<Path>) -> Result<()> {
  let p = p.as_ref();
  if p.exists() {
    return Ok(());
  }
  fs::create_dir_all(p).with_context(|| format!("Could not create directory: {}", p.display()))
}

pub fn get_git_root(cwd: &Path) -> Option<PathBuf> {
  let mut cmd = Command::new("git");
  cmd.args(["rev-parse", "--show-toplevel"]).current_dir(cwd);
  let output = cmd.output().ok()?;
  output
    .status
    .success()
    .then(|| PathBuf::from(String::from_utf8(output.stdout).unwrap().trim()))
}

pub fn remove_dir_all_if_exists(dir: impl AsRef<Path>) -> Result<()> {
  let dir = dir.as_ref();
  if !dir.exists() {
    return Ok(());
  }
  fs::remove_dir_all(dir).with_context(|| format!("Could not remove dir: {}", dir.display()))
}

#[macro_export]
macro_rules! packages {
  ($($manifest:tt),*) => {{
    use $crate::workspace::package::{Package, PackageManifest, Target, Platform, PackageDepotConfig};
    let index = std::cell::Cell::new(0);
    [$({
      let mut manifest: package_json_schema::PackageJson =
        serde_json::from_value(serde_json::json!($manifest)).expect("Manifest failed to parse");
      let other = manifest.other.as_mut().unwrap();
      if !other.contains_key("depot") {
        other.insert(String::from("depot"), serde_json::to_value(PackageDepotConfig {
          platform: Platform::Browser
        }).unwrap());
      }
      let manifest = PackageManifest::from_json(manifest, std::path::Path::new("dummy.rs")).expect("Manifest failed to convert to Depot format");
      let pkg = Package::from_parts("dummy.rs".into(), manifest, index.get(), "dummy".into(), Target::Lib).expect("Package failed to build");
      index.set(index.get() + 1);
      pkg
    }),*]
  }};
}

#[macro_export]
macro_rules! shareable {
  ($name:ident, $inner:ty) => {
    #[derive(Clone)]
    pub struct $name(std::sync::Arc<$inner>);

    impl std::ops::Deref for $name {
      type Target = $inner;

      fn deref(&self) -> &Self::Target {
        &self.0
      }
    }

    impl std::hash::Hash for $name {
      fn hash<H: std::hash::Hasher>(&self, hasher: &mut H) {
        std::ptr::hash(&*self.0, hasher)
      }
    }

    impl PartialEq for $name {
      fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(&*self.0, &*other.0)
      }
    }

    impl Eq for $name {}

    impl $name {
      pub fn new(inner: $inner) -> Self {
        $name(Arc::new(inner))
      }
    }
  };
}
