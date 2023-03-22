use anyhow::Result;

use crate::{
  utils,
  workspace::{package::Package, PackageCommand, Workspace, WorkspaceCommand},
};

#[derive(clap::Parser)]
pub struct CleanArgs {}

pub struct CleanCommand {
  #[allow(unused)]
  args: CleanArgs,
}

impl CleanCommand {
  pub fn new(args: CleanArgs) -> Self {
    CleanCommand { args }
  }
}

#[async_trait::async_trait]
impl PackageCommand for CleanCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let to_delete = vec!["node_modules", "dist"];
    for dir in to_delete {
      utils::remove_dir_all_if_exists(pkg.root.join(dir))?;
    }

    Ok(())
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for CleanCommand {
  async fn run(&self, ws: &Workspace) -> Result<()> {
    let to_delete = vec!["node_modules"];
    for dir in to_delete {
      utils::remove_dir_all_if_exists(ws.root.join(dir))?;
    }

    Ok(())
  }
}
