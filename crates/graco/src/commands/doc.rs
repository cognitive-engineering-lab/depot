use anyhow::{ensure, Context, Result};

use crate::workspace::{Command, CoreCommand, Workspace, WorkspaceCommand};

/// Generate documentation for libraries with typedoc
#[derive(clap::Parser, Debug)]
pub struct DocArgs {
  /// Additional arguments to pass to typedoc
  #[arg(last = true)]
  pub typedoc_args: Option<String>,
}

#[derive(Debug)]
pub struct DocCommand {
  args: DocArgs,
}

impl DocCommand {
  pub fn new(args: DocArgs) -> Self {
    DocCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::workspace(self)
  }
}

impl CoreCommand for DocCommand {
  fn name(&self) -> String {
    "doc".into()
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for DocCommand {
  async fn run_ws(&self, ws: &Workspace) -> Result<()> {
    let mut cmd = async_process::Command::new(ws.global_config.bindir().join("pnpm"));
    cmd.args(["exec", "typedoc"]);
    cmd.current_dir(&ws.root);

    if let Some(typedoc_args) = &self.args.typedoc_args {
      cmd.args(shlex::split(typedoc_args).context("Failed to parse typedoc args")?);
    }

    let status = cmd.status().await?;
    ensure!(status.success(), "typedoc failed");
    Ok(())
  }
}
