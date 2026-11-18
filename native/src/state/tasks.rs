//! The background task manager: spawn, track, cancel and drain tasks that
//! outlive the screen that started them, with their states visible to the
//! diagnostics page.

use std::cell::RefCell;
use std::future::Future;

use nsh_core::element::{SharedString, Signal, Spawned};
use nsh_core::prelude::*;

use super::event_bus;

/// The most tracked tasks the log keeps, like the bus ring's bound. Past
/// it the oldest ended record drops first; only when every record is
/// running does the oldest running record drop with the rest.
pub const TASK_LOG_MAX: usize = 50;

/// The tracked tasks, oldest first, ended ones included (the diagnostics
/// page shows the list). One per surface: `global` keeps one value per
/// type.
fn log() -> Signal<Vec<TaskInfo>> {
    global::<Vec<TaskInfo>>()
}

/// A task's lifecycle state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskState {
    /// Its future is running (or waiting at an await point).
    Running,
    /// Its future ran to its end.
    Done,
    /// It was cancelled before its end: its future was dropped.
    Cancelled,
}

impl TaskState {
    /// The name the diagnostics page shows and the bus payload uses,
    /// lower case.
    pub fn as_str(self) -> &'static str {
        match self {
            TaskState::Running => "running",
            TaskState::Done => "done",
            TaskState::Cancelled => "cancelled",
        }
    }
}

/// One tracked task.
#[derive(Clone, Debug, PartialEq)]
pub struct TaskInfo {
    /// The task's id ([`TaskHandle::id`]).
    pub id: u64,
    /// What it is, shown to people: "sample", "sync-feed".
    pub name: SharedString,
    /// Where it is now.
    pub state: TaskState,
    /// When it started (`foundation::time::now_ms`).
    pub started_ms: f64,
    /// When it ended, if it has.
    pub ended_ms: Option<f64>,
}

/// The handle a [`spawn_task`] caller keeps: cancelling is the manager's
/// one control over a task.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TaskHandle {
    /// The task's id, also [`TaskInfo::id`].
    pub id: u64,
}

impl TaskHandle {
    /// Ends the task: its future is dropped, at an await point if it is
    /// waiting there. What a dropped future leaves behind is the future's
    /// own cancellation-safety contract: write tasks so any await may be
    /// their last act.
    pub fn cancel(&self) {
        cancel(self.id);
    }
}

// The live tasks' engine-side handles. Everything here runs on the engine
// thread, where `spawn` and the executor live; the thread ending (the app
// leaving) drops the handles, which cancels what is still running.
thread_local! {
    static LIVE: RefCell<Vec<(u64, Spawned<()>)>> = const { RefCell::new(Vec::new()) };
}

/// Takes the live handle of task `id`, if it has one.
fn take_live(id: u64) -> Option<Spawned<()>> {
    LIVE.with(|live| {
        let mut live = live.borrow_mut();
        let at = live.iter().position(|(live_id, _)| *live_id == id)?;
        Some(live.remove(at).1)
    })
}

/// Records a task's end in the log and on the bus.
fn record(id: u64, state: TaskState, name: SharedString) {
    let ended = crate::foundation::time::now_ms();
    log().update(|tasks| {
        if let Some(info) = tasks.iter_mut().find(|info| info.id == id) {
            info.state = state;
            info.ended_ms = Some(ended);
        }
    });
    event_bus::publish(event_bus::TOPIC_TASK, format!("{} {name}", state.as_str()));
    crate::foundation::logging::log_info(&format!("task {id} {name} {}", state.as_str()));
}

/// A task's future ended by itself: its handle goes (dropping a finished
/// task's handle cancels nothing; the executor has already forgotten it).
fn complete(id: u64) {
    drop(take_live(id));
    let name = task_name(id);
    record(id, TaskState::Done, name);
}

/// Spawns `future` as a tracked task named `name`. Call it where an owner
/// lives (a handler, an effect, the root): like every NSH task, it ends
/// with that owner too, so app-wide tasks belong to the root's context.
/// The future's output is `()` on purpose: a task reports through signals
/// and the bus, not through a value nobody awaits.
pub fn spawn_task(
    name: impl Into<SharedString>,
    future: impl Future<Output = ()> + 'static,
) -> TaskHandle {
    let name = name.into();
    let info = TaskInfo {
        id: crate::foundation::id::next_id(),
        name: name.clone(),
        state: TaskState::Running,
        started_ms: crate::foundation::time::now_ms(),
        ended_ms: None,
    };
    let handle = TaskHandle { id: info.id };
    log().update(|tasks| {
        tasks.push(info);
        if tasks.len() > TASK_LOG_MAX {
            let drop_at = tasks
                .iter()
                .position(|task| task.state != TaskState::Running)
                .unwrap_or(0);
            tasks.remove(drop_at);
        }
    });
    event_bus::publish(event_bus::TOPIC_TASK, format!("running {name}"));
    crate::foundation::logging::log_info(&format!("task {} {name} started", handle.id));

    let spawned = spawn({
        let id = handle.id;
        async move {
            future.await;
            complete(id);
        }
    });
    LIVE.with(|live| live.borrow_mut().push((handle.id, spawned)));
    handle
}

/// The name task `id` was spawned with, else empty.
fn task_name(id: u64) -> SharedString {
    log()
        .with_untracked(|tasks| {
            tasks
                .iter()
                .find(|info| info.id == id)
                .map(|info| info.name.clone())
        })
        .unwrap_or_default()
}

/// Cancels the task `id`: its future is dropped. No-op when nothing lives
/// under that id (already done or cancelled).
pub fn cancel(id: u64) {
    let Some(spawned) = take_live(id) else {
        return;
    };
    let name = task_name(id);
    spawned.cancel();
    record(id, TaskState::Cancelled, name);
}

/// Cancels every running task (drain on quit): how many it ended.
pub fn cancel_all() -> usize {
    let live = LIVE.with(|live| std::mem::take(&mut *live.borrow_mut()));
    let ids: Vec<u64> = live.iter().map(|(id, _)| *id).collect();
    let count = ids.len();
    for (_, spawned) in live {
        spawned.cancel();
    }
    for id in ids {
        record(id, TaskState::Cancelled, task_name(id));
    }
    crate::foundation::logging::log_info(&format!("drained {count} tasks"));
    count
}

/// The tracked tasks, oldest first (the diagnostics page binds to this).
pub fn tasks() -> Signal<Vec<TaskInfo>> {
    log()
}
