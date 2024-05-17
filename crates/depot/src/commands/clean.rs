use anyhow::Result;

use crate::{
  utils,
  workspace::{Command, CoreCommand, Workspace, WorkspaceCommand},
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
    Command::workspace(self)
  }
}

impl CoreCommand for CleanCommand {
  fn name(&self) -> String {
    "clean".into()
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for CleanCommand {
  async fn run_ws(&self, ws: &Workspace) -> Result<()> {
    let mut to_delete = vec![ws.root.join("node_modules")];
    for pkg in &ws.packages {
      to_delete.extend([pkg.root.join("node_modules"), pkg.root.join("dist")])
    }

    for dir in to_delete {
      utils::remove_dir_all_if_exists(dir)?;
    }

    Ok(())
  }
}
