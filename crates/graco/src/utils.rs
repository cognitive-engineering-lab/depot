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
  fs::create_dir(p).with_context(|| format!("Could not create directory: {}", p.display()))
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
  Ok(fs::remove_dir_all(dir)?)
}

#[macro_export]
macro_rules! packages {
  ($($manifest:tt),*) => {{
    use $crate::workspace::package::{Package, PackageManifest, Target, Platform, PackageGracoConfig};
    let index = std::cell::Cell::new(0);
    [$({
      let mut manifest: package_json_schema::PackageJson =
        serde_json::from_value(serde_json::json!($manifest)).expect("Manifest failed to parse");
      let other = manifest.other.as_mut().unwrap();
      if !other.contains_key("graco") {
        other.insert(String::from("graco"), serde_json::to_value(PackageGracoConfig {
          platform: Platform::Browser
        }).unwrap());
      }
      let manifest = PackageManifest::from_json(manifest, std::path::Path::new("dummy.rs")).expect("Manifest failed to convert to Graco format");
      let pkg = Package::from_parts("dummy.rs".into(), manifest, index.get(), "dummy".into(), Target::Lib).expect("Package failed to build");
      index.set(index.get() + 1);
      pkg
    }),*]
  }};
}
