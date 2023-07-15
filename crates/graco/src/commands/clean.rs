use anyhow::Result;

use crate::{
  utils,
  workspace::{
    package::Package, Command, CoreCommand, PackageCommand, Workspace, WorkspaceCommand,
  },
};

/// Remove auto-generated files
#[derive(clap::Parser, Debug)]
pub struct CleanArgs {}

#[derive(Debug)]
pub struct CleanCommand {
  #[allow(unused)]
  args: CleanArgs,
}

impl CleanCommand {
  pub fn new(args: CleanArgs) -> Self {
    CleanCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::both(self)
  }
}

impl CoreCommand for CleanCommand {
  fn name(&self) -> String {
    "clean".into()
  }
}

#[async_trait::async_trait]
impl PackageCommand for CleanCommand {
  async fn run_pkg(&self, pkg: &Package) -> Result<()> {
    let to_delete = vec!["node_modules", "dist"];
    for dir in to_delete {
      utils::remove_dir_all_if_exists(pkg.root.join(dir))?;
    }

    Ok(())
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for CleanCommand {
  async fn run_ws(&self, ws: &Workspace) -> Result<()> {
    let to_delete = vec!["node_modules"];
    for dir in to_delete {
      utils::remove_dir_all_if_exists(ws.root.join(dir))?;
    }

    Ok(())
  }
}
