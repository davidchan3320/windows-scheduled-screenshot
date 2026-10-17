use crate::{
    config::Settings,
    output::resolve_folder,
    schedule::{self, Runtime},
};
use anyhow::{Context, Result, ensure};
use chrono::Utc;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing data directory")?;
    fs::create_dir_all(parent)?;
    let tmp = parent.join(format!(".{}.tmp", Uuid::new_v4()));
    let result = (|| -> Result<()> {
        let mut f = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        #[cfg(windows)]
        {
            use std::os::windows::ffi::OsStrExt;
            use windows::{
                Win32::Storage::FileSystem::{
                    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
                },
                core::PCWSTR,
            };
            let from: Vec<u16> = tmp.as_os_str().encode_wide().chain(Some(0)).collect();
            let to: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
            unsafe {
                MoveFileExW(
                    PCWSTR(from.as_ptr()),
                    PCWSTR(to.as_ptr()),
                    MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
                )?;
            }
        }
        #[cfg(not(windows))]
        fs::rename(&tmp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result.with_context(|| {
        format!(
            "Could not save {}. Check free space and folder permissions.",
            path.display()
        )
    })
}
pub fn save_json<T: Serialize>(path: &Path, value: &T) -> Result<()> {
    atomic_write(path, &serde_json::to_vec_pretty(value)?)
}
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    ensure!(
        fs::metadata(path)?.len() <= 2_097_152,
        "{} is larger than 2 MB.",
        path.display()
    );
    let bytes = fs::read(path)?;
    serde_json::from_slice(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes))
        .with_context(|| format!("Invalid JSON in {}", path.display()))
}

#[derive(Clone)]
pub struct Store {
    pub base: PathBuf,
    pub data: PathBuf,
}
pub struct Loaded {
    pub settings: Settings,
    pub runtime: Runtime,
    pub warning: Option<String>,
}
impl Store {
    pub fn new(base: PathBuf) -> Result<Self> {
        let data = base.join("data");
        let store = Self { base, data };
        fs::create_dir_all(&store.data).and_then(|_| {
            let probe = store.data.join(format!(".write-test-{}", Uuid::new_v4()));
            let mut file = OpenOptions::new().write(true).create_new(true).open(&probe)?;
            file.write_all(b"portable")?;
            file.sync_all()?;
            drop(file);
            fs::remove_file(probe)
        }).with_context(|| format!("The application folder is not writable: {}. Move ScheduledScreenshot.exe and its data folder to a writable folder, then open it again.", store.base.display()))?;
        fs::create_dir_all(store.data.join("logs"))?;
        Ok(store)
    }
    pub fn load(&self) -> Result<Loaded> {
        let path = self.data.join("settings.json");
        if !path.exists() && self.base.join("settings.json").exists() {
            self.migrate()?;
        }
        if !path.exists() {
            self.save_settings(&Settings::default())?;
        }
        let mut warning = None;
        let settings = match read_json::<Settings>(&path).and_then(|s| {
            s.validate()?;
            Ok(s)
        }) {
            Ok(s) => s,
            Err(e) => {
                let mut valid: Settings = read_json(&self.data.join("settings.last-good.json")).with_context(|| format!("Settings could not be loaded: {e:#}. Correct data/settings.json or restore its backup."))?;
                valid.validate()?;
                valid.paused = true;
                warning = Some(format!(
                    "Settings rejected: {e:#}. Last valid settings loaded, scheduling paused. Review and save settings to continue."
                ));
                valid
            }
        };
        let runtime_path = self.data.join("runtime-state.json");
        let mut runtime: Runtime = if runtime_path.exists() {
            read_json(&runtime_path).context("Runtime progress is invalid. Restore data/runtime-state.json from a backup before restarting; progress was not reset.")?
        } else {
            Runtime::default()
        };
        for p in runtime.tasks.values_mut() {
            if p.once_result.as_deref() == Some("running") {
                p.once_result = Some("failed".into());
                p.last_result =
                    Some("Failed — application stopped during capture; not retried.".into());
            }
        }
        Ok(Loaded {
            settings,
            runtime,
            warning,
        })
    }
    pub fn save_settings(&self, s: &Settings) -> Result<()> {
        s.validate()?;
        let primary = self.data.join("settings.json");
        // Back up the previous validated configuration before replacing it.
        if let Ok(old) = read_json::<Settings>(&primary)
            && old.validate().is_ok()
        {
            save_json(&self.data.join("settings.last-good.json"), &old)?;
        }
        save_json(&primary, s)?;
        // Primary is authoritative if refreshing the fallback fails.
        if let Err(e) = save_json(&self.data.join("settings.last-good.json"), s) {
            self.log(&format!("Backup refresh failed: {e:#}"));
        }
        Ok(())
    }
    pub fn save_runtime(&self, r: &Runtime) -> Result<()> {
        save_json(&self.data.join("runtime-state.json"), r)
    }
    pub fn log(&self, message: &str) {
        // Called under the application state lock. Four 1 MiB files, no token or image data.
        let log = self.data.join("logs/app.log");
        let write = || -> Result<()> {
            if fs::metadata(&log).is_ok_and(|m| m.len() >= 1_048_576) {
                let _ = fs::remove_file(self.data.join("logs/app.3.log"));
                for n in (1..3).rev() {
                    let _ = fs::rename(
                        self.data.join(format!("logs/app.{n}.log")),
                        self.data.join(format!("logs/app.{}.log", n + 1)),
                    );
                }
                fs::rename(&log, self.data.join("logs/app.1.log"))?;
            }
            let mut f = OpenOptions::new().create(true).append(true).open(log)?;
            let clean: String = message
                .chars()
                .take(4096)
                .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
                .collect();
            writeln!(f, "{} {clean}", Utc::now().to_rfc3339())?;
            Ok(())
        };
        let _ = write();
    }
    fn migrate(&self) -> Result<()> {
        let source = self.base.join("settings.json");
        let mut json: serde_json::Value = read_json(&source)?;
        let version = json
            .get("schemaVersion")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        ensure!(
            version == 1,
            "Legacy settings have an unsupported schema version {version}."
        );
        let object = json
            .as_object_mut()
            .context("Legacy settings must be an object.")?;
        object.remove("logging");
        object.insert("schemaVersion".into(), 2.into());
        if let Some(tasks) = object.get_mut("tasks").and_then(|v| v.as_array_mut()) {
            for task in tasks {
                let task = task
                    .as_object_mut()
                    .context("Legacy task must be an object")?;
                let capture = task
                    .entry("capture")
                    .or_insert_with(|| serde_json::json!({}))
                    .as_object_mut()
                    .context("Legacy capture must be an object")?;
                capture.entry("outputFolder").or_insert_with(|| {
                    serde_json::json!("%USERPROFILE%\\Pictures\\Scheduled Screenshots")
                });
            }
        }
        let mut settings: Settings = serde_json::from_value(json)?;
        for t in &mut settings.tasks {
            // Legacy paths were relative to the old executable, and always dated.
            t.capture.output_folder = resolve_folder(&self.base, &t.capture.output_folder)?
                .to_string_lossy()
                .into_owned();
            t.capture.date_subfolders = true;
        }
        settings.validate()?;
        let old_runtime = self.base.join("runtime-state.json");
        let mut runtime: Runtime = if old_runtime.exists() {
            let mut raw: serde_json::Value = read_json(&old_runtime)?;
            if let Some(tasks) = raw.get_mut("tasks").and_then(|v| v.as_object_mut()) {
                for p in tasks.values_mut() {
                    if p.get("completedFixedTimes").is_some_and(|v| v.is_null()) {
                        p["completedFixedTimes"] = serde_json::json!([]);
                    }
                }
            }
            serde_json::from_value(raw)?
        } else {
            Runtime::default()
        };
        for t in &settings.tasks {
            if let Some(p) = runtime.tasks.get_mut(&t.id) {
                // Replace the old C# hashes without throwing away anchors/counts/slots.
                p.schedule_signature = Some(schedule::signature(t));
                p.stop_signature = Some(schedule::stop_signature(t));
            }
        }
        let backup = self.data.join("legacy-backup");
        fs::create_dir_all(&backup)?;
        for name in ["settings.json", "runtime-state.json"] {
            let src = self.base.join(name);
            let dst = backup.join(name);
            if src.exists() && !dst.exists() {
                fs::copy(src, dst)?;
            }
        }
        // Commit settings last. Interrupted migration can safely resume.
        self.save_runtime(&runtime)?;
        self.save_settings(&settings)?;
        self.log("Migrated legacy settings and runtime progress; originals preserved in data/legacy-backup.");
        Ok(())
    }
}
