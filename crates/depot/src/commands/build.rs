use std::{fs, path::Path, time::Duration};

use anyhow::{anyhow, ensure, Result};
use futures::{future::try_join_all, FutureExt};
use log::debug;
use notify::RecursiveMode;

use super::init::{InitArgs, InitCommand};
use crate::{
  utils,
  workspace::{
    package::{Package, Target},
    Command, CommandRuntime, CoreCommand, PackageCommand,
  },
};

/// Check and build packages
#[derive(clap::Parser, Default, Debug)]
pub struct BuildArgs {
  /// Build in release mode
  #[arg(short, long)]
  pub release: bool,

  /// Don't attempt to download packages from the web
  #[arg(long, action)]
  pub offline: bool,

  /// Rebuild when files change
  #[clap(short, long, action)]
  pub watch: bool,

  /// Fail if eslint finds a lint issue
  #[clap(short, long, action)]
  pub lint_fail: bool,
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

    match pkg.target {
      Target::Script | Target::Site => processes.push(self.vite(pkg).boxed()),
      Target::Lib => processes.push(self.copy_assets(pkg).boxed()),
    }

    processes.extend([self.tsc(pkg).boxed(), self.eslint(pkg).boxed()]);

    if pkg.root.join(BUILD_SCRIPT).exists() {
      processes.push(self.build_script(pkg).boxed());
    }

    try_join_all(processes).await?;

    Ok(())
  }

  fn deps(&self) -> Vec<Command> {
    vec![InitCommand::new(InitArgs::default()).kind()]
  }

  fn runtime(&self) -> CommandRuntime {
    if self.args.watch {
      CommandRuntime::RunForever
    } else {
      CommandRuntime::WaitForDependencies
    }
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
        if self.args.watch {
          cmd.arg("--watch");
        }
        if pkg.target.is_lib() && !self.args.release {
          cmd.arg("--sourceMap");
        }
      })
      .await
  }

  async fn eslint(&self, pkg: &Package) -> Result<()> {
    let process = pkg.start_process("eslint", |cmd| {
      cmd.args(pkg.source_files());
      cmd.arg("--color");
      // TODO: watch mode
    })?;

    let status = process.wait().await?;
    ensure!(!self.args.lint_fail || status.success(), "eslint failed");

    Ok(())
  }

  async fn vite(&self, pkg: &Package) -> Result<()> {
    pkg
      .exec("vite", |cmd| {
        cmd.env("FORCE_COLOR", "1");
        match pkg.target {
          Target::Site => {
            cmd.arg(if self.args.watch { "dev" } else { "build" });
          }
          _ => {
            cmd.arg("build");
            if self.args.watch {
              cmd.arg("--watch");
            }
            if !self.args.release {
              cmd.args(["--sourcemap", "true"]);
              cmd.args(["--minify", "false"]);
            }
          }
        }
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

  async fn copy_assets(&self, pkg: &Package) -> Result<()> {
    let src_dir = pkg.root.join("src");
    let dst_dir = pkg.root.join("dist");

    let copy = |file: &Path| -> Result<()> {
      let rel_path = file.strip_prefix(&src_dir)?;
      let target_path = dst_dir.join(rel_path);
      utils::create_dir_if_missing(target_path.parent().unwrap())?;
      debug!("copying: {} -> {}", file.display(), target_path.display());
      fs::copy(file, target_path)?;
      Ok(())
    };

    for file in pkg.asset_files() {
      copy(&file)?;
    }

    if self.args.watch {
      let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
      let timeout = Duration::from_secs(1);
      let mut debouncer = notify_debouncer_mini::new_debouncer(timeout, None, move |events| {
        let _ = tx.send(events);
      })?;

      for file in pkg.asset_files() {
        debouncer
          .watcher()
          .watch(&file, RecursiveMode::NonRecursive)?;
      }

      while let Some(events) = rx.recv().await {
        let events = events.map_err(|e| anyhow!("File watch errors: {e:?}"))?;
        for event in events {
          copy(&event.path)?;
        }
      }
    }

    Ok(())
  }
}
