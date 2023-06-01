use anyhow::{Context, Result};
use cfg_if::cfg_if;

use futures::{
  stream::{self, TryStreamExt},
  StreamExt,
};
use log::debug;
use std::{
  borrow::Cow,
  cmp::Ordering,
  env, iter,
  ops::Deref,
  path::{Path, PathBuf},
  sync::Arc,
};

use package::Package;

use crate::{commands::setup::GlobalConfig, utils};

use self::{
  dep_graph::DepGraph,
  package::{PackageIndex, PackageName},
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
  pub dep_graph: DepGraph,

  package_display_order: Vec<PackageIndex>,
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

#[async_trait::async_trait]
pub trait PackageCommand: Send + Sync + 'static {
  async fn run(&self, package: &Package) -> Result<()>;

  fn ignore_dependencies(&self) -> bool {
    false
  }

  fn only_run(&self) -> Cow<'_, Option<PackageName>> {
    Cow::Owned(None)
  }
}

#[async_trait::async_trait]
pub trait WorkspaceCommand {
  async fn run(&self, ws: &Workspace) -> Result<()>;
}

impl Workspace {
  pub async fn load(global_config: GlobalConfig, cwd: Option<PathBuf>) -> Result<Self> {
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

    let dep_graph = DepGraph::build(&packages);

    let package_display_order = {
      let mut order = packages.iter().map(|pkg| pkg.index).collect::<Vec<_>>();

      order.sort_by(|pkg1, pkg2| {
        if dep_graph.is_dependent_on(*pkg2, *pkg1) {
          Ordering::Less
        } else if dep_graph.is_dependent_on(*pkg1, *pkg2) {
          Ordering::Greater
        } else {
          Ordering::Equal
        }
      });

      order.sort_by(|pkg1, pkg2| {
        if dep_graph.is_dependent_on(*pkg2, *pkg1) {
          Ordering::Less
        } else if dep_graph.is_dependent_on(*pkg1, *pkg2) {
          Ordering::Greater
        } else {
          packages[*pkg1].name.cmp(&packages[*pkg2].name)
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
      dep_graph,
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
}
