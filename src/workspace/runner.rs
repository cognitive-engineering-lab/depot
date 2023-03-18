use anyhow::Result;

use futures::FutureExt;
use log::debug;
use std::{
  cell::Cell,
  collections::{HashMap, HashSet},
  future::Future,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  thread,
};

use crate::logger::LoggerUi;

use super::{package::PackageIndex, PackageCommand, Workspace};

#[derive(Clone, Copy)]
enum TaskStatus {
  Pending,
  Running,
  Finished,
}

struct Task<F: Future<Output = (PackageIndex, Result<()>)>> {
  status: Cell<TaskStatus>,
  future: F,
}

impl Workspace {
  fn spawn_log_thread(&self) -> impl FnOnce() {
    let ws = self.clone();
    let should_exit = Arc::new(AtomicBool::new(false));
    let should_exit_ref = Arc::clone(&should_exit);
    let log_thread = thread::spawn(move || {
      let inner = || -> Result<()> {
        let mut terminal = ws.terminal.lock().unwrap();
        let mut ui = LoggerUi::new(&ws, &mut terminal);
        ui.setup()?;
        loop {
          if should_exit_ref.load(Ordering::SeqCst) {
            break;
          }
          ui.draw()?;
        }
        ui.cleanup()?;
        Ok(())
      };
      if let Err(e) = inner() {
        eprintln!("{e}");
        std::process::exit(1);
      }
    });

    move || {
      should_exit.store(true, Ordering::SeqCst);
      log_thread.join().unwrap();
    }
  }

  pub async fn run(&self, cmd: impl PackageCommand) -> Result<()> {
    let ignore_deps = cmd.ignore_dependencies();
    let roots = self.packages.clone();
    let pkgs = roots
      .iter()
      .flat_map(|root| self.dep_graph.all_deps_for(root.index))
      .collect::<HashSet<_>>();

    let cmd = Arc::new(cmd);
    let mut tasks = pkgs
      .into_iter()
      .map(|idx| {
        let pkg = &self.packages[idx];
        let cmd = Arc::clone(&cmd);
        (
          idx,
          Task {
            status: Cell::new(TaskStatus::Pending),
            future: (async move { (idx, cmd.run(pkg).await) }).boxed(),
          },
        )
      })
      .collect::<HashMap<_, _>>();

    let cleanup_logs = self.spawn_log_thread();

    let result = loop {
      if tasks
        .iter()
        .all(|(_, task)| matches!(task.status.get(), TaskStatus::Finished))
      {
        break Ok(());
      }

      let pending = tasks
        .iter()
        .filter(|(_, task)| matches!(task.status.get(), TaskStatus::Pending));
      for (index, task) in pending {
        let deps_finished = ignore_deps
          || self
            .dep_graph
            .immediate_deps_for(*index)
            .all(|dep_index| matches!(tasks[&dep_index].status.get(), TaskStatus::Finished));
        if deps_finished {
          debug!("Starting task for: {}", self.packages[*index].name);
          task.status.set(TaskStatus::Running);
        }
      }

      let running = tasks
        .iter_mut()
        .filter(|(_, task)| matches!(task.status.get(), TaskStatus::Running))
        .map(|(_, task)| &mut task.future);
      let ((index, result), _, _) = futures::future::select_all(running).await;
      if result.is_err() {
        break result;
      }
      tasks[&index].status.set(TaskStatus::Finished);
      log::debug!("Finished task for: {}", self.packages[index].name);
    };

    cleanup_logs();

    result
  }
}
