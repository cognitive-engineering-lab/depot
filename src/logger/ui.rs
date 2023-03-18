use ansi_to_tui::IntoText;
use anyhow::Result;
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
  execute,
  terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use std::time::{Duration, Instant};
use tui::{
  layout::{Constraint, Direction, Layout},
  style::{Modifier, Style},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Widget},
};

use crate::workspace::{Terminal, Workspace};

use super::Logger;

pub struct LoggerUi<'a> {
  ws: &'a Workspace,
  terminal: &'a mut Terminal,
  selected: isize,
  last_tick: Instant,
}

const TICK_RATE: Duration = Duration::from_millis(33);
const BINARY_ORDER: &[&str] = &["vite", "pnpm", "tsc", "eslint"];

impl<'a> LoggerUi<'a> {
  pub fn new(ws: &'a Workspace, terminal: &'a mut Terminal) -> Self {
    LoggerUi {
      ws,
      terminal,
      selected: 0,
      last_tick: Instant::now(),
    }
  }

  pub fn setup(&mut self) -> Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    execute!(
      self.terminal.backend_mut(),
      EnterAlternateScreen,
      EnableMouseCapture
    )?;
    self.terminal.clear()?;
    Ok(())
  }

  pub fn cleanup(&mut self) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    execute!(
      self.terminal.backend_mut(),
      LeaveAlternateScreen,
      DisableMouseCapture
    )?;
    self.terminal.show_cursor()?;
    Ok(())
  }

  pub fn handle_input(&mut self) -> Result<()> {
    let timeout = TICK_RATE
      .checked_sub(self.last_tick.elapsed())
      .unwrap_or_else(|| Duration::from_secs(0));
    if crossterm::event::poll(timeout)? {
      if let Event::Key(key) = crossterm::event::read()? {
        match key.code {
          KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            self.cleanup()?;
            std::process::exit(1)
          }
          KeyCode::Left => self.selected += 1,
          KeyCode::Right => self.selected -= 1,
          _ => {}
        }
      }
    }
    Ok(())
  }

  fn log_widget<'b>(&self, logger: &'b Logger, pkg: usize, binary: &'b str) -> impl Widget + 'b {
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

  pub fn draw(&mut self) -> Result<()> {
    self.handle_input()?;

    if self.last_tick.elapsed() < TICK_RATE {
      return Ok(());
    }

    let logger = self.ws.logger.lock().unwrap();
    let mut indices = logger.logs.keys().copied().collect::<Vec<_>>();
    indices.sort();

    let n = indices.len() as isize;
    if n == 0 {
      return Ok(());
    }

    let index = ((n + self.selected % n) % n) as usize;
    let pkg = indices[index];

    let titles = indices
      .iter()
      .enumerate()
      .map(|(i, pkg)| {
        let pkg_name = self.ws.packages[*pkg].name.to_string();
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
      .map(|binary| self.log_widget(&logger, pkg, binary))
      .collect::<Vec<_>>();

    self.terminal.draw(|f| {
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

    self.last_tick = Instant::now();

    Ok(())
  }
}
