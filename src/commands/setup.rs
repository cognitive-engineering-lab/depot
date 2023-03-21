use crate::utils;
#[cfg(unix)]
use std::os::unix::prelude::PermissionsExt;
use std::{
  env,
  fs::{File, Permissions},
  io::BufWriter,
  path::{Path, PathBuf},
  process::Command,
};

use anyhow::{ensure, Context, Result};

#[derive(clap::Parser)]
pub struct SetupArgs {
  #[arg(short, long)]
  config_dir: Option<PathBuf>,
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

  /*
  TODO: pick back up here.
  need to decide whether to run ALL pnpm commands thru custom global dir
  or default user home dir. getting inconsistent store locations.
   */

  pub fn pnpm(&self) -> Command {
    let bindir = self.bindir();
    let mut cmd = Command::new(bindir.join("pnpm"));
    cmd.env("PNPM_HOME", &bindir);
    let path = env::var("PATH").unwrap_or_else(|_| String::new());
    cmd.env("PATH", format!("{}:{path}", bindir.display()));
    cmd
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

  let pnpm_url =
    format!("https://github.com/pnpm/pnpm/releases/download/v{version}/pnpm-{platform}-{arch}");
  let mut response = reqwest::blocking::get(pnpm_url).context("Failed to download pnpm")?;

  let mut file = File::create(dst).context("Could not save pnpm binary to file")?;
  {
    let mut writer = BufWriter::new(&mut file);
    response.copy_to(&mut writer)?;
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
      // Binary bundling
      "esbuild",   

      // Types   
      "typescript",

      // Styling
      "sass",
      "esbuild-sass-plugin",
      "resolve",

      // Site bundlig
      "vite",
      "@vitejs/plugin-react",

      // Testing
      "jest",
      "@types/jest",
      "jest-environment-jsdom",       
      "ts-jest",   

      // Linting
      "eslint",
      "eslint-plugin-react",
      "eslint-plugin-react-hooks",
      "@typescript-eslint/eslint-plugin",
      "@typescript-eslint/parser",
      "eslint-plugin-prettier",

      // Formatting
      "prettier",
      "@trivago/prettier-plugin-sort-imports",
    ];

    println!("Installing JS dependencies...");
    let status = config
      .pnpm()
      .args(["install", "--global"])
      .args(PACKAGES)
      .status()?;
    ensure!(status.success(), "pnpm global installation failed");

    Ok(())
  }
}
