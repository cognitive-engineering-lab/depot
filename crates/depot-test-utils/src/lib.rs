#![allow(clippy::new_without_default)]

use anyhow::{Result, ensure};
use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use either::Either;
use tempfile::TempDir;

pub struct ProjectBuilder {
  tmpdir: Either<TempDir, PathBuf>,
}

#[allow(unused)]
pub struct CommandOutput {
  stdout: String,
  stderr: String,
}

fn new_cmd(s: impl AsRef<str>) -> String {
  format!("{} --prefer-offline", s.as_ref())
}

impl ProjectBuilder {
  pub fn new() -> Self {
    let tmpdir = TempDir::new().unwrap();
    ProjectBuilder {
      tmpdir: Either::Left(tmpdir),
    }
  }

  pub fn persist(mut self) -> Self {
    eprintln!("Persisted: {}", self.root().display());
    self.tmpdir = Either::Right(self.tmpdir.unwrap_left().keep());
    self
  }

  pub fn root(&self) -> PathBuf {
    let tmpdir = match &self.tmpdir {
      Either::Left(tmpdir) => tmpdir.path(),
      Either::Right(path) => path,
    };
    tmpdir.join("foo")
  }

  pub fn file(&self, path: impl AsRef<Path>, body: impl AsRef<str>) -> &Self {
    let (path, body) = (path.as_ref(), body.as_ref());
    fs::create_dir_all(self.root().join(path.parent().unwrap())).unwrap();
    fs::write(self.root().join(path), body).unwrap();
    self
  }

  pub fn maybe_depot_in(
    &self,
    cmd: impl AsRef<str>,
    dir: impl AsRef<Path>,
  ) -> Result<CommandOutput> {
    let mut process = Command::new(depot_exe());
    process.current_dir(dir);
    process.args(shlex::split(cmd.as_ref()).unwrap());

    let output = process.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    ensure!(
      output.status.success(),
      "process failed:\n{stdout}\n{stderr}"
    );

    Ok(CommandOutput { stdout, stderr })
  }

  pub fn depot_in(&self, cmd: impl AsRef<str>, dir: impl AsRef<Path>) -> CommandOutput {
    self.maybe_depot_in(cmd, dir).unwrap()
  }

  pub fn maybe_depot(&self, cmd: impl AsRef<str>) -> Result<CommandOutput> {
    self.maybe_depot_in(cmd, self.root())
  }

  pub fn depot(&self, cmd: impl AsRef<str>) -> CommandOutput {
    self.maybe_depot(cmd).unwrap()
  }

  pub fn read(&self, path: impl AsRef<Path>) -> String {
    fs::read_to_string(self.root().join(path)).unwrap()
  }

  pub fn exists(&self, path: impl AsRef<Path>) -> bool {
    self.root().join(path).exists()
  }
}

pub fn project() -> ProjectBuilder {
  project_for("lib", "browser")
}

pub fn custom_project_for(target: &str, platform: &str, flags: &str) -> ProjectBuilder {
  let builder = ProjectBuilder::new();
  builder.depot_in(
    new_cmd(format!(
      "new foo --target {target} --platform {platform} {flags}"
    )),
    builder.root().parent().unwrap(),
  );
  builder
}

pub fn project_for(target: &str, platform: &str) -> ProjectBuilder {
  custom_project_for(target, platform, "")
}

pub fn workspace() -> ProjectBuilder {
  let builder = ProjectBuilder::new();
  builder.depot_in(
    new_cmd("new foo --workspace"),
    builder.root().parent().unwrap(),
  );
  builder
}

pub fn workspace_single_lib() -> ProjectBuilder {
  let ws = workspace();
  ws.depot(new_cmd("new bar"));
  ws
}

pub fn depot_exe() -> PathBuf {
  snapbox::cmd::cargo_bin("depot")
}
