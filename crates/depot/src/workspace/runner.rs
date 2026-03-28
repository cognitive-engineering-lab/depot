use anyhow::Result;

use futures::{FutureExt, future::BoxFuture};
use log::debug;
use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    sync::{Arc, atomic::Ordering},
};
use tokio::sync::Notify;

use crate::{
    logger::ui::{FullscreenRenderer, InlineRenderer, Renderer},
    shareable,
};

use super::{
    Command, CommandGraph, CommandInner, CommandRuntime, Workspace, build_command_graph,
    dep_graph::DepGraph,
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
    key: String,
    command: Command,
    deps: Vec<String>,
    status: AtomicTaskStatus,
    can_skip: bool,
}

shareable!(Task, TaskInner);

impl Task {
    fn make<F: Future<Output = Result<()>> + Send + 'static>(
        key: String,
        command: Command,
        fut: F,
        deps: Vec<String>,
        can_skip: bool,
    ) -> (Self, TaskFuture) {
        let task = Task::new(TaskInner {
            key,
            command,
            deps,
            can_skip,
            status: AtomicTaskStatus::new(TaskStatus::Pending),
        });
        let task2 = task.clone();
        let boxed_fut = Box::new(move || {
            async move {
                let result = fut.await;
                (result, task2)
            }
            .boxed()
        });
        (task, boxed_fut)
    }
}

impl TaskInner {
    fn key(&self) -> &str {
        &self.key
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
        runtime: Option<CommandRuntime>,
    ) -> impl Future {
        let ws = self.clone();
        let log_should_exit = Arc::clone(log_should_exit);
        let runner_should_exit = Arc::clone(runner_should_exit);
        let use_fullscreen_renderer =
            !ws.common.no_fullscreen && matches!(runtime, Some(CommandRuntime::RunForever));
        tokio::spawn(async move {
            let result = if use_fullscreen_renderer {
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
        runtime: Option<CommandRuntime>,
    ) -> (TaskGraph, HashMap<Task, TaskFuture>) {
        let futures = RefCell::new(HashMap::new());
        let task_pool = RefCell::new(HashMap::new());

        let tasks_for = |cmd: &Command| -> Vec<Task> {
            macro_rules! add_task {
                ($key:expr, $task:expr, $deps:expr, $files:expr) => {{
                    task_pool
                        .borrow_mut()
                        .entry($key.clone())
                        .or_insert_with(|| {
                            let can_skip = self.common.incremental
                                && !matches!(runtime, Some(CommandRuntime::RunForever))
                                && match $files {
                                    Some(files) => {
                                        let fingerprints = self.fingerprints.read().unwrap();
                                        fingerprints.can_skip(&$key, files)
                                    }
                                    None => false,
                                };

                            let (task, future) =
                                Task::make($key, cmd.clone(), $task, $deps, can_skip);
                            futures.borrow_mut().insert(task.clone(), future);
                            task
                        })
                        .clone()
                }};
            }

            match &**cmd {
                CommandInner::Package(pkg_cmd) => self
                    .roots
                    .iter()
                    .flat_map(|pkg| {
                        self.pkg_graph.all_deps_for(pkg).chain([pkg]).map(|pkg| {
                            let pkg = pkg.clone();
                            let key = pkg_cmd.pkg_key(&pkg);
                            let deps = self
                                .pkg_graph
                                .immediate_deps_for(&pkg)
                                .map(|pkg| pkg_cmd.pkg_key(pkg))
                                .collect();
                            let files = pkg.all_files().collect::<Vec<_>>();
                            add_task!(key, cmd.clone().run_pkg(pkg), deps, Some(files))
                        })
                    })
                    .collect(),
                CommandInner::Workspace(ws_cmd) => {
                    let this = self.clone();
                    let key = ws_cmd.ws_key();
                    let deps = vec![];
                    let files = ws_cmd.input_files(self);
                    vec![add_task!(key, cmd.clone().run_ws(this), deps, files)]
                }
            }
        };

        let task_graph = DepGraph::build(
            cmd_graph.roots().flat_map(tasks_for).collect(),
            |t| t.key.clone(),
            |task: &Task| {
                let mut deps = cmd_graph
                    .immediate_deps_for(&task.command)
                    .flat_map(tasks_for)
                    .collect::<Vec<_>>();
                let runtime = task.command.runtime();
                if let Some(CommandRuntime::WaitForDependencies) = runtime {
                    deps.extend(task.deps.iter().map(|key| task_pool.borrow()[key].clone()));
                }
                deps
            },
        )
        .unwrap();

        (task_graph, futures.into_inner())
    }

    pub async fn run(&self, root: Command) -> Result<()> {
        let runtime = root.runtime();
        let cmd_graph = build_command_graph(&root);
        let (task_graph, mut task_futures) = self.build_task_graph(&cmd_graph, runtime);

        let log_should_exit: Arc<Notify> = Arc::new(Notify::new());
        let runner_should_exit: Arc<Notify> = Arc::new(Notify::new());

        let runner_should_exit_fut = runner_should_exit.notified();
        tokio::pin!(runner_should_exit_fut);

        let cleanup_logs = self.spawn_log_thread(&log_should_exit, &runner_should_exit, runtime);

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
                let imm_deps = task_graph.immediate_deps_for(task).collect::<Vec<_>>();
                let deps_finished = imm_deps
                    .iter()
                    .all(|dep| dep.status() == TaskStatus::Finished);
                if deps_finished {
                    let can_skip = task.can_skip && imm_deps.iter().all(|dep| dep.can_skip);
                    let task_fut = task_futures.remove(task).unwrap();
                    if can_skip {
                        task.status.store(TaskStatus::Finished, Ordering::SeqCst);
                    } else {
                        debug!("Starting task for: {}", task.key());
                        task.status.store(TaskStatus::Running, Ordering::SeqCst);
                        running_futures.push(tokio::spawn(task_fut()));
                    }
                }
            }

            if running_futures.is_empty() {
                continue;
            }

            let one_output = futures::future::select_all(&mut running_futures);
            let (result, idx, _) = tokio::select! { biased;
              () = &mut runner_should_exit_fut => break Ok(()),
              output = one_output => output,
            };

            running_futures.remove(idx);

            let (result, completed_task) = result?;

            if result.is_err() {
                break result;
            }

            debug!("Finishing task for: {}", completed_task.key());
            completed_task
                .status
                .store(TaskStatus::Finished, Ordering::SeqCst);
            self.fingerprints
                .write()
                .unwrap()
                .update_time(completed_task.key().to_string());
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

        if root.name() != "clean" {
            self.fingerprints.read().unwrap().save(&self.root)?;
        }

        result
    }
}
