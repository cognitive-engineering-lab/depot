use anyhow::{ensure, Context, Result};

use crate::workspace::{Workspace, WorkspaceCommand};

#[derive(clap::Parser)]
pub struct FmtArgs {
  #[arg(last = true)]
  prettier_args: Option<String>,
}

pub struct FmtCommand {
  #[allow(unused)]
  args: FmtArgs,
}

impl FmtCommand {
  pub fn new(args: FmtArgs) -> Self {
    FmtCommand { args }
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for FmtCommand {
  async fn run(&self, ws: &Workspace) -> Result<()> {
    let mut cmd = async_process::Command::new(ws.global_config.bindir().join("prettier"));
    cmd.current_dir(&ws.root);
    let prefix = if ws.monorepo { "packages/**/" } else { "" };
    cmd.arg(prefix.to_owned() + "{src,tests}/**/*.{ts,tsx}");
    cmd.arg("-w");

    if let Some(jest_args) = &self.args.prettier_args {
      cmd.args(shlex::split(jest_args).context("Failed to parse prettier args")?);
    }

    let status = cmd.status().await?;
    ensure!(status.success(), "prettier failed");
    Ok(())
  }
}
