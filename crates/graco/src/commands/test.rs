use std::borrow::Cow;

use crate::workspace::{
  package::{Package, PackageName},
  PackageCommand,
};
use anyhow::{Context, Result};

/// Run tests via vitest
#[derive(clap::Parser)]
pub struct TestArgs {  
  /// If true, rerun tests when source files change
  #[arg(short, long, action)]
  pub watch: bool,

  /// Only run tests for a specific package
  #[arg(short, long)]
  pub package: Option<PackageName>,

  /// Additional arguments to pass to vitest
  #[arg(last = true)]
  pub vitest_args: Option<String>,
}

pub struct TestCommand {
  args: TestArgs,
}

#[async_trait::async_trait]
impl PackageCommand for TestCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    if !pkg.root.join("tests").exists() {
      return Ok(());
    }

    let vitest_args = match &self.args.vitest_args {
      Some(vitest_args) => Some(shlex::split(vitest_args).context("Failed to parse vitest args")?),
      None => None,
    };

    pkg
      .exec("vitest", |cmd| {
        let subcmd = if self.args.watch { "watch" } else { "run" };
        cmd.arg(subcmd);

        cmd.arg("--passWithNoTests");

        if let Some(vitest_args) = vitest_args {
          cmd.args(vitest_args);
        }
      })
      .await
  }

  fn only_run(&self) -> Cow<'_, Option<PackageName>> {
    Cow::Borrowed(&self.args.package)
  }
}

impl TestCommand {
  pub fn new(args: TestArgs) -> Self {
    TestCommand { args }
  }
}
