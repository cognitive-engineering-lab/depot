use crate::workspace::{Workspace, WorkspaceCommand};
use anyhow::{ensure, Result};

/// Initialize a workspace
#[derive(clap::Parser, Default)]
pub struct InitArgs {}

pub struct InitCommand {
  #[allow(unused)]
  args: InitArgs,
}

impl InitCommand {
  pub fn new(args: InitArgs) -> Self {
    InitCommand { args }
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for InitCommand {
  async fn run(&self, ws: &Workspace) -> Result<()> {
    let mut cmd = async_process::Command::new(ws.global_config.bindir().join("pnpm"));
    cmd.current_dir(&ws.root);
    cmd.arg("install");

    let status = cmd.status().await?;
    ensure!(status.success(), "pnpm install failed");

    Ok(())
  }
}
