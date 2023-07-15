use anyhow::{Context, Result};

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
    let typedoc_args = match &self.args.typedoc_args {
      Some(typedoc_args) => {
        Some(shlex::split(typedoc_args).context("Failed to parse typedoc args")?)
      }
      None => None,
    };

    ws.exec("typedoc", |cmd| {
      if let Some(typedoc_args) = typedoc_args {
        cmd.args(typedoc_args);
      }
    })
    .await
  }
}
