use ansi_to_tui::IntoText;
use anyhow::{Context, Result};
use crossterm::{
  event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
  execute,
  style::{Color, ResetColor, SetForegroundColor},
  terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use futures::StreamExt;
use ratatui::{
  layout::{Constraint, Direction, Layout},
  style::{Modifier, Style},
  text::{Span, Spans, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Widget},
};
use std::{
  io::{Stdout, Write},
  sync::{
    atomic::{AtomicIsize, Ordering},
    Arc, Mutex,
  },
  time::Duration,
};
use tokio::sync::Notify;

use crate::workspace::{process::Process, Workspace};

pub struct FullscreenRenderer {
  terminal: Mutex<Terminal>,
  selected: AtomicIsize,
}

const TICK_RATE: Duration = Duration::from_millis(33);

pub type TerminalBackend = ratatui::backend::CrosstermBackend<Stdout>;
pub type Terminal = ratatui::Terminal<TerminalBackend>;

impl FullscreenRenderer {
  pub fn new() -> Result<Self> {
    let stdout = std::io::stdout();
    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend).context("Failed to initialize terminal")?;

    crossterm::terminal::enable_raw_mode()?;
    execute!(
      terminal.backend_mut(),
      EnterAlternateScreen,
      EnableMouseCapture
    )?;
    terminal.clear()?;

    Ok(FullscreenRenderer {
      terminal: Mutex::new(terminal),
      selected: AtomicIsize::new(0),
    })
  }

  fn build_tabs(ws: &Workspace, selected: usize) -> Option<Tabs> {
    ws.monorepo.then(|| {
      let titles = ws
        .packages
        .iter()
        .enumerate()
        .map(|(i, pkg)| {
          let pkg_name = pkg.name.to_string();
          let mut style = Style::default();
          if i == selected {
            style = style.add_modifier(Modifier::BOLD);
          }
          Spans::from(vec![Span::styled(pkg_name, style)])
        })
        .collect::<Vec<_>>();
      Tabs::new(titles)
    })
  }

  fn build_process_pane(process: &Process) -> impl Widget + '_ {
    let mut spans = Vec::new();
    for line in process.stdout().iter() {
      match line.into_text() {
        Ok(text) => spans.extend(text.lines),
        Err(e) => spans.push(Spans::from(Span::raw(format!(
          "failed to parse line with error: {e:?}"
        )))),
      }
    }
    let text = Paragraph::new(Text::from(spans));
    text.block(
      Block::default()
        .title(process.script())
        .borders(Borders::ALL),
    )
  }

  fn build_package_pane(processes: &[Arc<Process>]) -> Vec<impl Widget + '_> {
    processes
      .iter()
      .map(|process| Self::build_process_pane(process))
      .collect::<Vec<_>>()
  }

  fn render_widgets(&self, tabs: Option<Tabs>, package_pane: Vec<impl Widget>) -> Result<()> {
    let mut terminal = self.terminal.lock().unwrap();
    terminal.draw(|f| {
      let size = f.size();
      let constraints = if tabs.is_some() {
        vec![Constraint::Min(0), Constraint::Length(2)]
      } else {
        vec![]
      };
      let canvas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(size);

      if let Some(tabs) = tabs {
        f.render_widget(tabs, canvas[1]);
      }

      let log_halves = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Ratio(7, 10), Constraint::Ratio(3, 10)])
        .split(canvas[0]);
      let log_slots = log_halves.iter().flat_map(|half| {
        Layout::default()
          .direction(Direction::Horizontal)
          .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
          .split(*half)
          .to_vec()
      });

      for (process, slot) in package_pane.into_iter().zip(log_slots) {
        f.render_widget(process, slot);
      }
    })?;
    Ok(())
  }
}

#[async_trait::async_trait]
impl Renderer for FullscreenRenderer {
  fn render(&self, ws: &Workspace) -> Result<()> {
    let n = ws.packages.len() as isize;
    let selected_unbounded = self.selected.load(Ordering::SeqCst);
    let selected = ((n + selected_unbounded % n) % n) as usize;
    let pkg = &ws.packages[selected];
    let processes = pkg.processes();

    let tabs = Self::build_tabs(ws, selected);
    let package = Self::build_package_pane(&processes);
    self.render_widgets(tabs, package)?;

    Ok(())
  }

  // TODO: This still occasionally drops inputs, seems to conflict with async-process.
  // See the note on `crossterm` dependency in Cargo.toml.
  // Maybe we should try to spawn this future in a separate thread?
  async fn handle_input(&self) -> Result<bool> {
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

  fn complete(self, ws: &Workspace) -> Result<()> {
    let mut terminal = self.terminal.into_inner()?;

    crossterm::terminal::disable_raw_mode()?;
    execute!(
      terminal.backend_mut(),
      LeaveAlternateScreen,
      DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    let inline_renderer = InlineRenderer::new()?;
    inline_renderer.complete(ws)?;

    Ok(())
  }
}

#[async_trait::async_trait]
pub trait Renderer: Sized + Send + Sync {
  fn render(&self, ws: &Workspace) -> Result<()>;
  fn complete(self, ws: &Workspace) -> Result<()>;

  async fn handle_input(&self) -> Result<bool> {
    Ok(false)
  }

  async fn render_loop(mut self, ws: &Workspace, should_exit: &Arc<Notify>) -> Result<bool> {
    let exit_early = {
      let this = &self;

      let input_future = this.handle_input();
      tokio::pin!(input_future);

      let draw_future = async move {
        loop {
          this.render(ws).unwrap();
          tokio::time::sleep(TICK_RATE).await;
        }
      };
      tokio::pin!(draw_future);

      let exit_future = should_exit.notified();
      tokio::pin!(exit_future);

      loop {
        tokio::select! { biased;
          _ = &mut exit_future => break false,
          result = &mut input_future => {
            if result? {
              break true;
            }
          },
          _ = &mut draw_future => {}
        }
      }
    };

    self.complete(ws)?;

    Ok(exit_early)
  }
}

// Clone of pnpm output format
pub struct InlineRenderer {
  diff: Mutex<ansi_diff::Diff>,
}

impl InlineRenderer {
  pub fn new() -> Result<Self> {
    let (w, h) = crossterm::terminal::size()?;
    let diff = Mutex::new(ansi_diff::Diff::new((w as u32, h as u32)));
    Ok(InlineRenderer { diff })
  }

  fn build_output(ws: &Workspace) -> Result<String> {
    let mut output = Vec::new();

    macro_rules! meta {
      ($($arg:tt)*) => {
        execute!(output, SetForegroundColor(Color::Magenta))?;
        write!(output, $($arg),*)?;
        execute!(output, ResetColor)?;
      }
    }

    for pkg in &ws.packages {
      let processes = pkg.processes();
      if processes.is_empty() {
        continue;
      }

      if ws.monorepo {
        writeln!(&mut output, "{}", pkg.name)?;
      }

      for (j, process) in processes.iter().enumerate() {
        let last_process = j == processes.len() - 1;
        if ws.monorepo {
          if last_process {
            meta!("└─ ");
          } else {
            meta!("├─ ");
          }
        }
        writeln!(&mut output, "{}", process.script())?;

        let monorepo_prefix = if ws.monorepo {
          if last_process {
            "   "
          } else {
            "│  "
          }
        } else {
          ""
        };

        let stdout = process.stdout();
        for line in stdout.iter() {
          meta!("{monorepo_prefix}│ ");
          writeln!(&mut output, "{line}")?;
        }
        let status = if process.finished() {
          "finished"
        } else {
          "running..."
        };

        meta!("{monorepo_prefix}└─ {status}\n");
      }
    }

    Ok(String::from_utf8(output)?)
  }
}

impl Renderer for InlineRenderer {
  fn render(&self, ws: &Workspace) -> Result<()> {
    let output = Self::build_output(ws)?;
    print!("{}", self.diff.lock().unwrap().update(&output));
    std::io::stdout().flush()?;
    Ok(())
  }

  fn complete(self, ws: &Workspace) -> Result<()> {
    self.render(ws)
  }
}
