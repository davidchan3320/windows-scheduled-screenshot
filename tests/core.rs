use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Utc};
use scheduled_screenshot::{
    config::{Settings, Task},
    engine::App,
    http::authorized,
    output,
    schedule::{self, Progress},
    storage::{self, Store},
};
use std::{collections::HashMap, fs, io::Write};

fn utc(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}
fn task() -> Task {
    Task {
        enabled: true,
        ..Default::default()
    }
}
fn progress(t: &Task, now: DateTime<Utc>) -> Progress {
    let mut p = Progress::default();
    schedule::reconcile(t, &mut p, now);
    p
}

#[test]
fn defaults_are_portable_and_disabled() {
    let t = Task::default();
    t.validate().unwrap();
    assert!(!t.enabled);
    assert_eq!(t.capture.output_folder, "captures");
    assert!(t.capture.date_subfolders);
    assert_eq!(t.capture.jpeg_quality, 85);
    assert!(!t.capture.split_by_monitor);
}

#[test]
fn combined_capture_defaults_and_split_preferences_survive_save_and_import() {
    let mut settings: Settings = serde_json::from_value(serde_json::json!({
        "schemaVersion": 2,
        "tasks": [{"capture": {"outputFolder": "task captures"}}]
    }))
    .unwrap();
    assert!(!settings.manual_split_by_monitor);
    assert!(!settings.manual_capture_task().capture.split_by_monitor);
    assert!(!settings.tasks[0].capture.split_by_monitor);
    settings.manual_split_by_monitor = true;
    assert!(settings.manual_capture_task().capture.split_by_monitor);
    assert!(!settings.tasks[0].capture.split_by_monitor);
    settings.tasks[0].capture.split_by_monitor = true;
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path().into()).unwrap();
    store.save_settings(&settings).unwrap();
    let loaded = store.load().unwrap().settings;
    assert_eq!(loaded, settings);
    let exported = serde_json::to_value(&loaded).unwrap();
    assert_eq!(exported["manualSplitByMonitor"], true);
    assert_eq!(exported["tasks"][0]["capture"]["splitByMonitor"], true);
    let imported: Settings = serde_json::from_value(exported).unwrap();
    imported.validate().unwrap();
    assert_eq!(imported, settings);
    assert!(
        serde_json::from_value::<Settings>(serde_json::json!({
            "manualSplitByMonitor": "yes"
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<Task>(serde_json::json!({
            "capture": {"splitByMonitor": "yes"}
        }))
        .is_err()
    );
}
#[test]
fn language_defaults_persist_and_reject_unknown_packs() {
    let old: Settings = serde_json::from_str(r#"{"schemaVersion":2,"tasks":[]}"#).unwrap();
    assert_eq!(old.language, "en");
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path().into()).unwrap();
    let app = App::start(store.clone()).unwrap();
    let mut settings = old;
    settings.language = "zh-Hant".into();
    app.save(settings.clone(), 0).unwrap();
    assert_eq!(store.load().unwrap().settings.language, "zh-Hant");
    settings.language = "invalid".into();
    assert!(app.save(settings, 1).is_err());
    assert_eq!(store.load().unwrap().settings.language, "zh-Hant");
    app.shutdown();
    let restarted = App::start(store).unwrap();
    assert_eq!(restarted.snapshot()["settings"]["language"], "zh-Hant");
    restarted.shutdown();
}

#[test]
fn bundled_language_packs_have_matching_keys_and_placeholders() {
    use scheduled_screenshot::localization::{EN, ZH_HANT, text};
    let en: HashMap<String, String> = serde_json::from_str(EN).unwrap();
    let zh: HashMap<String, String> = serde_json::from_str(ZH_HANT).unwrap();
    assert_eq!(en.len(), zh.len());
    fn placeholders(value: &str) -> std::collections::BTreeSet<&str> {
        value
            .split('{')
            .skip(1)
            .filter_map(|tail| tail.split_once('}').map(|(key, _)| key))
            .collect()
    }
    for (key, value) in &en {
        let translated = zh.get(key).expect("matching translation key");
        assert!(!translated.is_empty());
        assert_eq!(placeholders(value), placeholders(translated), "{key}");
    }
    assert_eq!(text("zh-Hant", "Open task manager"), "開啟工作管理介面");
    assert_eq!(text("en", "Open task manager"), "Open task manager");
    assert_eq!(text("zh-Hant", "unknown"), "unknown");
}
#[test]
fn malformed_types_and_unknown_schema_are_rejected() {
    for json in [
        r#"{"paused":"false"}"#,
        r#"{"tasks":[null]}"#,
        r#"{"tasks":{}}"#,
        r#"{"tasks":[{"schedule":{"intervalSeconds":"<img>"}}]}"#,
    ] {
        assert!(serde_json::from_str::<Settings>(json).is_err());
    }
    assert!(
        Settings {
            schema_version: 3,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
}
#[test]
fn expired_stop_remains_valid_and_is_exhausted() {
    let mut t = task();
    t.stop_condition.mode = "at".into();
    t.stop_condition.end_at_local = Some("2020-01-01T12:00:00".into());
    t.validate().unwrap();
    assert!(schedule::exhausted(
        &t,
        &Progress::default(),
        Utc::now(),
        &Local
    ));
}
#[test]
fn interval_cadence_skips_missed_and_capture_duration() {
    let t = task();
    let anchor = utc("2026-01-01T00:00:00Z");
    let p = progress(&t, anchor);
    assert_eq!(
        schedule::next(&t, &p, anchor, &Utc).unwrap().due,
        anchor + Duration::seconds(60)
    );
    assert_eq!(
        schedule::next(&t, &p, anchor + Duration::seconds(185), &Utc)
            .unwrap()
            .due,
        anchor + Duration::seconds(240)
    );
}
#[test]
fn long_intervals_are_not_lost_outside_week_horizon() {
    let mut t = task();
    t.schedule.interval_seconds = 31_536_000;
    let now = utc("2026-01-01T00:00:00Z");
    let p = progress(&t, now);
    assert_eq!(
        schedule::next(&t, &p, now, &Utc).unwrap().due,
        now + Duration::seconds(31_536_000)
    );
}
#[test]
fn active_hours_and_weekdays_keep_original_cadence() {
    let mut t = task();
    t.schedule.interval_seconds = 600;
    t.schedule.active_hours_enabled = true;
    t.schedule.weekdays = vec!["Mon".into()];
    let anchor = utc("2026-10-16T08:03:00Z");
    let p = progress(&t, anchor);
    assert_eq!(
        schedule::next(&t, &p, anchor, &Utc).unwrap().due,
        utc("2026-10-19T09:03:00Z")
    );
}
#[test]
fn dst_gap_is_skipped_and_fold_executes_once() {
    let tz = chrono_tz::America::New_York;
    let gap = NaiveDateTime::parse_from_str("2026-03-08T02:30:00", "%Y-%m-%dT%H:%M:%S").unwrap();
    assert!(schedule::local_to_utc(&tz, gap).is_none());
    let mut t = task();
    t.schedule.kind = "fixed".into();
    t.schedule.times = vec!["02:30:00".into()];
    let now = utc("2026-03-08T05:00:00Z");
    let p = progress(&t, now);
    assert_eq!(
        schedule::next(&t, &p, now, &tz).unwrap().due,
        utc("2026-03-09T06:30:00Z")
    );
    t.schedule.times = vec!["01:30:00".into()];
    let now = utc("2026-11-01T04:00:00Z");
    let mut p = progress(&t, now);
    let first = schedule::next(&t, &p, now, &tz).unwrap();
    assert_eq!(first.due, utc("2026-11-01T06:30:00Z"));
    p.last_fixed_occurrence = first.key;
    // Even a clock rollback cannot replay the same wall-clock occurrence.
    assert_eq!(
        schedule::next(&t, &p, now, &tz).unwrap().due,
        utc("2026-11-02T06:30:00Z")
    );
}
#[test]
fn once_missed_and_running_never_retry() {
    let mut t = task();
    t.schedule.kind = "once".into();
    t.schedule.at_local = Some("2026-01-01T10:00:00".into());
    let now = utc("2026-01-01T09:00:00Z");
    let mut p = progress(&t, now);
    assert_eq!(
        schedule::next(&t, &p, now, &Utc).unwrap().due,
        utc("2026-01-01T10:00:00Z")
    );
    assert!(schedule::mark_missed_once(
        &t,
        &mut p,
        utc("2026-01-01T11:00:00Z"),
        &Utc
    ));
    assert_eq!(p.once_result.as_deref(), Some("missed"));
    assert!(schedule::next(&t, &p, now, &Utc).is_none());
    p.once_result = Some("running".into());
    assert!(schedule::next(&t, &p, now, &Utc).is_none());
}
#[test]
fn legacy_fixed_once_consumes_each_slot_only_once() {
    let mut t = task();
    t.schedule.kind = "fixedOnce".into();
    t.schedule.times = vec!["09:00:00".into(), "15:00:00".into()];
    let now = utc("2026-01-01T12:00:00Z");
    let mut p = progress(&t, now);
    p.completed_fixed_times.push("15:00:00".into());
    assert_eq!(
        schedule::next(&t, &p, now, &Utc).unwrap().due,
        utc("2026-01-02T09:00:00Z")
    );
    p.completed_fixed_times.push("09:00:00".into());
    assert!(schedule::exhausted(&t, &p, now, &Utc));
}
#[test]
fn stop_count_duration_and_deadline_are_exclusive() {
    let now = utc("2026-01-01T09:00:00Z");
    let mut t = task();
    t.stop_condition.mode = "duration".into();
    t.stop_condition.duration_seconds = Some(60);
    let p = progress(&t, now);
    assert!(schedule::next(&t, &p, now, &Utc).is_none());
    assert!(schedule::exhausted(
        &t,
        &p,
        now + Duration::seconds(60),
        &Utc
    ));
    t.stop_condition.mode = "count".into();
    t.stop_condition.capture_count = Some(2);
    let mut p = progress(&t, now);
    p.completed_captures = 1;
    assert!(!schedule::exhausted(&t, &p, now, &Utc));
    p.completed_captures = 2;
    assert!(schedule::exhausted(&t, &p, now, &Utc));
}
#[test]
fn unchanged_reconciliation_preserves_progress() {
    let t = task();
    let now = Utc::now();
    let mut p = progress(&t, now);
    p.completed_captures = 5;
    let before = p.clone();
    schedule::reconcile(&t, &mut p, now + Duration::hours(1));
    assert_eq!(p, before);
}
#[test]
fn templates_preserve_tokens_and_reject_windows_characters() {
    for bad in [
        "../x", "x\\x", "{bad}", "{date", "x}", "{date}}", "x\0", "x:y",
    ] {
        assert!(output::validate_template(bad).is_err(), "{bad:?}");
    }
    let mut t = task();
    t.name = "Daily: report".into();
    t.capture.file_name_template = "{DATE}_{task}_{DISPLAYINDEX}.jpg".into();
    t.capture.date_subfolders = false;
    let at = Local.with_ymd_and_hms(2026, 8, 24, 13, 59, 1).unwrap();
    let p = output::path_for(std::path::Path::new("C:/App"), &t, at, "\\\\.\\DISPLAY1", 2).unwrap();
    assert_eq!(p.file_name().unwrap(), "20260824_Daily_ report_2.jpg");
    assert_eq!(output::sanitize("CON.txt"), "_CON.txt");
    assert_eq!(output::sanitize("LPT¹"), "_LPT¹");
}
#[test]
fn collision_reservations_never_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("test.jpg");
    let (mut a, first) = output::reserve(&p).unwrap();
    a.write_all(b"original").unwrap();
    drop(a);
    let (_, second) = output::reserve(&p).unwrap();
    assert_ne!(first, second);
    assert_eq!(fs::read(first).unwrap(), b"original");
}
#[test]
fn relative_paths_move_with_application() {
    let t = task();
    let now = Local::now();
    let p = output::path_for(
        std::path::Path::new("C:/Folder with spaces/截圖"),
        &t,
        now,
        "DISPLAY1",
        1,
    )
    .unwrap();
    assert!(p.starts_with("C:/Folder with spaces/截圖/captures"));
    assert!(
        p.to_string_lossy()
            .contains(&now.format("%Y-%m-%d").to_string())
    );
}

#[test]
fn folder_picker_paths_can_contain_literal_percent_signs() {
    let base = std::path::Path::new("C:/Portable");
    assert_eq!(
        output::resolve_folder(base, "100% complete").unwrap(),
        base.join("100% complete")
    );
    assert_eq!(
        output::resolve_folder(base, "%SCHEDULED_SCREENSHOT_UNDEFINED_TEST%").unwrap(),
        base.join("%SCHEDULED_SCREENSHOT_UNDEFINED_TEST%")
    );
}
#[test]
fn atomic_writes_replace_without_partial_files() {
    let d = tempfile::tempdir().unwrap();
    let p = d.path().join("state.json");
    for n in 0..20 {
        storage::save_json(&p, &serde_json::json!({"n":n})).unwrap();
        let v: serde_json::Value = storage::read_json(&p).unwrap();
        assert_eq!(v["n"], n);
    }
    assert_eq!(fs::read_dir(d.path()).unwrap().count(), 1);
}
#[test]
fn rejected_settings_preserve_last_good_and_revision_conflicts() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(d.path().into()).unwrap();
    let app = App::start(store.clone()).unwrap();
    let mut settings = Settings {
        tasks: vec![Task::default()],
        ..Default::default()
    };
    app.save(settings.clone(), 0).unwrap();
    assert!(
        app.save(settings.clone(), 0)
            .unwrap_err()
            .to_string()
            .starts_with("CONFLICT:")
    );
    settings.tasks[0].capture.file_name_template = "../bad".into();
    assert!(app.save(settings, 1).is_err());
    assert_eq!(store.load().unwrap().settings.revision, 1);
    app.shutdown();
    fs::write(store.data.join("settings.json"), b"broken").unwrap();
    let loaded = store.load().unwrap();
    assert!(loaded.settings.paused);
    assert_eq!(loaded.settings.tasks.len(), 1);
    assert!(loaded.warning.is_some());
}
#[test]
fn migration_preserves_id_count_anchor_slots_and_locations() {
    let d = tempfile::tempdir().unwrap();
    let t = task();
    let id = t.id.clone();
    let now = Utc::now();
    let legacy = serde_json::json!({"schemaVersion":1,"logging":{"directory":"logs"},"tasks":[{"id":id,"name":"Legacy","enabled":true,"capture":{"outputFolder":"old captures"},"schedule":{"type":"fixedOnce","times":["09:00:00","15:00:00"]},"stopCondition":{"mode":"count","captureCount":5}}]});
    storage::save_json(&d.path().join("settings.json"), &legacy).unwrap();
    storage::save_json(&d.path().join("runtime-state.json"),&serde_json::json!({"tasks":{id.clone():{"completedCaptures":3,"completedFixedTimes":["09:00:00"],"intervalAnchorUtc":now,"scheduleSignature":"old hash"}}})).unwrap();
    let store = Store::new(d.path().into()).unwrap();
    let mut loaded = store.load().unwrap();
    assert_eq!(loaded.settings.tasks[0].id, id);
    assert_eq!(loaded.settings.schema_version, 2);
    assert_eq!(
        loaded.settings.tasks[0].capture.output_folder,
        d.path().join("old captures").to_string_lossy()
    );
    assert!(store.data.join("legacy-backup/settings.json").exists());
    assert!(d.path().join("settings.json").exists());
    let p = loaded.runtime.tasks.get_mut(&id).unwrap();
    schedule::reconcile(&loaded.settings.tasks[0], p, Utc::now());
    assert_eq!(p.completed_captures, 3);
    assert_eq!(p.completed_fixed_times, vec!["09:00:00"]);
    assert_eq!(p.interval_anchor_utc, Some(now));
}
#[test]
fn corrupt_runtime_does_not_silently_reset_counts() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(d.path().into()).unwrap();
    store.save_settings(&Settings::default()).unwrap();
    fs::write(store.data.join("runtime-state.json"), b"bad").unwrap();
    assert!(store.load().is_err());
}
#[test]
fn startup_interrupted_once_is_failed_without_retry() {
    let d = tempfile::tempdir().unwrap();
    let store = Store::new(d.path().into()).unwrap();
    store.save_settings(&Settings::default()).unwrap();
    let mut r = schedule::Runtime::default();
    r.tasks.insert(
        "x".into(),
        Progress {
            once_result: Some("running".into()),
            ..Default::default()
        },
    );
    store.save_runtime(&r).unwrap();
    assert_eq!(
        store.load().unwrap().runtime.tasks["x"]
            .once_result
            .as_deref(),
        Some("failed")
    );
}
#[test]
fn host_origin_and_token_are_all_required() {
    let mut headers = HashMap::from([
        ("host".into(), "127.0.0.1:1234".into()),
        ("origin".into(), "http://127.0.0.1:1234".into()),
        ("x-app-token".into(), "secret".into()),
    ]);
    assert!(authorized(
        &headers,
        "127.0.0.1:1234",
        "http://127.0.0.1:1234",
        "secret",
        true
    ));
    for key in ["host", "origin", "x-app-token"] {
        let mut bad = headers.clone();
        bad.remove(key);
        assert!(!authorized(
            &bad,
            "127.0.0.1:1234",
            "http://127.0.0.1:1234",
            "secret",
            true
        ));
    }
    headers.insert("origin".into(), "https://evil.example".into());
    assert!(!authorized(
        &headers,
        "127.0.0.1:1234",
        "http://127.0.0.1:1234",
        "secret",
        true
    ));
}
#[test]
fn unwritable_application_folder_reports_move_instruction() {
    let d = tempfile::tempdir().unwrap();
    let file = d.path().join("file");
    fs::write(&file, b"occupied").unwrap();
    let e = Store::new(file).err().unwrap();
    assert!(e.to_string().contains("Move ScheduledScreenshot.exe"));
}

#[cfg(windows)]
#[test]
fn single_instance_guard_is_session_wide_and_released_on_exit() {
    let first = scheduled_screenshot::platform::single_instance()
        .unwrap()
        .unwrap();
    assert!(
        scheduled_screenshot::platform::single_instance()
            .unwrap()
            .is_none()
    );
    drop(first);
    assert!(
        scheduled_screenshot::platform::single_instance()
            .unwrap()
            .is_some()
    );
}

#[test]
fn manual_folder_defaults_persists_and_rejects_invalid_or_stale_edits() {
    let legacy: Settings = serde_json::from_str(r#"{"schemaVersion":2,"tasks":[]}"#).unwrap();
    assert_eq!(legacy.manual_output_folder, "captures");
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path().join("original")).unwrap();
    let app = App::start(store.clone()).unwrap();
    app.notify(Some(false));
    let settings = Settings {
        manual_output_folder: "manual shots/\u{622a}\u{5716}".into(),
        ..Settings::default()
    };
    app.save(settings.clone(), 0).unwrap();
    let saved = fs::read(store.data.join("settings.json")).unwrap();
    for folder in ["", "   ", "bad\0path"] {
        let mut invalid = settings.clone();
        invalid.manual_output_folder = folder.into();
        assert!(app.save(invalid, 1).is_err());
        assert_eq!(fs::read(store.data.join("settings.json")).unwrap(), saved);
    }
    assert!(
        app.save(Settings::default(), 0)
            .unwrap_err()
            .to_string()
            .starts_with("CONFLICT:")
    );
    assert_eq!(
        app.manual_output_folder().unwrap(),
        store.base.join(&settings.manual_output_folder)
    );
    app.shutdown();
    let moved = directory.path().join("moved");
    fs::rename(&store.base, &moved).unwrap();
    let app = App::start(Store::new(moved.clone()).unwrap()).unwrap();
    assert_eq!(
        app.manual_output_folder().unwrap(),
        moved.join(&settings.manual_output_folder)
    );
    let mut absolute = app.state.lock().unwrap().settings.clone();
    absolute.manual_output_folder = directory
        .path()
        .join("outside captures")
        .to_string_lossy()
        .into_owned();
    app.save(absolute.clone(), 1).unwrap();
    assert_eq!(
        app.manual_output_folder().unwrap().to_string_lossy(),
        absolute.manual_output_folder
    );
    assert_eq!(app.snapshot()["settings"]["tasks"], serde_json::json!([]));
    app.shutdown();
    let app = App::start(Store::new(moved).unwrap()).unwrap();
    assert_eq!(
        app.snapshot()["settings"]["manualOutputFolder"],
        absolute.manual_output_folder
    );
    app.shutdown();
}

#[test]
fn capture_shortcut_validation_and_persistence() {
    let mut settings: Settings = serde_json::from_str(r#"{"schemaVersion":2,"tasks":[]}"#).unwrap();
    assert_eq!(settings.capture_shortcut.label(), "Ctrl+Shift+S");
    assert!(settings.capture_shortcut.enabled);
    for modifiers in ["", "win", "ctrl", "ctrl+shift+shift"] {
        settings.capture_shortcut.modifiers = modifiers.into();
        assert!(settings.validate().is_err());
    }
    settings.capture_shortcut.modifiers = "ctrl+alt+shift".into();
    for key in ["", "s", "F0", "F12", "F01", "Escape", "SS"] {
        settings.capture_shortcut.key = key.into();
        assert!(settings.validate().is_err());
    }
    for (key, code) in [("S", 0x53), ("0", 0x30), ("F1", 0x70), ("F11", 0x7a)] {
        settings.capture_shortcut.key = key.into();
        settings.validate().unwrap();
        assert_eq!(settings.capture_shortcut.virtual_key().unwrap(), code);
    }
    let directory = tempfile::tempdir().unwrap();
    let store = Store::new(directory.path().into()).unwrap();
    let app = App::start(store.clone()).unwrap();
    app.notify(Some(false));
    app.save(settings.clone(), 0).unwrap();
    let mut invalid = settings.clone();
    invalid.capture_shortcut.key = "F12".into();
    assert!(app.save(invalid, 1).is_err());
    assert_eq!(
        store.load().unwrap().settings.capture_shortcut,
        settings.capture_shortcut
    );
    app.shutdown();
    let app = App::start(store).unwrap();
    assert_eq!(
        app.state.lock().unwrap().settings.capture_shortcut,
        settings.capture_shortcut
    );
    settings.capture_shortcut.enabled = false;
    app.save(settings, 1).unwrap();
    assert!(app.snapshot()["shortcutActive"].is_null());
    app.shutdown();
}
