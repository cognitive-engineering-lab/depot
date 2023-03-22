use anyhow::{Context, Result};
use cfg_if::cfg_if;
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
    .then(|| PathBuf::from(String::from_utf8(output.stdout).unwrap()))
}

pub fn symlink_dir_if_missing(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
  let (src, dst) = (src.as_ref(), dst.as_ref());
  if dst.exists() {
    return Ok(());
  }
  cfg_if! {
    if #[cfg(unix)] {
      let result = std::os::unix::fs::symlink(src, dst);
    } else {
      let result = std::os::windows::fs::symlink_dir(src, dst);
    }
  };
  result.with_context(|| {
    format!(
      "Could not create symlink from {} to {}",
      src.display(),
      dst.display()
    )
  })
}

pub fn remove_dir_all_if_exists(dir: impl AsRef<Path>) -> Result<()> {
  let dir = dir.as_ref();
  if !dir.exists() {
    return Ok(());
  }
  Ok(fs::remove_dir_all(dir)?)
}
