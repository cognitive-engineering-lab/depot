use crate::workspace::{Workspace, WorkspaceCommand};
use anyhow::{ensure, Context, Result};

#[derive(clap::Parser)]
pub struct TestArgs {
  #[arg(last = true)]
  jest_args: Option<String>,
}

pub struct TestCommand {
  args: TestArgs,
}

#[async_trait::async_trait]
impl WorkspaceCommand for TestCommand {
  async fn run(&self, ws: &Workspace) -> Result<()> {
    let mut cmd = async_process::Command::new(ws.global_config.bindir().join("jest"));
    cmd.current_dir(&ws.root);

    if let Some(jest_args) = &self.args.jest_args {
      cmd.args(shlex::split(jest_args).context("Failed to parse jest args")?);
    }

    let status = cmd.status().await?;
    ensure!(status.success(), "jest failed");
    Ok(())
  }
}

impl TestCommand {
  pub fn new(args: TestArgs) -> Self {
    TestCommand { args }
  }
}
