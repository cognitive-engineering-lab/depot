use ansi_parser::AnsiParser;
use anyhow::Result;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use std::{
  collections::HashMap,
  sync::Mutex,
  time::{Duration, Instant},
};
use tui::{
  layout::{Constraint, Direction, Layout},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs},
};

use crate::workspace::{
  package::{Package, PackageIndex},
  Terminal, Workspace,
};

use self::logbuffer::LogBuffer;

mod logbuffer;

pub struct Logger {
  logs: HashMap<PackageIndex, HashMap<String, LogBuffer>>,
}

impl Logger {
  pub fn new() -> Result<Self> {
    Ok(Logger {
      logs: HashMap::default(),
    })
  }

  pub fn register_log(&mut self, index: PackageIndex, process: &str) {
    self
      .logs
      .entry(index)
      .or_default()
      .insert(process.to_string(), LogBuffer::new());
  }

  pub fn log(&mut self, index: PackageIndex, process: &str, contents: &[u8]) {
    self
      .logs
      .get_mut(&index)
      .unwrap()
      .get_mut(process)
      .unwrap()
      .push(contents);
  }
}

pub struct LoggerUi<'a> {
  ws: &'a Workspace,
  terminal: &'a mut Terminal,
  selected: isize,
  last_tick: Instant,
}

const TICK_RATE: Duration = Duration::from_millis(33);

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
    self.terminal.clear()?;
    Ok(())
  }

  pub fn cleanup(&mut self) -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
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

  pub fn draw(&mut self) -> Result<()> {
    self.handle_input()?;

    if self.last_tick.elapsed() < TICK_RATE {
      return Ok(());
    }

    let logger = self.ws.logger.lock().unwrap();
    self.terminal.draw(|f| {
      let size = f.size();
      let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)].as_ref())
        .split(size);

      let mut indices = logger.logs.keys().copied().collect::<Vec<_>>();
      indices.sort();
      let titles = indices
        .iter()
        .map(|index| {
          let pkg_name = self.ws.packages[*index].name.to_string();
          Spans::from(vec![Span::raw(pkg_name)])
        })
        .collect::<Vec<_>>();

      let tabs = Tabs::new(titles);
      f.render_widget(tabs, chunks[1]);

      let n = indices.len() as isize;
      let index = (n + self.selected % n) % n;
      let pkg = indices[index as usize];
      let log = &logger.logs[&pkg]["tsc"];
      let (first, second) = log.contents();
      // let bytes = [first, second].concat();
      // TODO: PICK BACK UP FROM HERE

      let text = Paragraph::new(Spans::from(vec![
        Span::raw(String::from_utf8_lossy(first)),
        Span::raw(String::from_utf8_lossy(second)),
      ]));
      f.render_widget(
        text.block(Block::default().title("Content").borders(Borders::ALL)),
        chunks[0],
      );
    })?;

    self.last_tick = Instant::now();

    Ok(())
  }
}
