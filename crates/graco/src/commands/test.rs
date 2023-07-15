use std::borrow::Cow;

use super::build::{BuildArgs, BuildCommand};
use crate::workspace::{
  package::{Package, PackageName},
  Command, CoreCommand, PackageCommand,
};
use anyhow::{Context, Result};

/// Run tests via vitest
#[derive(clap::Parser, Default, Debug)]
pub struct TestArgs {
  /// Additional arguments to pass to vitest
  #[arg(last = true)]
  pub vitest_args: Option<String>,
}

#[derive(Debug)]
pub struct TestCommand {
  args: TestArgs,
}

impl CoreCommand for TestCommand {
  fn name(&self) -> String {
    "test".into()
  }
}

#[async_trait::async_trait]
impl PackageCommand for TestCommand {
  async fn run_pkg(&self, pkg: &Package) -> Result<()> {
    if !pkg.root.join("tests").exists() {
      return Ok(());
    }

    let vitest_args = match &self.args.vitest_args {
      Some(vitest_args) => Some(shlex::split(vitest_args).context("Failed to parse vitest args")?),
      None => None,
    };

    pkg
      .exec("vitest", |cmd| {
        let subcmd = if pkg.workspace().watch() {
          "watch"
        } else {
          "run"
        };
        cmd.arg(subcmd);

        cmd.arg("--passWithNoTests");

        if let Some(vitest_args) = vitest_args {
          cmd.args(vitest_args);
        }
      })
      .await
  }

  fn deps(&self) -> Vec<Command> {
    vec![BuildCommand::new(BuildArgs::default()).kind()]
  }
}

impl TestCommand {
  pub fn new(args: TestArgs) -> Self {
    TestCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::package(self)
  }
}
