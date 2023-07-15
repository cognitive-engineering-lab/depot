use crate::workspace::{Command, CoreCommand, Workspace, WorkspaceCommand};
use anyhow::{ensure, Result};

/// Initialize a workspace
#[derive(clap::Parser, Default, Debug)]
pub struct InitArgs {
  /// If true, then don't attempt to download packages from the web
  #[arg(long, action)]
  pub offline: bool,
}

#[derive(Debug)]
pub struct InitCommand {
  #[allow(unused)]
  args: InitArgs,
}

impl InitCommand {
  pub fn new(args: InitArgs) -> Self {
    InitCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::workspace(self)
  }
}

impl CoreCommand for InitCommand {
  fn name(&self) -> String {
    "init".into()
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for InitCommand {
  async fn run_ws(&self, ws: &Workspace) -> Result<()> {
    let mut cmd = async_process::Command::new(ws.global_config.bindir().join("pnpm"));
    cmd.current_dir(&ws.root);
    cmd.arg("install");

    if self.args.offline {
      cmd.arg("--offline");
    }

    let status = cmd.status().await?;
    ensure!(status.success(), "pnpm install failed");

    Ok(())
  }
}
