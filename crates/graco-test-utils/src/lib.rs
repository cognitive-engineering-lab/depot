use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
  sync::Once,
};

use either::Either;
use tempfile::TempDir;

pub struct ProjectBuilder {
  tmpdir: Either<TempDir, PathBuf>,
}

pub struct CommandOutput {
  stdout: String,
  stderr: String,
}

static SETUP: Once = Once::new();

impl ProjectBuilder {
  pub fn new(target: &str, platform: &str) -> Self {
    SETUP.call_once(|| {
      let status = Command::new(graco_exe()).arg("setup").status().unwrap();
      if !status.success() {
        panic!("graco setup failed");
      }
    });

    let tmpdir = TempDir::new().unwrap();
    let mut process = Command::new(graco_exe());
    process.current_dir(tmpdir.path());
    process.args(["new", "foo", "-t", target, "-p", platform]);
    let status = process.status().unwrap();
    if !status.success() {
      panic!("graco new failed");
    }

    ProjectBuilder {
      tmpdir: Either::Left(tmpdir),
    }
  }

  pub fn persist(mut self) -> Self {
    println!("Persisted: {}", self.root().display());
    self.tmpdir = Either::Right(self.tmpdir.unwrap_left().into_path());
    self
  }

  pub fn root(&self) -> PathBuf {
    let tmpdir = match &self.tmpdir {
      Either::Left(tmpdir) => tmpdir.path(),
      Either::Right(path) => path,
    };
    tmpdir.join("foo")
  }

  pub fn file(self, path: impl AsRef<Path>, body: impl AsRef<str>) -> Self {
    let (path, body) = (path.as_ref(), body.as_ref());
    fs::create_dir_all(self.root().join(path.parent().unwrap())).unwrap();
    fs::write(self.root().join(path), body).unwrap();
    self
  }

  pub fn graco(&self, cmd: impl AsRef<str>) -> CommandOutput {
    let mut process = Command::new(graco_exe());
    process.current_dir(self.root());
    process.args(shlex::split(cmd.as_ref()).unwrap());

    let output = process.output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    if !output.status.success() {
      panic!("{stderr}");
    }

    CommandOutput { stdout, stderr }
  }

  pub fn exists(&self, path: impl AsRef<Path>) -> bool {
    self.root().join(path).exists()
  }
}

pub fn project() -> ProjectBuilder {
  ProjectBuilder::new("lib", "browser")
}

pub fn project_for(target: &str, platform: &str) -> ProjectBuilder {
  ProjectBuilder::new(target, platform)
}

pub fn graco_exe() -> PathBuf {
  snapbox::cmd::cargo_bin("graco")
}
