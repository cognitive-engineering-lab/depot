use anyhow::Result;

use futures::{future::BoxFuture, FutureExt};
use log::debug;
use std::{
  cell::RefCell,
  collections::HashMap,
  future::Future,
  sync::{atomic::Ordering, Arc},
};
use tokio::sync::Notify;

use crate::{
  logger::ui::{FullscreenRenderer, InlineRenderer, Renderer},
  shareable,
};

use super::{
  build_command_graph, dep_graph::DepGraph, package::Package, Command, CommandGraph, CommandInner,
  Workspace,
};

#[atomic_enum::atomic_enum]
#[derive(PartialEq)]
enum TaskStatus {
  Pending = 0,
  Running,
  Finished,
}

type TaskFuture = Box<dyn FnOnce() -> BoxFuture<'static, (Result<()>, Task)>>;

pub struct TaskInner {
  status: AtomicTaskStatus,
  command: Command,
  pkg: Option<Package>,
}

shareable!(Task, TaskInner);

fn task_name(cmd: &Command, pkg: &Option<Package>) -> String {
  let cmd_name = cmd.name();
  match pkg {
    Some(pkg) => format!("{cmd_name}:pkg({})", pkg.name),
    None => format!("{cmd_name}:ws"),
  }
}

impl Task {
  fn make<F: Future<Output = Result<()>> + Send + 'static>(
    command: &Command,
    pkg: Option<Package>,
    mk_future: impl FnOnce(Command) -> F,
  ) -> (Self, TaskFuture) {
    let task = Task::new(TaskInner {
      status: AtomicTaskStatus::new(TaskStatus::Pending),
      pkg,
      command: command.clone(),
    });
    let task2 = task.clone();
    let fut = mk_future(command.clone());
    (
      task,
      Box::new(move || {
        async move {
          let result = fut.await;
          (result, task2)
        }
        .boxed()
      }),
    )
  }
}

impl TaskInner {
  fn name(&self) -> String {
    task_name(&self.command, &self.pkg)
  }

  fn status(&self) -> TaskStatus {
    self.status.load(Ordering::SeqCst)
  }
}

type TaskGraph = DepGraph<Task>;

impl Workspace {
  fn spawn_log_thread(
    &self,
    log_should_exit: &Arc<Notify>,
    runner_should_exit: &Arc<Notify>,
  ) -> impl Future {
    let ws = self.clone();
    let log_should_exit = Arc::clone(log_should_exit);
    let runner_should_exit = Arc::clone(runner_should_exit);
    let watch = self.common.watch;
    tokio::spawn(async move {
      let result = if watch {
        FullscreenRenderer::new()
          .unwrap()
          .render_loop(&ws, &log_should_exit)
          .await
      } else {
        InlineRenderer::new()
          .render_loop(&ws, &log_should_exit)
          .await
      };
      match result {
        Ok(true) => runner_should_exit.notify_one(),
        Ok(false) => {}
        Err(e) => {
          eprintln!("{e}");
          runner_should_exit.notify_one();
        }
      }
    })
  }

  fn build_task_graph(
    &self,
    cmd_graph: &CommandGraph,
  ) -> Result<(TaskGraph, HashMap<Task, TaskFuture>)> {
    let futures = RefCell::new(HashMap::new());
    let task_pool = RefCell::new(HashMap::new());

    let pkg_roots = match &self.common.only {
      Some(name) => vec![self.find_package_by_name(name)?.clone()],
      None => self.packages.clone(),
    };

    let tasks_for = |cmd: &Command| -> Vec<Task> {
      macro_rules! add_task {
        ($pkg:expr, $task:expr) => {{
          let mut task_pool = task_pool.borrow_mut();
          let pkg = $pkg;
          task_pool
            .entry(task_name(cmd, &pkg))
            .or_insert_with(|| {
              let (task, future) = Task::make(cmd, pkg, $task);
              futures.borrow_mut().insert(task.clone(), future);
              task
            })
            .clone()
        }};
      }

      macro_rules! pkg_tasks {
        () => {{
          pkg_roots.iter().flat_map(|pkg| {
            self.pkg_graph.all_deps_for(pkg).chain([pkg]).map(|pkg| {
              let pkg = pkg.clone();
              add_task!(Some(pkg.clone()), move |cmd| cmd.run_pkg(pkg))
            })
          })
        }};
      }

      macro_rules! ws_task {
        () => {{
          let this = self.clone();
          add_task!(None, move |cmd| cmd.run_ws(this))
        }};
      }

      match &**cmd {
        CommandInner::Package(_) => pkg_tasks!().collect(),
        CommandInner::Workspace(_) => vec![ws_task!()],
        CommandInner::Both(_) => {
          // TODO: this semantics makes avoids an issue where
          // non-monorepos have race conditions, e.g. cleaning node_modules
          // twice concurrently. But this solution is hacky
          if self.monorepo {
            pkg_tasks!().chain([ws_task!()]).collect()
          } else {
            pkg_tasks!().collect()
          }
        }
      }
    };

    let task_graph = DepGraph::build(
      cmd_graph.roots().flat_map(|root| tasks_for(root)).collect(),
      |task: Task| {
        let mut deps = cmd_graph
          .immediate_deps_for(&task.command)
          .flat_map(|dep| tasks_for(dep))
          .collect::<Vec<_>>();
        if let Some(pkg) = &task.pkg {
          deps.extend(self.pkg_graph.immediate_deps_for(pkg).map(|pkg| {
            let name = task_name(&task.command, &Some(pkg.clone()));
            task_pool.borrow()[&name].clone()
          }));
        }
        deps
      },
    );

    Ok((task_graph, futures.into_inner()))
  }

  pub async fn run(&self, root: Command) -> Result<()> {
    let cmd_graph = build_command_graph(&root);
    let (task_graph, mut task_futures) = self.build_task_graph(&cmd_graph)?;

    let log_should_exit: Arc<Notify> = Arc::new(Notify::new());
    let runner_should_exit: Arc<Notify> = Arc::new(Notify::new());

    let runner_should_exit_fut = runner_should_exit.notified();
    tokio::pin!(runner_should_exit_fut);

    let cleanup_logs = self.spawn_log_thread(&log_should_exit, &runner_should_exit);

    let mut running_futures = Vec::new();
    let result = loop {
      let finished = task_graph
        .nodes()
        .all(|task| task.status() == TaskStatus::Finished);
      if finished {
        break Ok(());
      }

      let pending = task_graph
        .nodes()
        .filter(|task| task.status() == TaskStatus::Pending);
      for task in pending {
        let deps_finished = task_graph
          .immediate_deps_for(task)
          .all(|dep| dep.status() == TaskStatus::Finished);
        if deps_finished {
          debug!("Starting task for: {}", task.name());
          task.status.store(TaskStatus::Running, Ordering::SeqCst);
          running_futures.push(tokio::spawn((task_futures.remove(task).unwrap())()));
        }
      }

      let one_output = futures::future::select_all(&mut running_futures);
      let (result, idx, _) = tokio::select! { biased;
        _ = &mut runner_should_exit_fut => break Ok(()),
        output = one_output => output,
      };

      running_futures.remove(idx);

      let (result, completed_task) = result?;

      if result.is_err() {
        break result;
      }

      debug!("Finishing task for: {}", completed_task.name());
      completed_task
        .status
        .store(TaskStatus::Finished, Ordering::SeqCst);
    };

    for fut in &mut running_futures {
      fut.abort();
    }

    for fut in &mut running_futures {
      let _ = fut.await;
    }

    log::debug!("All tasks complete, waiting for log thread to exit");
    log_should_exit.notify_one();
    cleanup_logs.await;

    result
  }
}
