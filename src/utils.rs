use anyhow::{Context, Result};
use std::{
  fs, io,
  path::{Path, PathBuf},
  process::Command,
};

pub fn create_dir_if_missing(p: impl AsRef<Path>) -> Result<()> {
  let p = p.as_ref();
  match fs::create_dir(p) {
    Ok(()) => Ok(()),
    Err(e) => match e.kind() {
      io::ErrorKind::AlreadyExists => Ok(()),
      _ => Err(e).context(format!("Could not create directory: {}", p.display())),
    },
  }
}

pub fn get_git_root(cwd: &Path) -> Option<PathBuf> {
  let mut cmd = Command::new("git");
  cmd.args(["rev-parse", "--show-toplevel"]).current_dir(cwd);
  Some(PathBuf::from(
    String::from_utf8(cmd.output().ok()?.stdout).unwrap(),
  ))
}
