use anyhow::{Context, Result};
use log::debug;
use rayon::prelude::*;
use std::{
  env, iter,
  path::{Path, PathBuf},
  process::Command,
};

use package::Package;

pub mod package;

pub struct Workspace {
  pub root: PathBuf,
  pub packages: Vec<Package>,
  pub monorepo: bool,
}

fn get_git_root(cwd: &Path) -> Option<PathBuf> {
  let mut cmd = Command::new("git");
  cmd.args(["rev-parse", "--show-toplevel"]).current_dir(cwd);
  Some(PathBuf::from(
    String::from_utf8(cmd.output().ok()?.stdout).unwrap(),
  ))
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

impl Workspace {
  pub fn load(cwd: Option<PathBuf>) -> Result<Self> {
    let cwd = match cwd {
      Some(cwd) => cwd,
      None => PathBuf::from(env::current_dir()?),
    };

    let fs_root = cwd.components().next().unwrap();
    let git_root = get_git_root(&cwd);
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

    let packages = pkg_roots
      .into_par_iter()
      .map(|pkg_root| Package::load(&pkg_root))
      .collect::<Result<Vec<_>>>()?;

    Ok(Workspace {
      root,
      packages,
      monorepo,
    })
  }
}
