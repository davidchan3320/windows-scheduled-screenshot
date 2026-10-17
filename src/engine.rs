use crate::{
    config::{Settings, Task},
    schedule::{self, Occurrence, Runtime},
    storage::Store,
};
use anyhow::{Result, bail, ensure};
use chrono::{DateTime, Local, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

pub struct State {
    pub settings: Settings,
    pub runtime: Runtime,
    pub next: BTreeMap<String, Occurrence>,
    pub error: Option<String>,
    pub busy: bool,
    pub manual_capture_running: bool,
    pub manual_capture: Option<ManualCapture>,
    pub shortcut_active: Option<String>,
    pub shortcut_error: Option<String>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualCapture {
    finished_at: DateTime<Utc>,
    success: bool,
    result: String,
    files: Vec<String>,
}
#[derive(Clone)]
pub struct App {
    pub state: Arc<Mutex<State>>,
    pub store: Store,
    pub available: Arc<AtomicBool>,
    pub stopping: Arc<AtomicBool>,
    tx: SyncSender<Event>,
    thread: Arc<Mutex<Option<thread::JoinHandle<()>>>>,
    desktop_epoch: Arc<std::sync::atomic::AtomicU64>,
}
enum Event {
    Wake,
    Manual {
        desktop_epoch: u64,
        output_folder: String,
    },
    Done(Vec<Finished>),
    Stop,
}
#[derive(Clone)]
struct Job {
    task: Task,
    generation: u64,
    manual: bool,
    epoch: u64,
    desktop_epoch: u64,
}
struct Finished {
    job: Job,
    result: Result<Vec<String>>,
}

impl App {
    pub fn start(store: Store) -> Result<Self> {
        let loaded = store.load()?;
        let (tx, rx) = mpsc::sync_channel(32);
        let app = Self {
            state: Arc::new(Mutex::new(State {
                settings: loaded.settings,
                runtime: loaded.runtime,
                next: BTreeMap::new(),
                error: loaded.warning,
                busy: false,
                manual_capture_running: false,
                manual_capture: None,
                shortcut_active: None,
                shortcut_error: None,
            })),
            store,
            available: Arc::new(AtomicBool::new(true)),
            stopping: Arc::new(AtomicBool::new(false)),
            tx,
            thread: Arc::new(Mutex::new(None)),
            desktop_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        };
        {
            let mut s = app.state.lock().unwrap();
            let now = Utc::now();
            let tasks = s.settings.tasks.clone();
            for t in &tasks {
                if t.enabled {
                    schedule::reconcile(t, s.runtime.tasks.entry(t.id.clone()).or_default(), now);
                }
            }
            app.rearm(&mut s, now, true);
            app.finish_stopped(&mut s, now);
            app.store.save_runtime(&s.runtime)?;
        }
        let scheduler = app.clone();
        *app.thread.lock().unwrap() = Some(
            thread::Builder::new()
                .name("scheduler".into())
                .spawn(move || scheduler.run(rx))?,
        );
        Ok(app)
    }
    pub fn snapshot(&self) -> Value {
        let s = self.state.lock().unwrap();
        let tasks: Vec<_> = s.settings.tasks.iter().map(|t| json!({ "id": t.id, "nextExecution": s.next.get(&t.id).map(|o| o.due), "progress": s.runtime.tasks.get(&t.id) })).collect();
        json!({ "settings": s.settings, "tasks": tasks, "busy": s.busy, "captureAllowed": self.available.load(Ordering::SeqCst), "error": s.error, "applicationDirectory": self.store.base, "defaultTask": Task::default(), "shortcutActive": s.shortcut_active, "shortcutError": s.shortcut_error, "manualCaptureRunning": s.manual_capture_running, "manualCapture": s.manual_capture, "manualOutputFolder": crate::output::resolve_folder(&self.store.base, &s.settings.manual_output_folder).ok() })
    }
    pub fn save(&self, mut candidate: Settings, revision: u64) -> Result<Value> {
        candidate.validate()?;
        crate::output::path_for(
            &self.store.base,
            &candidate.manual_capture_task(),
            Local::now(),
            "DISPLAY1",
            1,
        )?;
        for t in &candidate.tasks {
            crate::output::path_for(&self.store.base, t, Local::now(), "DISPLAY1", 1)?;
        }
        let mut s = self.state.lock().unwrap();
        ensure!(
            s.settings.revision == revision,
            "CONFLICT: Settings changed in another window. Reload before saving."
        );
        candidate.revision = revision
            .checked_add(1)
            .ok_or_else(|| anyhow::anyhow!("Revision overflow"))?;
        let mut runtime = s.runtime.clone();
        runtime
            .tasks
            .retain(|id, _| candidate.tasks.iter().any(|t| &t.id == id));
        for t in &candidate.tasks {
            let old = s.settings.tasks.iter().find(|o| o.id == t.id);
            let p = runtime.tasks.entry(t.id.clone()).or_default();
            if t.enabled && old.is_some_and(|o| !o.enabled) {
                let generation = p.generation + 1;
                *p = Default::default();
                p.generation = generation;
            }
            if old != Some(t) {
                p.generation += 1;
            }
            if t.enabled {
                schedule::reconcile(t, p, Utc::now());
            }
        }
        #[cfg(windows)]
        if candidate.start_at_sign_in != s.settings.start_at_sign_in {
            crate::platform::set_startup(candidate.start_at_sign_in, &self.store.base)?;
        }
        if let Err(e) = self
            .store
            .save_runtime(&runtime)
            .and_then(|_| self.store.save_settings(&candidate))
        {
            // A failed settings replacement must not leave rejected progress
            // resets on disk when the preceding runtime write succeeded.
            if let Err(restore) = self.store.save_runtime(&s.runtime) {
                s.settings.paused = true;
                s.next.clear();
                s.error = Some(format!(
                    "Could not restore runtime progress after a failed settings save: {restore:#}"
                ));
            }
            #[cfg(windows)]
            if candidate.start_at_sign_in != s.settings.start_at_sign_in {
                let _ = crate::platform::set_startup(s.settings.start_at_sign_in, &self.store.base);
            }
            return Err(e);
        }
        let shortcut_changed = s.settings.capture_shortcut != candidate.capture_shortcut;
        if shortcut_changed {
            s.shortcut_active = None;
            s.shortcut_error = None;
        }
        s.settings = candidate;
        s.runtime = runtime;
        s.error = None;
        self.rearm(&mut s, Utc::now(), true);
        self.persist(&mut s);
        drop(s);
        #[cfg(windows)]
        if shortcut_changed {
            crate::platform::refresh_capture_shortcut();
        }
        let _ = self.tx.try_send(Event::Wake);
        Ok(self.snapshot())
    }
    pub fn pause(&self, paused: bool, revision: u64) -> Result<Value> {
        let mut settings = self.state.lock().unwrap().settings.clone();
        settings.paused = paused;
        self.save(settings, revision)
    }
    pub fn manual_output_folder(&self) -> Result<std::path::PathBuf> {
        let folder = self
            .state
            .lock()
            .unwrap()
            .settings
            .manual_output_folder
            .clone();
        crate::output::resolve_folder(&self.store.base, &folder)
    }
    pub fn capture_now(&self) -> Result<()> {
        ensure!(
            !self.stopping.load(Ordering::SeqCst),
            "The application is exiting."
        );
        ensure!(
            self.available.load(Ordering::SeqCst),
            "Capture is unavailable while the desktop is locked or disconnected."
        );
        let mut s = self.state.lock().unwrap();
        ensure!(
            !s.busy,
            "A capture is already running. Wait for it to finish."
        );
        s.busy = true; // Reserve before queuing; rapid clicks cannot create a backlog.
        s.manual_capture_running = true;
        let desktop_epoch = self.desktop_epoch.load(Ordering::SeqCst);
        let request = Event::Manual {
            desktop_epoch,
            output_folder: s.settings.manual_output_folder.clone(),
        };
        if self.tx.try_send(request).is_err() {
            s.busy = false;
            s.manual_capture_running = false;
            bail!("Scheduler is busy; try again.");
        }
        Ok(())
    }
    pub fn notify(&self, available: Option<bool>) {
        self.desktop_epoch.fetch_add(1, Ordering::SeqCst);
        if let Some(value) = available {
            self.available.store(value, Ordering::SeqCst);
        }
        // Re-arm immediately under the lock, invalidating pending occurrences.
        let mut s = self.state.lock().unwrap();
        self.rearm(&mut s, Utc::now(), true);
        self.persist(&mut s);
        drop(s);
        let _ = self.tx.try_send(Event::Wake);
    }
    pub fn stop(&self) {
        self.stopping.store(true, Ordering::SeqCst);
        let _ = self.tx.try_send(Event::Stop);
    }
    pub fn shutdown(&self) {
        self.stop();
        if let Some(handle) = self.thread.lock().unwrap().take() {
            let _ = handle.join();
        }
    }
    fn persist(&self, s: &mut State) -> bool {
        if let Err(e) = self.store.save_runtime(&s.runtime) {
            s.settings.paused = true;
            s.next.clear();
            s.error = Some(format!(
                "Scheduling paused because progress could not be saved: {e:#}"
            ));
            self.store.log(s.error.as_deref().unwrap());
            return false;
        }
        true
    }
    fn finish_manual(&self, s: &mut State, result: Result<Vec<String>>, now: DateTime<Utc>) {
        let (success, result, files) = match result {
            Ok(files) => (
                true,
                format!("Saved {} monitor image(s).", files.len()),
                files,
            ),
            Err(error) => (false, format!("Failed: {error:#}"), vec![]),
        };
        self.store.log(&format!("Manual capture: {result}"));
        s.manual_capture_running = false;
        s.manual_capture = Some(ManualCapture {
            finished_at: now,
            success,
            result,
            files,
        });
    }
    fn rearm(&self, s: &mut State, now: DateTime<Utc>, skip_missed: bool) {
        s.next.clear();
        for t in &s.settings.tasks {
            if !t.enabled {
                continue;
            }
            let p = s.runtime.tasks.entry(t.id.clone()).or_default();
            if skip_missed {
                schedule::mark_missed_once(t, p, now, &Local);
            }
            if !s.settings.paused
                && self.available.load(Ordering::SeqCst)
                && let Some(o) = schedule::next(t, p, now, &Local)
            {
                s.next.insert(t.id.clone(), o);
            }
        }
    }
    fn finish_stopped(&self, s: &mut State, now: DateTime<Utc>) {
        if s.busy {
            return;
        }
        let mut settings = s.settings.clone();
        let mut changed = false;
        for t in &mut settings.tasks {
            if t.enabled
                && s.runtime.tasks.get(&t.id).is_some_and(|p| {
                    schedule::exhausted(t, p, now, &Local)
                        && p.once_result.as_deref() != Some("running")
                })
            {
                t.enabled = false;
                changed = true;
            }
        }
        if changed {
            settings.revision += 1;
            match self.store.save_settings(&settings) {
                Ok(()) => s.settings = settings,
                Err(e) => {
                    s.settings.paused = true;
                    s.error = Some(format!("Could not save task completion: {e:#}"));
                }
            }
        }
    }
    fn run(&self, rx: Receiver<Event>) {
        let (work_tx, work_rx) = mpsc::sync_channel::<Vec<Job>>(1);
        let worker_app = self.clone();
        let worker = thread::Builder::new()
            .name("capture".into())
            .spawn(move || {
                while let Ok(jobs) = work_rx.recv() {
                    let mut finished = Vec::new();
                    for job in jobs {
                        if worker_app.stopping.load(Ordering::SeqCst) {
                            break;
                        }
                        let valid = {
                            let s = worker_app.state.lock().unwrap();
                            worker_app.desktop_epoch.load(Ordering::SeqCst) == job.desktop_epoch
                                && (job.manual
                                    || (s.settings.revision == job.epoch
                                        && s.settings.tasks.iter().any(|t| t == &job.task)
                                        && !s.settings.paused
                                        && job.task.enabled))
                        };
                        let result = if !valid || !worker_app.available.load(Ordering::SeqCst) {
                            Err(anyhow::anyhow!(
                                "Skipped — settings or desktop availability changed before capture."
                            ))
                        } else {
                            #[cfg(windows)]
                            {
                                crate::platform::capture(
                                    &worker_app.store.base,
                                    &job.task,
                                    &worker_app.available,
                                    &worker_app.desktop_epoch,
                                    job.desktop_epoch,
                                )
                            }
                            #[cfg(not(windows))]
                            {
                                Err(anyhow::anyhow!("Capture requires Windows."))
                            }
                        };
                        finished.push(Finished { job, result });
                    }
                    if worker_app.stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    if worker_app.tx.send(Event::Done(finished)).is_err() {
                        break;
                    }
                }
            })
            .expect("capture worker");
        loop {
            let timeout = {
                let s = self.state.lock().unwrap();
                let mut wake = s.next.values().map(|o| o.due).min();
                for t in s.settings.tasks.iter().filter(|t| t.enabled) {
                    if let Some(p) = s.runtime.tasks.get(&t.id) {
                        // Once tasks must become missed even while paused/locked.
                        let mut due = schedule::deadline(t, p, &Local);
                        if t.schedule.kind == "once" && p.once_result.is_none() {
                            due = schedule::next(
                                t,
                                p,
                                Utc::now() - chrono::Duration::days(365_000),
                                &Local,
                            )
                            .map(|o| o.due)
                            .into_iter()
                            .chain(due)
                            .min();
                        }
                        if let Some(due) = due.filter(|d| *d > Utc::now()) {
                            wake = Some(wake.map_or(due, |w| w.min(due)));
                        }
                    }
                }
                wake.map(|w| (w - Utc::now()).to_std().unwrap_or(Duration::ZERO))
            };
            let event = if let Some(d) = timeout {
                match rx.recv_timeout(d) {
                    Ok(e) => Some(e),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(_) => break,
                }
            } else {
                match rx.recv() {
                    Ok(e) => Some(e),
                    Err(_) => break,
                }
            };
            if self.stopping.load(Ordering::SeqCst) {
                break;
            }
            let mut s = self.state.lock().unwrap();
            let now = Utc::now();
            match event {
                Some(Event::Stop) => break,
                Some(Event::Done(finished)) => {
                    s.busy = false;
                    for finished in finished {
                        let job = finished.job;
                        if job.manual {
                            self.finish_manual(&mut s, finished.result, now);
                            continue;
                        }
                        let Some(p) = s.runtime.tasks.get_mut(&job.task.id) else {
                            continue;
                        };
                        if p.generation != job.generation {
                            continue;
                        }
                        p.last_run_utc = Some(now);
                        match finished.result {
                            Ok(files) => {
                                p.last_result =
                                    Some(format!("Saved {} monitor image(s).", files.len()));
                                p.last_files = files;
                                p.completed_captures = p.completed_captures.saturating_add(1);
                                if job.task.schedule.kind == "once" {
                                    p.once_result = Some("completed".into());
                                }
                            }
                            Err(e) => {
                                p.last_result = Some(format!("Failed: {e:#}"));
                                if job.task.schedule.kind == "once" {
                                    p.once_result = Some("failed".into());
                                }
                            }
                        }
                        self.store.log(&format!(
                            "{}: {}",
                            job.task.name,
                            p.last_result.as_deref().unwrap_or("")
                        ));
                    }
                    self.persist(&mut s);
                }
                Some(Event::Manual {
                    desktop_epoch,
                    output_folder,
                }) => {
                    // Keep the destination selected when the request was accepted.
                    let job = Job {
                        task: Task {
                            name: "Manual capture".into(),
                            capture: crate::config::Capture {
                                output_folder,
                                ..Default::default()
                            },
                            ..Task::default()
                        },
                        generation: 0,
                        manual: true,
                        epoch: 0,
                        desktop_epoch,
                    };
                    if work_tx.try_send(vec![job]).is_err() {
                        s.busy = false;
                        self.finish_manual(
                            &mut s,
                            Err(anyhow::anyhow!("Capture worker unavailable.")),
                            now,
                        );
                    }
                }
                _ => {}
            }
            let due: Vec<_> = s
                .next
                .iter()
                .filter(|(_, o)| o.due <= now)
                .map(|(id, o)| (id.clone(), o.clone()))
                .collect();
            let mut jobs = Vec::new();
            for (id, occurrence) in due {
                s.next.remove(&id);
                let Some(t) = s.settings.tasks.iter().find(|t| t.id == id).cloned() else {
                    continue;
                };
                let blocked = s.busy
                    || s.settings.paused
                    || !self.available.load(Ordering::SeqCst)
                    || (now - occurrence.due).num_seconds() > 2;
                let p = s.runtime.tasks.entry(id.clone()).or_default();
                if schedule::exhausted(&t, p, now, &Local) {
                    continue;
                }
                if blocked {
                    if t.schedule.kind == "once" {
                        p.once_result = Some("missed".into());
                        p.last_result = Some(
                            "Missed — capture was busy or the desktop was unavailable.".into(),
                        );
                    }
                } else {
                    p.last_fixed_occurrence = occurrence.key;
                    if let Some(slot) = occurrence.slot {
                        p.completed_fixed_times.push(slot);
                    }
                    if t.schedule.kind == "once" {
                        p.once_result = Some("running".into());
                    }
                    jobs.push(Job {
                        task: t.clone(),
                        generation: p.generation,
                        manual: false,
                        epoch: s.settings.revision,
                        desktop_epoch: self.desktop_epoch.load(Ordering::SeqCst),
                    });
                }
                if let Some(o) = schedule::next(&t, &s.runtime.tasks[&id], now, &Local) {
                    s.next.insert(id, o);
                }
            }
            // Catch once tasks that expired while unavailable without replaying them.
            let tasks = s.settings.tasks.clone();
            for t in &tasks {
                if !s.next.contains_key(&t.id) {
                    schedule::mark_missed_once(
                        t,
                        s.runtime.tasks.entry(t.id.clone()).or_default(),
                        now,
                        &Local,
                    );
                }
            }
            if !jobs.is_empty() && self.persist(&mut s) {
                s.busy = true;
                if work_tx.try_send(jobs).is_err() {
                    s.busy = false;
                    s.error = Some("Capture worker unavailable.".into());
                }
            }
            self.finish_stopped(&mut s, now);
            // Remove exhausted schedules without disturbing armed simultaneous runs.
            let enabled: Vec<_> = s
                .settings
                .tasks
                .iter()
                .filter(|t| t.enabled)
                .map(|t| t.id.clone())
                .collect();
            s.next.retain(|id, _| enabled.contains(id));
            self.persist(&mut s);
        }
        // Disconnect completion senders before joining, even if the event queue
        // was full when Exit arrived.
        drop(rx);
        drop(work_tx);
        let _ = worker.join();
    }
}
