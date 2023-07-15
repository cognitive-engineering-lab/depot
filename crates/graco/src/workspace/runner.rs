use anyhow::{Context, Result};

use futures::{future::BoxFuture, FutureExt};
use log::debug;
use std::{
  cell::{Cell, RefCell},
  collections::{HashMap, HashSet},
  future::Future,
  sync::{atomic::Ordering, Arc, Mutex},
};
use tokio::sync::Notify;

use crate::{
  logger::ui::{FullscreenRenderer, InlineRenderer, Renderer},
  shareable,
};

use super::{
  build_command_graph,
  dep_graph::DepGraph,
  package::{Package, PackageGraph, PackageName},
  Command, CommandGraph, CommandInner, PackageCommand, Workspace, WorkspaceCommand,
};

#[atomic_enum::atomic_enum]
#[derive(PartialEq)]
enum TaskStatus {
  Pending = 0,
  Running,
  Finished,
}

type TaskFuture = BoxFuture<'static, (Result<()>, Task)>;

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
      async move {
        let result = fut.await;
        (result, task2)
      }
      .boxed(),
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
          .unwrap()
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
    let mut futures: RefCell<
      HashMap<
        Task,
        std::pin::Pin<
          Box<dyn Future<Output = (std::result::Result<(), anyhow::Error>, Task)> + Send>,
        >,
      >,
    > = RefCell::new(HashMap::new());
    let mut task_pool = RefCell::new(HashMap::new());

    let pkg_roots = match &self.common.only {
      Some(name) => vec![self.find_package_by_name(&name)?.clone()],
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
        CommandInner::Both(_) => pkg_tasks!().chain([ws_task!()]).collect(),
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
        }
      }

      let running_tasks = task_graph
        .nodes()
        .filter(|task| task.status() == TaskStatus::Running)
        .collect::<HashSet<_>>();
      let running_futures = task_futures
        .iter_mut()
        .filter_map(|(task, future)| running_tasks.contains(task).then_some(future));
      let one_output = futures::future::select_all(running_futures);
      let ((result, completed_task), _, _) = tokio::select! { biased;
        _ = &mut runner_should_exit_fut => break Ok(()),
        output = one_output => output,
      };

      if result.is_err() {
        break result;
      }

      debug!("Finishing task for: {}", completed_task.name());
      completed_task
        .status
        .store(TaskStatus::Finished, Ordering::SeqCst);
    };

    log::debug!("All tasks complete, waiting for log thread to exit");
    log_should_exit.notify_one();
    cleanup_logs.await;

    result
  }

  pub async fn run_both(&self, cmd: &(impl WorkspaceCommand + PackageCommand)) -> Result<()> {
    self.run_ws(cmd).await?;
    self.run_pkgs(cmd).await?;
    Ok(())
  }

  pub async fn run_ws(&self, cmd: &impl WorkspaceCommand) -> Result<()> {
    cmd.run_ws(self).await?;
    Ok(())
  }

  pub async fn run_pkgs(&self, cmd: &impl PackageCommand) -> Result<()> {
    todo!()
    // let ignore_deps = cmd.ignore_dependencies();
    // let only_run = cmd.only_run();

    // let roots = match only_run.as_ref() {
    //   Some(name) => {
    //     let only = self
    //       .packages
    //       .iter()
    //       .find(|pkg| &pkg.name == name)
    //       .with_context(|| format!("Could not find package in workspace with name \"{name}\""))?
    //       .clone();
    //     vec![only]
    //   }
    //   None => self.packages.clone(),
    // };
    // log::debug!(
    //   "Roots: {:?}",
    //   roots
    //     .iter()
    //     .map(|pkg| format!("{}", pkg.name))
    //     .collect::<Vec<_>>()
    // );

    // let pkgs = roots
    //   .iter()
    //   .flat_map(|root| self.pkg_graph.all_deps_for(root).chain([root]))
    //   .collect::<HashSet<_>>();

    // let cmd = Arc::new(cmd);
    // let mut tasks = pkgs
    //   .into_iter()
    //   .map(|pkg| {
    //     let cmd = Arc::clone(&cmd);
    //     let pkg_ref = pkg.clone();
    //     let future = async move {
    //       let result = cmd.run(&pkg_ref).await;
    //       (pkg_ref, result)
    //     };
    //     (
    //       pkg,
    //       Task {
    //         status: Cell::new(TaskStatus::Pending),
    //         future: future.boxed(),
    //       },
    //     )
    //   })
    //   .collect::<HashMap<_, _>>();

    // let result = loop {
    //   if tasks
    //     .iter()
    //     .all(|(_, task)| matches!(task.status.get(), TaskStatus::Finished))
    //   {
    //     break Ok(());
    //   }

    //   let pending = tasks
    //     .iter()
    //     .filter(|(_, task)| matches!(task.status.get(), TaskStatus::Pending));
    //   for (index, task) in pending {
    //     let deps_finished = ignore_deps
    //       || self
    //         .pkg_graph
    //         .immediate_deps_for(*index)
    //         .all(|dep_index| matches!(tasks[&dep_index].status.get(), TaskStatus::Finished));
    //     if deps_finished {
    //       debug!("Starting task for: {}", self.packages[*index].name);
    //       task.status.set(TaskStatus::Running);
    //     }
    //   }

    //   let running = tasks
    //     .iter_mut()
    //     .filter(|(_, task)| matches!(task.status.get(), TaskStatus::Running))
    //     .map(|(_, task)| &mut task.future);
    //   let running_fut = futures::future::select_all(running);
    //   let ((index, result), _, _) = tokio::select! { biased;
    //     _ = &mut runner_should_exit_fut => break Ok(()),
    //     output = running_fut => output,
    //   };
    //   if result.is_err() {
    //     break result;
    //   }
    //   tasks[&index].status.set(TaskStatus::Finished);
    //   log::debug!("Finished task for: {}", self.packages[index].name);
    // };

    // result
  }
}
