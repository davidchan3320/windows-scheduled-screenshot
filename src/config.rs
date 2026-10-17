use anyhow::{Result, bail, ensure};
use chrono::{Local, NaiveDateTime, NaiveTime};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct CaptureShortcut {
    pub enabled: bool,
    pub modifiers: String,
    pub key: String,
}
impl Default for CaptureShortcut {
    fn default() -> Self {
        Self {
            enabled: true,
            modifiers: "ctrl+shift".into(),
            key: "S".into(),
        }
    }
}
impl CaptureShortcut {
    pub fn modifiers_bits(&self) -> Result<u32> {
        Ok(match self.modifiers.as_str() {
            "ctrl+shift" => 0x0002 | 0x0004,
            "ctrl+alt" => 0x0002 | 0x0001,
            "alt+shift" => 0x0001 | 0x0004,
            "ctrl+alt+shift" => 0x0002 | 0x0001 | 0x0004,
            _ => bail!("Choose a supported shortcut modifier combination."),
        })
    }
    pub fn virtual_key(&self) -> Result<u32> {
        if self.key.len() == 1 {
            let key = self.key.as_bytes()[0];
            if key.is_ascii_uppercase() || key.is_ascii_digit() {
                return Ok(key.into());
            }
        }
        for number in 1..=11 {
            if self.key == format!("F{number}") {
                return Ok(0x70 + number - 1);
            }
        }
        bail!("Choose a shortcut key: A-Z, 0-9 or F1-F11.")
    }
    pub fn label(&self) -> String {
        format!(
            "{}+{}",
            self.modifiers
                .replace("ctrl", "Ctrl")
                .replace("alt", "Alt")
                .replace("shift", "Shift"),
            self.key
        )
    }
}

pub const DAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Settings {
    pub schema_version: u32,
    pub revision: u64,
    pub paused: bool,
    pub start_at_sign_in: bool,
    pub language: String,
    pub manual_output_folder: String,
    pub capture_shortcut: CaptureShortcut,
    pub tasks: Vec<Task>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: 2,
            revision: 0,
            paused: false,
            start_at_sign_in: false,
            language: "en".into(),
            manual_output_folder: "captures".into(),
            capture_shortcut: CaptureShortcut::default(),
            tasks: vec![],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Task {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub capture: Capture,
    pub schedule: Schedule,
    pub stop_condition: Stop,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "New task".into(),
            enabled: false,
            capture: Capture::default(),
            schedule: Schedule::default(),
            stop_condition: Stop::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Capture {
    pub output_folder: String,
    pub date_subfolders: bool,
    pub file_name_template: String,
    pub image_format: String,
    pub jpeg_quality: u8,
    pub include_cursor: bool,
}
impl Default for Capture {
    fn default() -> Self {
        Self {
            output_folder: "captures".into(),
            date_subfolders: true,
            file_name_template: "{timestamp}_{display}".into(),
            image_format: "jpeg".into(),
            jpeg_quality: 85,
            include_cursor: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Schedule {
    #[serde(rename = "type")]
    pub kind: String,
    pub at_local: Option<String>,
    pub interval_seconds: u32,
    pub weekdays: Vec<String>,
    pub active_hours_enabled: bool,
    pub active_start: String,
    pub active_end: String,
    pub times: Vec<String>,
}
impl Default for Schedule {
    fn default() -> Self {
        Self {
            kind: "interval".into(),
            at_local: None,
            interval_seconds: 60,
            weekdays: DAYS.iter().map(|s| s.to_string()).collect(),
            active_hours_enabled: false,
            active_start: "09:00:00".into(),
            active_end: "17:00:00".into(),
            times: vec!["09:00:00".into()],
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub struct Stop {
    pub mode: String,
    pub end_at_local: Option<String>,
    pub duration_seconds: Option<u32>,
    pub capture_count: Option<u32>,
}
impl Default for Stop {
    fn default() -> Self {
        Self {
            mode: "none".into(),
            end_at_local: None,
            duration_seconds: None,
            capture_count: None,
        }
    }
}

pub fn clock(s: &str) -> Result<NaiveTime> {
    ensure!(s.len() == 8, "Time must use HH:mm:ss.");
    Ok(NaiveTime::parse_from_str(s, "%H:%M:%S")?)
}
pub fn local_datetime(s: &str) -> Result<NaiveDateTime> {
    ensure!(s.len() == 19, "Date and time must use YYYY-MM-DDTHH:mm:ss.");
    let n = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S")?;
    ensure!(
        crate::schedule::local_to_utc(&Local, n).is_some(),
        "This local time does not exist because the clock moves forward. Choose another time."
    );
    Ok(n)
}
impl Settings {
    pub fn manual_capture_task(&self) -> Task {
        Task {
            name: "Manual capture".into(),
            capture: Capture {
                output_folder: self.manual_output_folder.clone(),
                ..Capture::default()
            },
            ..Task::default()
        }
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            matches!(self.language.as_str(), "en" | "zh-Hant"),
            "Unsupported language."
        );
        ensure!(
            self.schema_version == 2,
            "Unsupported settings schema; expected version 2."
        );
        self.capture_shortcut.modifiers_bits()?;
        self.capture_shortcut.virtual_key()?;
        self.manual_capture_task().validate()?;
        ensure!(self.tasks.len() <= 128, "At most 128 tasks are supported.");
        let mut ids = HashSet::new();
        for t in &self.tasks {
            ensure!(
                Uuid::parse_str(&t.id).is_ok() && ids.insert(t.id.to_lowercase()),
                "Task IDs must be unique UUIDs."
            );
            t.validate()
                .map_err(|e| anyhow::anyhow!("{}: {e}", t.name))?;
        }
        Ok(())
    }
}
impl Task {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.name.trim().is_empty() && self.name.chars().count() <= 64,
            "Name must contain 1–64 characters."
        );
        ensure!(Uuid::parse_str(&self.id).is_ok(), "Task ID must be a UUID.");
        let c = &self.capture;
        ensure!(
            matches!(c.image_format.as_str(), "jpeg" | "png"),
            "Choose JPEG or PNG."
        );
        ensure!(
            (1..=100).contains(&c.jpeg_quality),
            "JPEG quality must be 1–100."
        );
        ensure!(
            !c.output_folder.trim().is_empty()
                && !c.output_folder.contains('\0')
                && c.output_folder.len() <= 2000,
            "Enter a valid output folder."
        );
        crate::output::validate_template(&c.file_name_template)?;
        let s = &self.schedule;
        ensure!(
            matches!(s.kind.as_str(), "once" | "interval" | "fixed" | "fixedOnce"),
            "Unknown schedule type."
        );
        if s.kind == "once" {
            local_datetime(s.at_local.as_deref().unwrap_or_default())?;
        } else {
            ensure!(
                !s.weekdays.is_empty()
                    && s.weekdays.len() <= 7
                    && s.weekdays.iter().all(|d| DAYS.contains(&d.as_str())),
                "Select at least one valid weekday."
            );
            ensure!(
                (1..=31_536_000).contains(&s.interval_seconds),
                "Interval must be 1–31536000 seconds."
            );
            if s.active_hours_enabled {
                ensure!(
                    clock(&s.active_start)? < clock(&s.active_end)?,
                    "Active hours must end after they start on the same day."
                );
            }
            if s.kind != "interval" {
                ensure!(
                    !s.times.is_empty() && s.times.len() <= 128,
                    "Enter 1–128 times."
                );
                let mut times = HashSet::new();
                for t in &s.times {
                    ensure!(times.insert(clock(t)?), "Remove duplicate times.");
                }
            }
        }
        let st = &self.stop_condition;
        match st.mode.as_str() {
            "none" => {}
            "at" => {
                local_datetime(st.end_at_local.as_deref().unwrap_or_default())?;
            }
            "duration" => ensure!(
                st.duration_seconds
                    .is_some_and(|s| (1..=31_536_000).contains(&s)),
                "Duration must be 1–31536000 seconds."
            ),
            "count" => ensure!(
                st.capture_count
                    .is_some_and(|s| (1..=1_000_000).contains(&s)),
                "Capture count must be 1–1000000."
            ),
            _ => bail!("Unknown stop condition."),
        }
        Ok(())
    }
}
