use crate::workspace::{package::Package, PackageCommand};
use anyhow::{Context, Result};

#[derive(clap::Parser)]
pub struct TestArgs {
  #[arg(short, long)]
  watch: bool,

  #[arg(last = true)]
  vitest_args: Option<String>,
}

pub struct TestCommand {
  args: TestArgs,
}

#[async_trait::async_trait]
impl PackageCommand for TestCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let vitest_args = match &self.args.vitest_args {
      Some(vitest_args) => Some(shlex::split(vitest_args).context("Failed to parse vitest args")?),
      None => None,
    };

    pkg
      .exec("vitest", |cmd| {
        let subcmd = if self.args.watch { "watch" } else { "run" };
        cmd.arg(subcmd);

        if let Some(vitest_args) = vitest_args {
          cmd.args(vitest_args);
        }
      })
      .await
  }
}

impl TestCommand {
  pub fn new(args: TestArgs) -> Self {
    TestCommand { args }
  }
}
