use crate::workspace::{Command, CoreCommand, Workspace, WorkspaceCommand};
use anyhow::{Context, Result};

/// Initialize a workspace
#[derive(clap::Parser, Default, Debug)]
pub struct InitArgs {
  /// If true, then don't attempt to download packages from the web
  #[arg(long, action)]
  pub offline: bool,

  /// Additional arguments to pass to vitest
  #[arg(last = true)]
  pub pnpm_args: Option<String>,
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
    let pnpm_args = match &self.args.pnpm_args {
      Some(pnpm_args) => Some(shlex::split(pnpm_args).context("Failed to parse pnpm args")?),
      None => None,
    };

    ws.exec("pnpm", |cmd| {
      cmd.arg("install");

      if self.args.offline {
        cmd.arg("--offline");
      }

      if let Some(pnpm_args) = pnpm_args {
        cmd.args(pnpm_args);
      }
    })
    .await
  }
}
