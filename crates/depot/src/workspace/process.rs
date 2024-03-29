use std::{
  process::{ExitStatus, Stdio},
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, MutexGuard,
  },
};
use tokio::{
  io::{AsyncBufReadExt, AsyncRead, BufReader},
  task::JoinHandle,
};

use anyhow::{bail, ensure, Context, Result};

use crate::logger::ringbuffer::RingBuffer;

#[derive(Copy, Clone)]
pub enum OutputChannel {
  Stdout,
  Stderr,
}

pub struct LogLine {
  pub line: String,
  pub channel: OutputChannel,
}

pub type LogBuffer = RingBuffer<LogLine>;

pub struct Process {
  script: String,
  child: Mutex<Option<tokio::process::Child>>,
  logs: Arc<Mutex<LogBuffer>>,
  finished: AtomicBool,

  // TODO: is it necessary to abort these handles?
  #[allow(unused)]
  pipe_handles: Mutex<Vec<JoinHandle<()>>>,
}

impl Process {
  pub fn new(script: String, mut cmd: tokio::process::Command) -> Result<Self> {
    cmd.kill_on_drop(true);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
      .spawn()
      .with_context(|| format!("Failed to spawn process: `{}`", script))?;

    let logs: Arc<Mutex<RingBuffer<LogLine>>> = Arc::new(Mutex::new(RingBuffer::new()));
    let pipe_handles = vec![
      tokio::spawn(Self::pipe_stdio(
        child.stdout.take().unwrap(),
        logs.clone(),
        OutputChannel::Stdout,
      )),
      tokio::spawn(Self::pipe_stdio(
        child.stderr.take().unwrap(),
        logs.clone(),
        OutputChannel::Stderr,
      )),
    ];

    Ok(Process {
      script,
      child: Mutex::new(Some(child)),
      logs,
      finished: AtomicBool::new(false),
      pipe_handles: Mutex::new(pipe_handles),
    })
  }

  async fn pipe_stdio(
    stdio: impl AsyncRead + Unpin,
    buffer: Arc<Mutex<LogBuffer>>,
    channel: OutputChannel,
  ) {
    let mut lines = BufReader::new(stdio).lines();
    while let Some(line) = lines.next_line().await.unwrap() {
      let mut buffer = buffer.lock().unwrap();
      let line = match line.strip_prefix("\u{1b}c") {
        Some(rest) => {
          buffer.clear();
          rest.to_string()
        }
        None => line,
      };
      buffer.push(LogLine { line, channel });
    }
  }

  pub fn script(&self) -> &str {
    &self.script
  }

  pub fn stdout(&self) -> MutexGuard<'_, LogBuffer> {
    self.logs.lock().unwrap()
  }

  pub fn finished(&self) -> bool {
    self.finished.load(Ordering::SeqCst)
  }

  pub async fn wait(&self) -> Result<ExitStatus> {
    let mut child = self.child.lock().unwrap().take().unwrap();

    let status_res = child
      .wait()
      .await
      .with_context(|| format!("Process `{}` failed", self.script));

    self.finished.store(true, Ordering::SeqCst);

    status_res
  }

  pub async fn wait_for_success(&self) -> Result<()> {
    let status = self.wait().await?;
    match status.code() {
      Some(code) => ensure!(
        status.success(),
        "Process `{}` exited with non-zero exit code: {code}",
        self.script
      ),
      None => bail!("Process `{}` exited due to signal", self.script),
    }
    Ok(())
  }
}
