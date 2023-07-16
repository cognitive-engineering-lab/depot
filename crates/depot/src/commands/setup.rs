use crate::utils;
#[cfg(unix)]
use std::os::unix::prelude::PermissionsExt;
use std::{
  env,
  fs::{File, Permissions},
  io::{BufWriter, Write},
  path::{Path, PathBuf},
};

use anyhow::{ensure, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};

/// Setup Depot for use on this machine
#[derive(clap::Parser)]
pub struct SetupArgs {
  /// Directory for global Depot configuration, defaults to $HOME/.depot
  #[arg(short, long)]
  pub config_dir: Option<PathBuf>,
}

pub struct SetupCommand {
  args: SetupArgs,
}

#[derive(Clone)]
pub struct GlobalConfig {
  root: PathBuf,
}

const HOME_ENV_VAR: &str = "GRACO_HOME";

impl GlobalConfig {
  fn find_root() -> Result<PathBuf> {
    Ok(match env::var(HOME_ENV_VAR) {
      Ok(val) => PathBuf::from(val),
      Err(_) => {
        let home_dir = home::home_dir().context("Could not find home directory")?;
        home_dir.join(".depot")
      }
    })
  }

  pub fn load() -> Result<Self> {
    let root = Self::find_root()?;
    ensure!(
      root.exists(),
      "Depot global config directory does not exist: {}",
      root.display()
    );
    Ok(GlobalConfig { root })
  }

  pub fn bindir(&self) -> PathBuf {
    self.root.join("bin")
  }
}

const PNPM_VERSION: &str = "8.6.7";

fn download_file(url: &str, mut dst: impl Write) -> Result<()> {
  let mut curl = curl::easy::Easy::new();
  curl.url(url)?;
  curl.follow_location(true)?;

  curl.nobody(true)?;
  curl.perform()?;
  let total_size = curl.content_length_download()? as u64;

  curl.nobody(false)?;
  curl.progress(true)?;

  log::debug!("Starting download...");
  let bar = ProgressBar::new(total_size);
  bar.set_style(
    ProgressStyle::with_template(
      "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes}",
    )
    .unwrap()
    .progress_chars("#>-"),
  );

  let mut transfer = curl.transfer();
  transfer.progress_function(|_, current, _, _| {
    bar.set_position(current as u64);
    true
  })?;
  transfer.write_function(|data| dst.write(data).map_err(|_| curl::easy::WriteError::Pause))?;

  transfer.perform()?;
  bar.finish();

  Ok(())
}

fn download_pnpm(dst: &Path) -> Result<()> {
  let version = PNPM_VERSION;
  let platform = match env::consts::OS {
    "macos" | "ios" => "macos",
    "windows" => "win",
    _ => "linuxstatic",
  };
  let arch = match env::consts::ARCH {
    "arm" => "arm64",
    _ => "x64",
  };

  let pnpm_url =
    format!("https://github.com/pnpm/pnpm/releases/download/v{version}/pnpm-{platform}-{arch}");

  let mut file = File::create(dst).context("Could not save pnpm binary to file")?;
  download_file(&pnpm_url, BufWriter::new(&mut file))?;

  #[cfg(unix)]
  file.set_permissions(Permissions::from_mode(0o555))?;

  Ok(())
}

impl SetupCommand {
  pub fn new(args: SetupArgs) -> Self {
    SetupCommand { args }
  }

  pub fn run(self) -> Result<()> {
    let config_dir = match self.args.config_dir {
      Some(dir) => dir,
      None => GlobalConfig::find_root()?,
    };
    if config_dir.exists() {
      return Ok(());
    }
    utils::create_dir_if_missing(&config_dir)?;

    let config = GlobalConfig { root: config_dir };
    let bindir = config.bindir();
    utils::create_dir_if_missing(&bindir)?;

    let pnpm_path = bindir.join("pnpm");
    if !pnpm_path.exists() {
      println!("Downloading pnpm from Github...");
      download_pnpm(&pnpm_path)?;
    }

    println!("Setup complete!");

    Ok(())
  }
}
