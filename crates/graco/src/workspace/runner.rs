use anyhow::Result;

use futures::FutureExt;
use log::debug;
use std::{
  cell::Cell,
  collections::{HashMap, HashSet},
  future::Future,
  sync::Arc,
};
use tokio::sync::Notify;

use crate::logger::ui;

use super::{package::PackageIndex, PackageCommand, Workspace, WorkspaceCommand};

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
  fn spawn_log_thread(&self, should_exit: &Arc<Notify>) -> impl Future {
    let ws = self.clone();
    let should_exit = Arc::clone(should_exit);
    tokio::spawn(async move {
      let result = ui::render(&ws, &should_exit).await;
      if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(1);
      }
    })
  }

  pub async fn run_both(&self, cmd: &(impl WorkspaceCommand + PackageCommand)) -> Result<()> {
    self.run_ws(cmd).await?;
    self.run_pkgs(cmd).await?;
    Ok(())
  }

  pub async fn run_ws(&self, cmd: &impl WorkspaceCommand) -> Result<()> {
    cmd.run(self).await?;
    Ok(())
  }

  pub async fn run_pkgs(&self, cmd: &impl PackageCommand) -> Result<()> {
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
        let future = async move {
          let result = cmd.run(pkg).await;
          (idx, result)
        };
        (
          idx,
          Task {
            status: Cell::new(TaskStatus::Pending),
            future: future.boxed(),
          },
        )
      })
      .collect::<HashMap<_, _>>();

    let should_exit = Arc::new(Notify::new());
    let cleanup_logs = self.spawn_log_thread(&should_exit);

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

    log::debug!("All tasks complete, waiting for log thread to exit");
    should_exit.notify_one();
    cleanup_logs.await;

    log::debug!("Flushing logs");
    ui::complete(self)?;

    result
  }
}
