use anyhow::Result;
use futures::{
  future::{join_all, BoxFuture},
  FutureExt,
};

use crate::workspace::{
  package::{Package, PackageName},
  PackageCommand,
};

#[derive(clap::Parser)]
pub struct BuildArgs {
  #[arg(short, long)]
  watch: bool,

  #[arg(short, long)]
  release: bool,

  #[arg(short, long)]
  package: Option<PackageName>,
}

pub struct BuildCommand {
  args: BuildArgs,
}

#[async_trait::async_trait]
impl PackageCommand for BuildCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let processes: Vec<BoxFuture<'_, Result<()>>> =
      vec![self.check(pkg).boxed(), self.lint(pkg).boxed()];
    for result in join_all(processes).await {
      result?;
    }
    Ok(())
  }

  fn ignore_dependencies(&self) -> bool {
    self.args.watch
  }
}

impl BuildCommand {
  pub fn new(args: BuildArgs) -> Self {
    BuildCommand { args }
  }

  async fn check(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("tsc", |cmd| {
        cmd.arg("--pretty");
        if self.args.watch {
          cmd.arg("-w");
        }
      })
      .await
  }

  async fn lint(&self, pkg: &Package) -> Result<()> {
    pkg.exec("eslint", |_| {}).await
  }
}
