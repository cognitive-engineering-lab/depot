use std::str::FromStr;

use anyhow::Result;

use futures::{future::try_join_all, FutureExt};

use crate::{
  utils,
  workspace::{
    package::{Package, PackageName, Target},
    PackageCommand,
  },
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

const BUILD_SCRIPT: &str = "build.mjs";

#[async_trait::async_trait]
impl PackageCommand for BuildCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    self.init(pkg).await?;

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

  async fn init(&self, pkg: &Package) -> Result<()> {
    let local_node_modules = pkg.root.join("node_modules");
    utils::create_dir_if_missing(&local_node_modules)?;

    let ws = pkg.workspace();
    let pkgs_to_link = match pkg.target {
      Target::Script => vec!["esbuild"],
      Target::Site => vec!["vite", "@vitejs/plugin-react"],
      Target::Lib => vec![],
    };

    let global_node_modules = ws.global_config.node_path();
    for pkg in pkgs_to_link {
      let pkg_name = PackageName::from_str(pkg).unwrap();
      let src = global_node_modules.join(pkg);
      let dst = local_node_modules.join(pkg);
      if let Some(scope) = pkg_name.scope {
        utils::create_dir_if_missing(
          local_node_modules.join(local_node_modules.join(format!("@{scope}"))),
        )?;
      }

      utils::symlink_dir_if_missing(&src, &dst)?;
    }

    Ok(())
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
