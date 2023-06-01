use crate::utils;
#[cfg(unix)]
use std::os::unix::prelude::PermissionsExt;
use std::{
  env,
  fs::{File, Permissions},
  io::{BufWriter, Write},
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{ensure, Context, Result};

#[derive(clap::Parser)]
pub struct SetupArgs {
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
        home_dir.join(".graco")
      }
    })
  }

  pub fn load() -> Result<Self> {
    let root = Self::find_root()?;
    ensure!(
      root.exists(),
      "Graco global config directory does not exist: {}",
      root.display()
    );
    Ok(GlobalConfig { root })
  }

  pub fn bindir(&self) -> PathBuf {
    self.root.join("bin")
  }

  pub fn node_path(&self) -> PathBuf {
    self.bindir().join("global/5/node_modules")
  }
}

const PNPM_VERSION: &str = "7.29.1";

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

  let mut file = File::create(dst).context("Could not save pnpm binary to file")?;

  {
    let mut writer = BufWriter::new(&mut file);
    let pnpm_url =
      format!("https://github.com/pnpm/pnpm/releases/download/v{version}/pnpm-{platform}-{arch}");
    let mut curl = curl::easy::Easy::new();
    curl.url(&pnpm_url)?;
    curl.follow_location(true)?;
    let mut transfer = curl.transfer();
    transfer.write_function(|data| {
      writer
        .write(data)
        .map_err(|_| curl::easy::WriteError::Pause)
    })?;
    transfer.perform()?;
  }

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
      println!("Downloading pnpm...");
      download_pnpm(&pnpm_path)?;
    }

    #[rustfmt::skip]
    const PACKAGES: &[&str] = &[
      // Types
      "typescript@^5.0.2",
      "@types/node@^18.15.10",

      // Bundling
      "vite@^4.2.1",
      "@vitejs/plugin-react@^3.1.0",

      // Testing
      "vitest@^0.29.7",
      "jsdom@^21.1.1",

      // Linting
      "eslint@^8.36.0",
      "eslint-plugin-react@^7.32.2",
      "eslint-plugin-react-hooks@^4.6.0",
      "@typescript-eslint/eslint-plugin@^5.56.0",
      "@typescript-eslint/parser@^5.56.0",
      "eslint-plugin-prettier@^4.2.1",

      // Formatting
      "prettier@^2.8.7",
      "@trivago/prettier-plugin-sort-imports@^4.1.1",

      // Documentation generation
      "typedoc@^0.23.28"
    ];

    println!("Installing JS dependencies...");
    let bindir = config.bindir();
    let mut pnpm = Command::new(bindir.join("pnpm"));
    pnpm.env("PNPM_HOME", &bindir);
    let path = env::var("PATH").unwrap_or_else(|_| String::new());
    pnpm.env("PATH", format!("{}:{path}", bindir.display()));

    let status = pnpm.args(["install", "--global"]).args(PACKAGES).status()?;
    ensure!(status.success(), "pnpm global installation failed");

    Ok(())
  }
}
