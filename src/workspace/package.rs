use anyhow::{bail, ensure, Context, Error, Result};
use async_process::Stdio;
use futures::{io::BufReader, select, AsyncBufReadExt, AsyncRead, FutureExt, StreamExt};
use once_cell::sync::OnceCell;
use package_json_schema::PackageJson;
use std::{
  fmt::Display,
  fs,
  ops::Deref,
  path::{Path, PathBuf},
  str::FromStr,
  sync::Arc,
};

use super::Workspace;

#[derive(Copy, Clone, clap::ValueEnum, serde::Deserialize)]
pub enum Platform {
  Browser,
  Node,
}

impl Platform {
  pub fn to_esbuild_string(self) -> &'static str {
    match self {
      Platform::Browser => "browser",
      Platform::Node => "node",
    }
  }
}

#[derive(Copy, Clone, clap::ValueEnum)]
pub enum Target {
  Lib,
  Site,
  Script,
}

#[derive(Clone, PartialEq, Eq, Hash)]
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
        let components = rest.split('/').collect::<Vec<_>>();
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
  pub manifest: PackageJson,
  pub config: GracoConfig,
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
  pub root: PathBuf,
  pub manifest: Manifest,
  pub platform: Platform,
  pub target: Target,
  pub name: PackageName,
  pub entry_point: PathBuf,
  pub index: PackageIndex,
  ws: OnceCell<Workspace>,
}

impl Package {
  fn find_source_file(root: &Path, base: &str) -> Option<PathBuf> {
    ["tsx", "ts", "js"]
      .into_iter()
      .map(|ext| root.join("src").join(format!("{base}.{ext}")))
      .find(|path| path.exists())
  }

  pub fn load(root: &Path, index: PackageIndex) -> Result<Self> {
    let root = root.canonicalize()?;
    let manifest_path = root.join("package.json");
    let manifest_str = fs::read_to_string(manifest_path)
      .with_context(|| format!("Package does not have package.json: {}", root.display()))?;
    let manifest = Manifest::load(manifest_str)?;

    let (entry_point, target) = if let Some(entry_point) = Package::find_source_file(&root, "lib") {
      (entry_point, Target::Lib)
    } else if let Some(entry_point) = Package::find_source_file(&root, "main") {
      (entry_point, Target::Script)
    } else if let Some(entry_point) = Package::find_source_file(&root, "index") {
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
    })))
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

  async fn pipe_stdio(&self, stdio: impl AsyncRead + Unpin, script_name: &str) {
    let mut lines = BufReader::new(stdio).lines();
    while let Some(line) = lines.next().await {
      let line = line.unwrap();
      let mut logger = self.workspace().logger.lock().unwrap();
      let logger = logger.logger(self.index, script_name);
      let line = match line.strip_prefix("\u{1b}c") {
        Some(rest) => {
          logger.clear();
          rest.to_string()
        }
        None => line,
      };
      logger.push(line);
    }
  }

  pub async fn exec(
    &self,
    script: &str,
    configure: impl FnOnce(&mut async_process::Command),
  ) -> Result<()> {
    let ws = self.workspace();
    let script_path = ws.global_config.bindir().join(script);
    assert!(script_path.exists());

    let mut cmd = async_process::Command::new(&script_path);
    cmd.current_dir(&self.root);

    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    configure(&mut cmd);

    let mut child = cmd
      .spawn()
      .with_context(|| format!("Failed to spawn process: `{}`", script_path.display()))?;

    ws.logger.lock().unwrap().register_log(self.index, script);

    let stdout_future = self.pipe_stdio(child.stdout.take().unwrap(), script);
    let stderr_future = self.pipe_stdio(child.stderr.take().unwrap(), script);
    let process_future = child.status();

    select! {
      status = process_future.fuse() => { status?; },
      _ = stdout_future.fuse() => {},
      _ = stderr_future.fuse() => {}
    };

    Ok(())
  }
}
