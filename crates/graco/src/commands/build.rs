use std::borrow::Cow;

use anyhow::Result;

use futures::{future::try_join_all, FutureExt};

use super::init::{InitArgs, InitCommand};
use crate::workspace::{
  package::{Package, PackageName, Target},
  Command, CoreCommand, PackageCommand,
};

/// Check and build packages
#[derive(clap::Parser, Default, Debug)]
pub struct BuildArgs {
  /// Build in release mode
  #[arg(short, long)]
  pub release: bool,

  /// If true, then don't attempt to download packages from the web
  #[arg(long, action)]
  pub offline: bool,
}

#[derive(Debug)]
pub struct BuildCommand {
  args: BuildArgs,
}

const BUILD_SCRIPT: &str = "build.mjs";

impl CoreCommand for BuildCommand {
  fn name(&self) -> String {
    "build".into()
  }
}

#[async_trait::async_trait]
impl PackageCommand for BuildCommand {
  async fn run_pkg(&self, pkg: &Package) -> Result<()> {
    let mut processes = Vec::new();

    if pkg.target.is_script() || pkg.target.is_site() {
      processes.push(self.vite(pkg).boxed());
    }

    if pkg.root.join(BUILD_SCRIPT).exists() {
      processes.push(self.build_script(pkg).boxed());
    }

    processes.extend([self.tsc(pkg).boxed(), self.eslint(pkg).boxed()]);

    try_join_all(processes).await?;

    Ok(())
  }

  fn deps(&self) -> Vec<Command> {
    vec![InitCommand::new(InitArgs::default()).kind()]
  }

  fn ignore_dependencies(&self) -> bool {
    false
  }
}

impl BuildCommand {
  pub fn new(args: BuildArgs) -> Self {
    BuildCommand { args }
  }

  pub fn kind(self) -> Command {
    Command::package(self)
  }

  async fn tsc(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("tsc", |cmd| {
        cmd.arg("--pretty");
        if pkg.workspace().watch() {
          cmd.arg("--watch");
        }
        if pkg.target.is_lib() && !self.args.release {
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
      .exec("vite", |cmd| match pkg.target {
        Target::Site => {
          cmd.arg(if pkg.workspace().watch() {
            "dev"
          } else {
            "build"
          });
        }
        _ => {
          cmd.arg("build");
          if pkg.workspace().watch() {
            cmd.arg("--watch");
          }
          if !self.args.release {
            cmd.args(["--sourcemap", "true"]);
            cmd.args(["--minify", "false"]);
          }
        }
      })
      .await
  }

  async fn build_script(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("pnpm", |cmd| {
        cmd.args(["exec", "node", BUILD_SCRIPT]);
        if pkg.workspace().watch() {
          cmd.arg("--watch");
        }
        if self.args.release {
          cmd.arg("--release");
        }
      })
      .await
  }
}
