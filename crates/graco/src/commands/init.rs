use std::{borrow::Cow, path::Path, str::FromStr};

use crate::{
  utils,
  workspace::{
    package::{Package, PackageName, Platform},
    PackageCommand, Workspace, WorkspaceCommand,
  },
};
use anyhow::Result;

#[derive(clap::Parser, Default)]
pub struct InitArgs {
  #[arg(short, long)]
  pub package: Option<PackageName>,
}

pub struct InitCommand {
  #[allow(unused)]
  args: InitArgs,
}

impl InitCommand {
  pub fn new(args: InitArgs) -> Self {
    InitCommand { args }
  }

  fn link_packages(
    &self,
    pkgs_to_link: &[&str],
    local_node_modules: &Path,
    ws: &Workspace,
  ) -> Result<()> {
    let global_node_modules = ws.global_config.node_path();
    for to_link in pkgs_to_link {
      let pkg_name = PackageName::from_str(to_link).unwrap();
      let src = global_node_modules.join(to_link);
      let dst = local_node_modules.join(to_link);
      if let Some(scope) = pkg_name.scope {
        utils::create_dir_if_missing(
          local_node_modules.join(local_node_modules.join(format!("@{scope}"))),
        )?;
      }
      utils::symlink_dir_if_missing(&src, &dst)?;
    }
    Ok(())
  }
}

#[async_trait::async_trait]
impl WorkspaceCommand for InitCommand {
  async fn run(&self, ws: &Workspace) -> Result<()> {
    let local_node_modules = ws.root.join("node_modules");
    utils::create_dir_if_missing(&local_node_modules)?;

    let pkgs_to_link = vec!["@trivago/prettier-plugin-sort-imports"];
    self.link_packages(&pkgs_to_link, &local_node_modules, ws)?;

    Ok(())
  }
}

#[async_trait::async_trait]
impl PackageCommand for InitCommand {
  async fn run(&self, pkg: &Package) -> Result<()> {
    let local_node_modules = pkg.root.join("node_modules");
    utils::create_dir_if_missing(&local_node_modules)?;

    let mut pkgs_to_link = Vec::new();
    match pkg.platform {
      Platform::Browser => pkgs_to_link.extend(["jsdom", "@vitejs/plugin-react"]),
      Platform::Node => {}
    }
    pkgs_to_link.extend(["vite", "vitest"]);

    self.link_packages(&pkgs_to_link, &local_node_modules, pkg.workspace())?;

    Ok(())
  }

  fn only_run(&self) -> Cow<'_, Option<PackageName>> {
    Cow::Borrowed(&self.args.package)
  }
}
