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
  prelude::Rect,
  style::{Modifier, Style},
  text::{Line, Span, Text},
  widgets::{Block, Borders, Paragraph, Tabs, Wrap},
};
use std::{
  io::{Stdout, Write},
  sync::{
    Arc, Mutex,
    atomic::{AtomicIsize, Ordering},
  },
  time::Duration,
};
use tokio::sync::Notify;

use crate::workspace::{Workspace, process::Process};

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

  fn build_tabs(ws: &Workspace, selected: usize) -> Option<Tabs<'_>> {
    ws.monorepo.then(|| {
      let titles = ws
        .package_display_order()
        .enumerate()
        .map(|(i, pkg)| {
          let pkg_name = pkg.name.to_string();
          let mut style = Style::default();
          if i == selected {
            style = style.add_modifier(Modifier::BOLD);
          }
          Span::styled(pkg_name, style)
        })
        .collect::<Vec<_>>();
      Tabs::new(titles)
    })
  }

  fn render_process_pane(f: &mut ratatui::Frame, process: &Process, slot: Rect) {
    let mut spans = Vec::new();
    let height = slot.bottom() as usize;
    let stdout = process.stdout();
    let last_lines = stdout.iter().rev().take(height).rev();
    for line in last_lines {
      // TODO: distinguish stdout from stderr
      match line.line.into_text() {
        Ok(text) => spans.extend(text.lines),
        Err(e) => spans.push(Line::from(Span::raw(format!(
          "failed to parse line with error: {e:?}"
        )))),
      }
    }
    let p = Paragraph::new(Text::from(spans))
      .block(
        Block::default()
          .title(process.script())
          .borders(Borders::ALL),
      )
      .wrap(Wrap { trim: false });
    f.render_widget(p, slot);
  }
}

#[async_trait::async_trait]
impl Renderer for FullscreenRenderer {
  fn render(&self, ws: &Workspace) -> Result<()> {
    let n = isize::try_from(ws.pkg_graph.nodes().count()).unwrap();
    let selected_unbounded = self.selected.load(Ordering::SeqCst);
    let selected = usize::try_from((n + selected_unbounded % n) % n).unwrap();
    let pkg = ws.package_display_order().nth(selected).unwrap();
    let processes = pkg.processes();

    let tabs = Self::build_tabs(ws, selected);

    let mut terminal = self.terminal.lock().unwrap();
    terminal.draw(|f| {
      let size = f.area();
      let constraints = if tabs.is_some() {
        vec![Constraint::Min(0), Constraint::Length(2)]
      } else {
        vec![Constraint::Min(0)]
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
      let log_slots = log_halves
        .iter()
        .flat_map(|half| {
          Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .split(*half)
            .to_vec()
        })
        .collect::<Vec<_>>();

      for (process, slot) in processes.iter().zip(log_slots) {
        Self::render_process_pane(f, process, slot);
      }
    })?;

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

    let inline_renderer = InlineRenderer::new();
    inline_renderer.complete(ws)?;

    Ok(())
  }
}

#[async_trait::async_trait]
pub trait Renderer: Sized + Send + Sync {
  fn render(&self, ws: &Workspace) -> Result<()>;
  fn complete(self, ws: &Workspace) -> Result<()>;

  async fn handle_input(&self) -> Result<bool> {
    loop {
      tokio::time::sleep(Duration::MAX).await;
    }
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
          () = &mut exit_future => break false,
          result = &mut input_future => {
            if result? {
              break true;
            }
          },
          () = &mut draw_future => {}
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
  pub fn new() -> Self {
    // TODO: do we need a different rendering strategy if there's no tty?
    let (w, h) = crossterm::terminal::size().unwrap_or((80, 40));
    let diff = Mutex::new(ansi_diff::Diff::new((u32::from(w), u32::from(h))));
    InlineRenderer { diff }
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

    let ws_processes = ws.processes();
    if !ws_processes.is_empty() {
      // TODO: this repeats a lot of code with the block below
      for process in ws_processes.iter() {
        writeln!(&mut output, "ws/{}", process.script())?;

        let stdout = process.stdout();
        for line in stdout.iter() {
          meta!("│ ");
          // TODO: distinguish stdout from stderr
          writeln!(&mut output, "{}", line.line)?;
        }
        let status = if process.finished() {
          "finished"
        } else {
          "running..."
        };

        meta!("└─ {status}\n");
      }
    }

    for pkg in ws.package_display_order() {
      let pkg_processes = pkg.processes();
      if pkg_processes.is_empty() {
        continue;
      }

      if ws.monorepo {
        writeln!(&mut output, "{}", pkg.name)?;
      }

      for (j, process) in pkg_processes.iter().enumerate() {
        let last_process = j == pkg_processes.len() - 1;
        if ws.monorepo {
          if last_process {
            meta!("└─ ");
          } else {
            meta!("├─ ");
          }
        }
        writeln!(&mut output, "{}", process.script())?;

        let monorepo_prefix = if ws.monorepo {
          if last_process { "   " } else { "│  " }
        } else {
          ""
        };

        let stdout = process.stdout();
        for line in stdout.iter() {
          meta!("{monorepo_prefix}│ ");
          // TODO: distinguish stdout from stderr
          writeln!(&mut output, "{}", line.line)?;
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
