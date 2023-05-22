use async_process::Stdio;
use futures::{io::BufReader, AsyncBufReadExt, AsyncRead, StreamExt};
use std::{
  path::Path,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard,
  },
};

use anyhow::{bail, ensure, Context, Result};

use crate::logger::ringbuffer::RingBuffer;

pub struct Process {
  script: String,
  child: Mutex<Option<async_process::Child>>,
  stdout: Arc<Mutex<RingBuffer<String>>>,
  // stderr: Arc<Mutex<RingBuffer<String>>>,
  finished: AtomicBool,
}

impl Process {
  pub fn new(script: &Path, mut cmd: async_process::Command) -> Result<Self> {
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
      .spawn()
      .with_context(|| format!("Failed to spawn process: `{}`", script.display()))?;
    let script = script.file_name().unwrap().to_string_lossy().to_string();

    let stdout = Arc::new(Mutex::new(RingBuffer::new()));
    // let stderr = Arc::new(Mutex::new(RingBuffer::new()));
    tokio::spawn(Self::pipe_stdio(
      child.stdout.take().unwrap(),
      stdout.clone(),
    ));
    tokio::spawn(Self::pipe_stdio(
      child.stderr.take().unwrap(),
      stdout.clone(),
    ));

    Ok(Process {
      script,
      child: Mutex::new(Some(child)),
      stdout,
      // stderr,
      finished: AtomicBool::new(false),
    })
  }

  async fn pipe_stdio(stdio: impl AsyncRead + Unpin, buffer: Arc<Mutex<RingBuffer<String>>>) {
    let mut lines = BufReader::new(stdio).lines();
    while let Some(line) = lines.next().await {
      let mut buffer = buffer.lock().unwrap();
      let line = line.unwrap();
      let line = match line.strip_prefix("\u{1b}c") {
        Some(rest) => {
          buffer.clear();
          rest.to_string()
        }
        None => line,
      };
      buffer.push(line);
    }
  }

  pub fn script(&self) -> &str {
    &self.script
  }

  pub fn stdout(&self) -> MutexGuard<'_, RingBuffer<String>> {
    self.stdout.lock().unwrap()
  }

  // pub fn stderr(&self) -> MutexGuard<'_, RingBuffer<String>> {
  //   self.stderr.lock().unwrap()
  // }

  pub fn finished(&self) -> bool {
    self.finished.load(Ordering::SeqCst)
  }

  pub async fn wait(&self) -> Result<()> {
    let mut child = self.child.lock().unwrap().take().unwrap();

    let status = child
      .status()
      .await
      .with_context(|| format!("Process `{}` failed", self.script))?;

    match status.code() {
      Some(code) => ensure!(
        status.success(),
        "Process `{}` exited with non-zero exit code: {code}",
        self.script
      ),
      None => bail!("Process `{}` exited due to signal", self.script),
    }

    self.finished.store(true, Ordering::SeqCst);

    Ok(())
  }
}
