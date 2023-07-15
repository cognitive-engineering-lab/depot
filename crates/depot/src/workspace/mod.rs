use self::{
  dep_graph::DepGraph,
  package::{PackageGraph, PackageIndex, PackageName},
  process::Process,
};
use crate::{commands::setup::GlobalConfig, shareable, utils, CommonArgs};

use anyhow::{ensure, Context, Result};
use cfg_if::cfg_if;
use futures::{
  stream::{self, TryStreamExt},
  StreamExt,
};
use log::debug;
use package::Package;
use std::{
  cmp::Ordering,
  env,
  fmt::{self, Debug},
  iter,
  ops::Deref,
  path::{Path, PathBuf},
  sync::{Arc, RwLock, RwLockReadGuard},
};

mod dep_graph;
pub mod package;
pub mod process;
mod runner;

#[derive(Clone)]
pub struct Workspace(Arc<WorkspaceInner>);

impl Deref for Workspace {
  type Target = WorkspaceInner;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub struct WorkspaceInner {
  pub root: PathBuf,
  pub packages: Vec<Package>,
  pub monorepo: bool,
  pub global_config: GlobalConfig,
  pub pkg_graph: PackageGraph,
  pub common: CommonArgs,

  package_display_order: Vec<PackageIndex>,
  processes: RwLock<Vec<Arc<Process>>>,
}

fn find_workspace_root(max_ancestor: &Path, cwd: &Path) -> Result<PathBuf> {
  let rel_path_to_cwd = cwd.strip_prefix(max_ancestor).unwrap_or_else(|_| {
    panic!(
      "Internal error: Max ancestor `{}` is not a prefix of cwd `{}`",
      max_ancestor.display(),
      cwd.display()
    )
  });
  let components = rel_path_to_cwd.iter().collect::<Vec<_>>();
  (0..=components.len())
    .map(|i| {
      iter::once(max_ancestor.as_os_str())
        .chain(components[..i].iter().copied())
        .collect::<PathBuf>()
    })
    .find(|path| path.join("package.json").exists())
    .with_context(|| {
      format!(
        "Could not find workspace root in working dir: {}",
        cwd.display()
      )
    })
}

pub enum CommandInner {
  Package(Box<dyn PackageCommand>),
  Workspace(Box<dyn WorkspaceCommand>),
  Both(Box<dyn WorkspaceAndPackageCommand>),
}

impl CommandInner {
  pub fn deps(&self) -> Vec<Command> {
    match self {
      CommandInner::Package(cmd) => cmd.deps(),
      CommandInner::Both(cmd) => cmd.deps(),
      CommandInner::Workspace(_) => Vec::new(),
    }
  }

  pub fn name(&self) -> String {
    match self {
      CommandInner::Package(cmd) => cmd.name(),
      CommandInner::Workspace(cmd) => cmd.name(),
      CommandInner::Both(cmd) => cmd.name(),
    }
  }
}

impl Command {
  pub async fn run_pkg(self, package: Package) -> Result<()> {
    match &*self {
      CommandInner::Package(cmd) => cmd.run_pkg(&package).await,
      CommandInner::Both(cmd) => cmd.run_pkg(&package).await,
      CommandInner::Workspace(_) => panic!("run_pkg on workspace command"),
    }
  }

  pub async fn run_ws(self, ws: Workspace) -> Result<()> {
    match &*self {
      CommandInner::Workspace(cmd) => cmd.run_ws(&ws).await,
      CommandInner::Both(cmd) => cmd.run_ws(&ws).await,
      CommandInner::Package(_) => panic!("run_ws on package command"),
    }
  }
}

impl fmt::Debug for CommandInner {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      CommandInner::Package(cmd) => write!(f, "{cmd:?}"),
      CommandInner::Workspace(cmd) => write!(f, "{cmd:?}"),
      CommandInner::Both(cmd) => write!(f, "{cmd:?}"),
    }
  }
}

shareable!(Command, CommandInner);

impl Command {
  pub fn package(cmd: impl PackageCommand) -> Self {
    Self::new(CommandInner::Package(Box::new(cmd)))
  }

  pub fn workspace(cmd: impl WorkspaceCommand + 'static) -> Self {
    Self::new(CommandInner::Workspace(Box::new(cmd)))
  }

  pub fn both(cmd: impl WorkspaceAndPackageCommand) -> Self {
    Self::new(CommandInner::Both(Box::new(cmd)))
  }
}

pub trait CoreCommand {
  fn name(&self) -> String;
}

#[async_trait::async_trait]
pub trait PackageCommand: CoreCommand + Debug + Send + Sync + 'static {
  async fn run_pkg(&self, package: &Package) -> Result<()>;

  fn deps(&self) -> Vec<Command> {
    Vec::new()
  }

  fn ignore_dependencies(&self) -> bool {
    true
  }
}

#[async_trait::async_trait]
pub trait WorkspaceCommand: CoreCommand + Debug + Send + Sync + 'static {
  async fn run_ws(&self, ws: &Workspace) -> Result<()>;
}

pub trait WorkspaceAndPackageCommand: WorkspaceCommand + PackageCommand {}
impl<T: WorkspaceCommand + PackageCommand> WorkspaceAndPackageCommand for T {}

impl Workspace {
  pub async fn load(
    global_config: GlobalConfig,
    cwd: Option<PathBuf>,
    common: CommonArgs,
  ) -> Result<Self> {
    let cwd = match cwd {
      Some(cwd) => cwd,
      None => env::current_dir()?,
    };

    let fs_root = {
      cfg_if! {
        if #[cfg(unix)] {
          Path::new("/")
        } else {
          todo!()
        }
      }
    };

    let git_root = utils::get_git_root(&cwd);
    let max_ancestor: &Path = git_root.as_deref().unwrap_or(fs_root);
    let root = find_workspace_root(max_ancestor, &cwd)?;
    debug!("Workspace root: `{}`", root.display());

    let pkg_dir = root.join("packages");
    let monorepo = pkg_dir.exists();
    debug!("Workspace is monorepo: {monorepo}");

    let pkg_roots = if monorepo {
      pkg_dir
        .read_dir()?
        .map(|entry| Ok(entry?.path()))
        .collect::<Result<Vec<_>>>()?
    } else {
      vec![root.clone()]
    };

    let packages: Vec<_> = stream::iter(pkg_roots)
      .enumerate()
      .then(|(index, pkg_root)| async move { Package::load(&pkg_root, index) })
      .try_collect()
      .await?;

    let pkg_graph = package::build_package_graph(&packages);

    let package_display_order = {
      let mut order = packages.iter().map(|pkg| pkg.index).collect::<Vec<_>>();

      order.sort_by(|n1, n2| {
        if pkg_graph.is_dependent_on(&packages[*n2], &packages[*n1]) {
          Ordering::Less
        } else if pkg_graph.is_dependent_on(&packages[*n1], &packages[*n2]) {
          Ordering::Greater
        } else {
          Ordering::Equal
        }
      });

      order.sort_by(|n1, n2| {
        if pkg_graph.is_dependent_on(&packages[*n2], &packages[*n1]) {
          Ordering::Less
        } else if pkg_graph.is_dependent_on(&packages[*n1], &packages[*n2]) {
          Ordering::Greater
        } else {
          packages[*n1].name.cmp(&packages[*n2].name)
        }
      });

      order
    };

    let ws = Workspace(Arc::new(WorkspaceInner {
      root,
      packages,
      package_display_order,
      monorepo,
      global_config,
      pkg_graph,
      common,
      processes: Default::default(),
    }));

    for pkg in &ws.packages {
      pkg.set_workspace(&ws);
    }

    Ok(ws)
  }
}

impl WorkspaceInner {
  pub fn package_display_order(&self) -> impl Iterator<Item = &Package> {
    self
      .package_display_order
      .iter()
      .map(|idx| &self.packages[*idx])
  }

  pub fn find_package_by_name(&self, name: &PackageName) -> Result<&Package> {
    self
      .packages
      .iter()
      .find(|pkg| &pkg.name == name)
      .with_context(|| format!("Could not find package with name: {name}"))
  }

  pub fn watch(&self) -> bool {
    self.common.watch
  }

  pub fn start_process(
    &self,
    script: &'static str,
    configure: impl FnOnce(&mut async_process::Command),
  ) -> Result<Arc<Process>> {
    log::trace!("Starting process: {script}");

    let ws_bindir = self.root.join("node_modules").join(".bin");
    let script_path = if script == "pnpm" {
      self.global_config.bindir().join("pnpm")
    } else {
      ws_bindir.join(script)
    };
    ensure!(
      script_path.exists(),
      "Executable is missing: {}",
      script_path.display()
    );

    let mut cmd = async_process::Command::new(script_path);
    cmd.current_dir(&self.root);

    configure(&mut cmd);

    Ok(Arc::new(Process::new(script.to_owned(), cmd)?))
  }

  pub async fn exec(
    &self,
    script: &'static str,
    configure: impl FnOnce(&mut async_process::Command),
  ) -> Result<()> {
    let process = self.start_process(script, configure)?;
    self.processes.write().unwrap().push(process.clone());
    process.wait().await
  }

  pub fn processes(&self) -> RwLockReadGuard<'_, Vec<Arc<Process>>> {
    self.processes.read().unwrap()
  }
}

pub type CommandGraph = DepGraph<Command>;

pub fn build_command_graph(root: &Command) -> CommandGraph {
  DepGraph::build(vec![root.clone()], |cmd| cmd.deps())
}

#[cfg(test)]
mod test {
  use crate::commands::test::{TestArgs, TestCommand};

  use super::*;

  #[test]
  fn test_command_graph() {
    let root = TestCommand::new(TestArgs::default()).kind();
    let _cmd_graph = build_command_graph(&root);
    // TODO: finish this test
  }
}
