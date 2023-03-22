use anyhow::Result;

use futures::{future::try_join_all, FutureExt};

use crate::workspace::{
  package::{Package, PackageName, Target},
  PackageCommand,
};

#[derive(clap::Parser, Default)]
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

const BUILD_SCRIPT: &str = "build.mjs";

#[async_trait::async_trait]
impl PackageCommand for BuildCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let mut processes = Vec::new();

    if matches!(pkg.target, Target::Site) {
      processes.push(self.vite(pkg).boxed());
    }

    if pkg.root.join(BUILD_SCRIPT).exists() {
      processes.push(self.build_script(pkg).boxed());
    }

    processes.extend([self.tsc(pkg).boxed(), self.eslint(pkg).boxed()]);

    try_join_all(processes).await?;

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

  async fn tsc(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("tsc", |cmd| {
        cmd.arg("--pretty");
        if self.args.watch {
          cmd.arg("--watch");
        }
        if matches!(pkg.target, Target::Lib) && !self.args.release {
          cmd.arg("--sourceMap");
        }
      })
      .await
  }

  async fn eslint(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("eslint", |_| {
        // TODO: watch mode
      })
      .await
  }

  async fn vite(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("vite", |cmd| {
        cmd.arg(if self.args.watch { "dev" } else { "build" });
      })
      .await
  }

  async fn build_script(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("pnpm", |cmd| {
        cmd.args(["exec", "node", BUILD_SCRIPT]);
        if self.args.watch {
          cmd.arg("--watch");
        }
        if self.args.release {
          cmd.arg("--release");
        }
      })
      .await
  }
}
