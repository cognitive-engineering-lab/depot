use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen},
  tty::IsTty,
};
use futures::StreamExt;
use std::{
  io::stdout,
  sync::{
    atomic::{AtomicIsize, Ordering},
    MutexGuard,
  },
  time::Duration,
};
use tokio::sync::Notify;
use tui::{
  layout::{Constraint, Direction, Layout},
  style::{Modifier, Style},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Widget},
};

use crate::workspace::{Terminal, Workspace};

use super::Logger;

pub struct LoggerUi {
  selected: AtomicIsize,
}

const TICK_RATE: Duration = Duration::from_millis(33);
const BINARY_ORDER: &[&str] = &["vite", "pnpm", "tsc", "eslint"];

impl LoggerUi {
  fn new() -> Self {
    LoggerUi {
      selected: AtomicIsize::new(0),
    }
  }

  fn setup(mut terminal: MutexGuard<'_, Terminal>) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    execute!(
      terminal.backend_mut(),
      EnterAlternateScreen,
      EnableMouseCapture
    )?;
    terminal.clear()?;
    Ok(())
  }

  fn cleanup(mut terminal: MutexGuard<'_, Terminal>) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    execute!(
      terminal.backend_mut(),
      LeaveAlternateScreen,
      DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
  }

  // TODO: This still occasionally drops inputs, seems to conflict with async-process.
  // See the note on `crossterm` dependency in Cargo.toml.
  pub async fn handle_input(&self) -> Result<bool> {
    let mut reader = crossterm::event::EventStream::new();
    while let Some(event) = reader.next().await {
      if let Event::Key(key) = event? {
        match key.code {
          KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => return Ok(true),
          KeyCode::Left => {
            self.selected.fetch_sub(1, Ordering::SeqCst);
          }
          KeyCode::Right => {
            self.selected.fetch_add(1, Ordering::SeqCst);
          }
          _ => {}
        }
      }
    }
    Ok(false)
  }

  fn log_widget<'b>(logger: &'b Logger, pkg: usize, binary: &'b str) -> impl Widget + 'b {
    let log = &logger.logs[&pkg][binary];

    let mut spans = Vec::new();
    for line in log.iter() {
      match line.into_text() {
        Ok(text) => spans.extend(text.lines),
        Err(e) => spans.push(Spans::from(Span::raw(format!(
          "failed to parse line with error: {e:?}"
        )))),
      }
    }
    let text = Paragraph::new(Text::from(spans));
    text.block(Block::default().title(binary).borders(Borders::ALL))
  }

  pub fn draw(&self, ws: &Workspace) -> Result<()> {
    let mut terminal = ws.terminal();
    let logger = ws.logger.lock().unwrap();
    let mut indices = logger.logs.keys().copied().collect::<Vec<_>>();
    indices.sort();

    let n = indices.len() as isize;
    if n == 0 {
      return Ok(());
    }

    let selected = self.selected.load(Ordering::SeqCst);
    let index = ((n + selected % n) % n) as usize;
    let pkg = indices[index];

    let titles = indices
      .iter()
      .enumerate()
      .map(|(i, pkg)| {
        let pkg_name = ws.packages[*pkg].name.to_string();
        let mut style = Style::default();
        if i == index {
          style = style.add_modifier(Modifier::BOLD);
        }
        Spans::from(vec![Span::styled(pkg_name, style)])
      })
      .collect::<Vec<_>>();

    let mut log_keys = logger.logs[&pkg].keys().collect::<Vec<_>>();
    log_keys.sort_by_key(|s| {
      BINARY_ORDER
        .iter()
        .position(|other| s == other)
        .unwrap_or(usize::MAX)
    });
    let logs = log_keys
      .into_iter()
      .map(|binary| LoggerUi::log_widget(&logger, pkg, binary))
      .collect::<Vec<_>>();

    terminal.draw(|f| {
      let size = f.size();
      let canvas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)].as_ref())
        .split(size);

      let tabs = Tabs::new(titles);
      f.render_widget(tabs, canvas[1]);

      let log_halves = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(7, 10), Constraint::Ratio(3, 10)])
        .split(canvas[0]);
      let log_slots = log_halves.into_iter().flat_map(|half| {
        Layout::default()
          .direction(Direction::Horizontal)
          .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
          .split(half)
      });

      for (log, slot) in logs.into_iter().zip(log_slots) {
        f.render_widget(log, slot);
      }
    })?;

    Ok(())
  }

  async fn draw_loop(&self, ws: &Workspace) -> Result<()> {
    loop {
      self.draw(ws)?;
      tokio::time::sleep(TICK_RATE).await;
    }
  }
}

pub async fn render(ws: &Workspace, should_exit: &Notify) -> Result<()> {
  if !stdout().is_tty() {
    return Ok(());
  }

  let ui = LoggerUi::new();
  LoggerUi::setup(ws.terminal())?;

  let exit_future = should_exit.notified();
  tokio::pin!(exit_future);

  let input_future = ui.handle_input();
  tokio::pin!(input_future);

  let draw_future = ui.draw_loop(ws);
  tokio::pin!(draw_future);

  let exit_early = loop {
    tokio::select! { biased;
      _ = &mut exit_future => break false,
      result = &mut input_future => {
        if result? {
          break true;
        }
      },
      result = &mut draw_future => {
        result?;
      }
    }
  };

  LoggerUi::cleanup(ws.terminal())?;

  if exit_early {
    // TODO: don't call process::exit, we need to cleanup processes first
    std::process::exit(1);
  }

  Ok(())
}
