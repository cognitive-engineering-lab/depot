use anyhow::{Context, Result};
use log::debug;
use petgraph::{
  data::{Element, FromElements},
  graph::DiGraph,
  prelude::NodeIndex,
  visit::{DfsPostOrder, IntoNeighborsDirected, Walker},
};
use rayon::prelude::*;
use std::{
  env,
  io::Stdout,
  iter,
  ops::Deref,
  path::{Path, PathBuf},
  sync::{Arc, Mutex},
};

use package::Package;

use crate::{commands::setup::GlobalConfig, logger::Logger, utils};

use self::package::PackageIndex;

pub mod package;
mod runner;

#[derive(Clone)]
pub struct Workspace(Arc<WorkspaceInner>);

impl Deref for Workspace {
  type Target = WorkspaceInner;
  fn deref(&self) -> &Self::Target {
    &self.0
  }
}

pub type Terminal = tui::Terminal<tui::backend::CrosstermBackend<Stdout>>;

pub fn load_terminal() -> Result<Terminal> {
  let stdout = std::io::stdout();
  let backend = tui::backend::CrosstermBackend::new(stdout);
  Ok(tui::Terminal::new(backend)?)
}

pub struct WorkspaceInner {
  pub root: PathBuf,
  pub packages: Vec<Package>,
  pub monorepo: bool,
  pub global_config: GlobalConfig,
  pub dep_graph: DepGraph,
  pub logger: Mutex<Logger>,
  pub terminal: Mutex<Terminal>,
}

fn find_workspace_root(max_ancestor: &Path, cwd: &Path) -> Result<PathBuf> {
  let rel_path_to_cwd = cwd.strip_prefix(max_ancestor)?;
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

#[async_trait::async_trait]
pub trait PackageCommand: Send + Sync + 'static {
  async fn run(&self, package: &Package) -> Result<()>;
  fn ignore_dependencies(&self) -> bool {
    false
  }
}

pub trait WorkspaceCommand {
  fn run(&self) -> Result<()>;
}

pub struct DepGraph {
  graph: DiGraph<(), ()>,
}

impl DepGraph {
  pub fn build(packages: &[Package]) -> Self {
    let edges = packages.iter().flat_map(|pkg| {
      pkg
        .all_dependencies()
        .filter_map(|name| packages.iter().find(|other_pkg| &other_pkg.name == &name))
        .map(move |dep| Element::Edge {
          source: pkg.index,
          target: dep.index,
          weight: (),
        })
    });

    let graph = DiGraph::<(), ()>::from_elements(
      (0..packages.len())
        .map(|_| Element::Node { weight: () })
        .chain(edges),
    );

    DepGraph { graph }
  }

  pub fn immediate_deps_for(&self, index: PackageIndex) -> impl Iterator<Item = PackageIndex> + '_ {
    self
      .graph
      .neighbors_directed(NodeIndex::new(index), petgraph::Direction::Incoming)
      .map(|node| node.index())
  }

  pub fn all_deps_for(&self, index: PackageIndex) -> impl Iterator<Item = PackageIndex> + '_ {
    DfsPostOrder::new(&self.graph, NodeIndex::new(index))
      .iter(&self.graph)
      .map(|node| node.index())
  }
}

impl Workspace {
  pub fn load(global_config: GlobalConfig, cwd: Option<PathBuf>) -> Result<Self> {
    let cwd = match cwd {
      Some(cwd) => cwd,
      None => env::current_dir()?,
    };

    let fs_root = cwd.components().next().unwrap();
    let git_root = utils::get_git_root(&cwd);
    let max_ancestor: &Path = git_root.as_deref().unwrap_or_else(|| fs_root.as_ref());
    let root = find_workspace_root(max_ancestor, &cwd)?;
    debug!("Workspace root: {}", root.display());

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

    // TODO: replace rayon with async here
    let packages = pkg_roots
      .into_par_iter()
      .enumerate()
      .map(|(index, pkg_root)| Package::load(&pkg_root, index))
      .collect::<Result<Vec<_>>>()?;

    let dep_graph = DepGraph::build(&packages);

    let logger = Mutex::new(Logger::new()?);
    let terminal = Mutex::new(load_terminal()?);

    let ws = Workspace(Arc::new(WorkspaceInner {
      root,
      packages,
      monorepo,
      global_config,
      dep_graph,
      logger,
      terminal,
    }));

    for pkg in &ws.packages {
      pkg.set_workspace(&ws);
    }

    Ok(ws)
  }
}
