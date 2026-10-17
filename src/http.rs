use crate::{
    config::{Settings, Task},
    engine::App,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

const HTML: &str = include_str!("../ui/index.html");
const JS: &str = include_str!("../ui/app.js");
const I18N: &str = include_str!("../ui/i18n.js");
const CSS: &str = include_str!("../ui/style.css");
const NOTICES: &str = include_str!("../THIRD_PARTY_NOTICES.txt");
const MAX_BODY: usize = 1_048_576;
pub struct Server {
    pub url: String,
}
struct Request {
    method: String,
    path: String,
    headers: HashMap<String, String>,
    body: Vec<u8>,
}
fn read_request(stream: &mut TcpStream) -> Result<Request> {
    stream.set_read_timeout(Some(Duration::from_secs(3)))?;
    stream.set_write_timeout(Some(Duration::from_secs(3)))?;
    let start = Instant::now();
    let mut reader = BufReader::new(stream);
    let mut head = Vec::new();
    loop {
        ensure!(
            head.len() < 16_384 && start.elapsed() < Duration::from_secs(4),
            "Request headers too large or slow."
        );
        let mut b = [0];
        reader.read_exact(&mut b)?;
        head.push(b[0]);
        if head.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let text = std::str::from_utf8(&head)?;
    let mut lines = text.split("\r\n");
    let first: Vec<_> = lines
        .next()
        .context("Missing request")?
        .split(' ')
        .collect();
    ensure!(
        first.len() == 3 && first[2] == "HTTP/1.1",
        "Unsupported HTTP request."
    );
    let mut headers = HashMap::new();
    for line in lines.filter(|s| !s.is_empty()) {
        let (key, value) = line.split_once(':').context("Invalid header")?;
        ensure!(
            !key.is_empty() && key.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'),
            "Invalid header name."
        );
        ensure!(
            headers
                .insert(key.to_ascii_lowercase(), value.trim().to_string())
                .is_none(),
            "Duplicate header."
        );
    }
    ensure!(
        !headers.contains_key("transfer-encoding") && !headers.contains_key("expect"),
        "Unsupported transfer encoding."
    );
    let length: usize = headers.get("content-length").map_or(Ok(0), |s| s.parse())?;
    ensure!(length <= MAX_BODY, "Request body exceeds 1 MB.");
    let mut body = Vec::with_capacity(length);
    while body.len() < length {
        ensure!(
            start.elapsed() < Duration::from_secs(5),
            "Request body too slow."
        );
        let available = reader.fill_buf()?;
        ensure!(!available.is_empty(), "Incomplete request.");
        let n = available.len().min(length - body.len());
        body.extend_from_slice(&available[..n]);
        reader.consume(n);
    }
    Ok(Request {
        method: first[0].into(),
        path: first[1].into(),
        headers,
        body,
    })
}
fn reply(stream: &mut TcpStream, status: u16, content_type: &str, body: &[u8]) {
    let reason = match status {
        200 => "OK",
        202 => "Accepted",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        409 => "Conflict",
        _ => "Error",
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nContent-Security-Policy: default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(body);
}
pub fn authorized(
    headers: &HashMap<String, String>,
    host: &str,
    origin: &str,
    token: &str,
    api: bool,
) -> bool {
    headers.get("host").is_some_and(|s| s == host)
        && headers.get("origin").is_none_or(|s| s == origin)
        && headers
            .get("sec-fetch-site")
            .is_none_or(|s| matches!(s.as_str(), "same-origin" | "none"))
        && (!api
            || (headers.get("origin").is_some_and(|s| s == origin)
                && headers.get("x-app-token").is_some_and(|s| s == token)))
}
impl Server {
    pub fn start(app: App) -> Result<Self> {
        let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
        let host = listener.local_addr()?.to_string();
        let origin = format!("http://{host}");
        let token = format!(
            "{}{}",
            uuid::Uuid::new_v4().simple(),
            uuid::Uuid::new_v4().simple()
        );
        let url = format!("{origin}/#token={token}");
        let picker_busy = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::sync_channel::<TcpStream>(8);
        let rx = Arc::new(Mutex::new(rx));
        for n in 0..4 {
            let (rx, app, host, origin, token, picker_busy) = (
                rx.clone(),
                app.clone(),
                host.clone(),
                origin.clone(),
                token.clone(),
                picker_busy.clone(),
            );
            thread::Builder::new().name(format!("http-{n}")).spawn(move || loop {
                let Ok(mut stream) = rx.lock().unwrap().recv() else { break; };
                if app.stopping.load(Ordering::SeqCst) { break; }
                let request = match read_request(&mut stream) {
                    Ok(r) => r,
                    Err(_) => { reply(&mut stream, 400, "application/json", br#"{"error":"Invalid or oversized request."}"#); continue; }
                };
                let api = request.path.starts_with("/api/");
                if !authorized(&request.headers, &host, &origin, &token, api) { reply(&mut stream,403,"application/json", br#"{"error":"Open the task manager from the tray icon to reconnect."}"#); continue; }
                if !api {
                    let asset = match (request.method.as_str(), request.path.as_str()) {
                        ("GET", "/") => Some(("text/html; charset=utf-8", HTML)),
                        ("GET", "/app.js") => Some(("text/javascript; charset=utf-8", JS)),
                        ("GET", "/i18n.js") => Some(("text/javascript; charset=utf-8", I18N)),
                        ("GET", "/locales/en.json") => Some(("application/json; charset=utf-8", crate::localization::EN)),
                        ("GET", "/locales/zh-Hant.json") => Some(("application/json; charset=utf-8", crate::localization::ZH_HANT)),
                        ("GET", "/style.css") => Some(("text/css; charset=utf-8", CSS)),
                        ("GET", "/licenses.txt") => Some(("text/plain; charset=utf-8", NOTICES)), _ => None,
                    };
                    if let Some((kind, body)) = asset { reply(&mut stream,200,kind,body.as_bytes()); } else { reply(&mut stream,404,"text/plain",b"Not found"); }
                    continue;
                }
                if request.method != "POST" || !request.headers.get("content-type").is_some_and(|s| s == "application/json") { reply(&mut stream,400,"application/json",br#"{"error":"Use POST with application/json."}"#); continue; }
                let result = serde_json::from_slice::<Value>(&request.body).map_err(anyhow::Error::from).and_then(|v| route(&app, &request.path, v, &picker_busy));
                let (status, body) = match result {
                    Ok(value) => (200, value),
                    Err(e) => { let msg = format!("{e:#}"); (if msg.starts_with("CONFLICT:") { 409 } else { 400 }, json!({"error": msg})) },
                };
                reply(&mut stream,status,"application/json", &serde_json::to_vec(&body).unwrap());
                // Send the response before stopping the tray and scheduler.
                #[cfg(windows)]
                if status == 200 && request.path == "/api/restart"
                    && let Err(error) = crate::platform::request_restart() {
                    app.store.log(&format!("Could not request restart: {error:#}"));
                }
            })?;
        }
        thread::Builder::new()
            .name("http-listener".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if app.stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(stream) = stream {
                        let _ = tx.try_send(stream);
                    }
                }
            })?;
        Ok(Self { url })
    }
}
fn route(app: &App, path: &str, v: Value, picker_busy: &AtomicBool) -> Result<Value> {
    match path {
        "/api/restart" => {
            ensure!(
                v.as_object().is_some_and(|body| body.is_empty()),
                "Restart does not accept options."
            );
            #[cfg(windows)]
            {
                crate::platform::restart_window()?;
                Ok(json!({"accepted": true}))
            }
            #[cfg(not(windows))]
            anyhow::bail!("Application restart requires Windows")
        }
        "/api/status" | "/api/config" => Ok(app.snapshot()),
        "/api/export" => Ok(serde_json::to_value(&app.state.lock().unwrap().settings)?),
        "/api/validate-import" | "/api/import" => {
            let value = v.get("settings").context("Missing settings")?;
            ensure!(
                value.get("schemaVersion").and_then(Value::as_u64) == Some(2),
                "Import requires a schema version 2 configuration."
            );
            ensure!(
                value.get("tasks").is_some_and(Value::is_array),
                "Import requires a tasks array."
            );
            let mut settings: Settings = serde_json::from_value(value.clone())?;
            settings.validate()?;
            crate::output::path_for(
                &app.store.base,
                &settings.manual_capture_task(),
                chrono::Local::now(),
                "DISPLAY1",
                1,
            )?;
            for task in &settings.tasks {
                crate::output::path_for(
                    &app.store.base,
                    task,
                    chrono::Local::now(),
                    "DISPLAY1",
                    1,
                )?;
            }
            // Imported enabled tasks must not run before the user reviews them.
            settings.paused = true;
            if path == "/api/validate-import" {
                Ok(serde_json::to_value(settings)?)
            } else {
                app.save(
                    settings,
                    v.get("revision")
                        .and_then(Value::as_u64)
                        .context("Missing revision")?,
                )
            }
        }
        "/api/save" => {
            let settings: Settings =
                serde_json::from_value(v.get("settings").context("Missing settings")?.clone())?;
            app.save(
                settings,
                v.get("revision")
                    .and_then(Value::as_u64)
                    .context("Missing revision")?,
            )
        }
        "/api/pause" => app.pause(
            v.get("paused")
                .and_then(Value::as_bool)
                .context("Missing paused state")?,
            v.get("revision")
                .and_then(Value::as_u64)
                .context("Missing revision")?,
        ),
        "/api/capture" => {
            ensure!(
                v.as_object().is_some_and(|body| body.is_empty()),
                "Capture now does not accept a task."
            );
            app.capture_now()?;
            Ok(json!({"accepted":true}))
        }
        "/api/open-captures" => {
            #[cfg(windows)]
            {
                crate::platform::open_folder(&app.manual_output_folder()?)?;
                Ok(json!({"opened": true}))
            }
            #[cfg(not(windows))]
            {
                anyhow::bail!("Opening the capture folder requires Windows")
            }
        }
        "/api/open-folder" => {
            let id = v
                .get("id")
                .and_then(Value::as_str)
                .context("Missing task ID")?;
            let folder = app
                .state
                .lock()
                .unwrap()
                .settings
                .tasks
                .iter()
                .find(|task| task.id == id)
                .context("Task no longer exists. Refresh the task list.")?
                .capture
                .output_folder
                .clone();
            let folder = crate::output::resolve_folder(&app.store.base, &folder)?;
            #[cfg(windows)]
            {
                crate::platform::open_folder(&folder)?;
                Ok(json!({"opened": true}))
            }
            #[cfg(not(windows))]
            {
                anyhow::bail!("Opening {} requires Windows", folder.display())
            }
        }
        "/api/preview" => {
            let task: Task =
                serde_json::from_value(v.get("task").context("Missing task")?.clone())?;
            task.validate()?;
            Ok(
                json!({"path": crate::output::path_for(&app.store.base, &task, chrono::Local::now(), "DISPLAY1", 1)?}),
            )
        }
        "/api/folder" => {
            ensure!(
                !picker_busy.swap(true, Ordering::SeqCst),
                "A folder picker is already open."
            );
            struct Reset<'a>(&'a AtomicBool);
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.0.store(false, Ordering::SeqCst);
                }
            }
            let _reset = Reset(picker_busy);
            #[cfg(windows)]
            {
                let language = app.state.lock().unwrap().settings.language.clone();
                Ok(json!({"path": crate::platform::choose_folder(&language)?}))
            }
            #[cfg(not(windows))]
            {
                anyhow::bail!("Folder picker requires Windows")
            }
        }
        _ => anyhow::bail!("Unknown API endpoint."),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::Store;

    #[test]
    fn configuration_transfer_validates_and_preserves_revision_and_progress() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::new(directory.path().into()).unwrap();
        let app = App::start(store.clone()).unwrap();
        app.notify(Some(false));
        let picker = AtomicBool::new(false);
        let task = Task {
            name: "工作日截圖".into(),
            ..Task::default()
        };
        let settings = Settings {
            language: "zh-Hant".into(),
            tasks: vec![task.clone()],
            ..Settings::default()
        };
        app.save(settings, 0).unwrap();
        {
            let mut state = app.state.lock().unwrap();
            state
                .runtime
                .tasks
                .get_mut(&task.id)
                .unwrap()
                .completed_captures = 3;
        }
        let exported = route(&app, "/api/export", json!({}), &picker).unwrap();
        assert_eq!(exported, app.snapshot()["settings"]);
        assert!(exported.get("runtime").is_none());
        let disk_before = std::fs::read(store.data.join("settings.json")).unwrap();
        for invalid in [
            json!({}),
            json!({"schemaVersion": 1, "tasks": []}),
            json!({"schemaVersion": 2}),
            json!({"schemaVersion": 2, "tasks": null}),
            json!({"schemaVersion": 2, "tasks": [], "unknown": true}),
            json!({"schemaVersion": 2, "tasks": [task.clone(), task.clone()]}),
            json!({"schemaVersion": 2, "tasks": [{"capture": {"imageFormat": "gif"}}]}),
            json!({"schemaVersion": 2, "tasks": [{"capture": {"fileNameTemplate": "../bad"}}]}),
        ] {
            for endpoint in ["/api/validate-import", "/api/import"] {
                assert!(
                    route(
                        &app,
                        endpoint,
                        json!({"settings": invalid, "revision": 1}),
                        &picker
                    )
                    .is_err()
                );
            }
            assert_eq!(
                route(&app, "/api/export", json!({}), &picker).unwrap(),
                exported
            );
            assert_eq!(
                std::fs::read(store.data.join("settings.json")).unwrap(),
                disk_before
            );
        }
        let body = json!({"settings": exported, "revision": 1});
        let preview = route(&app, "/api/validate-import", body.clone(), &picker).unwrap();
        assert_eq!(preview["paused"], true);
        assert_eq!(
            std::fs::read(store.data.join("settings.json")).unwrap(),
            disk_before
        );
        app.pause(true, 1).unwrap();
        let conflict = route(&app, "/api/import", body, &picker).unwrap_err();
        assert!(conflict.to_string().starts_with("CONFLICT:"));
        // The source revision belongs to another installation and is ignored.
        let mut imported = exported.clone();
        imported["revision"] = json!(99999);
        let result = route(
            &app,
            "/api/import",
            json!({"settings": imported, "revision": 2}),
            &picker,
        )
        .unwrap();
        assert_eq!(result["settings"]["revision"], 3);
        assert_eq!(result["settings"]["paused"], true);
        assert_eq!(result["settings"]["tasks"], exported["tasks"]);
        assert_eq!(
            app.state.lock().unwrap().runtime.tasks[&task.id].completed_captures,
            3
        );
        assert_eq!(
            serde_json::to_value(store.load().unwrap().settings).unwrap(),
            result["settings"]
        );
        app.shutdown();
        let restarted = App::start(store).unwrap();
        assert_eq!(restarted.snapshot()["settings"], result["settings"]);
        let empty = route(
            &restarted,
            "/api/import",
            json!({"settings": {"schemaVersion": 2, "tasks": []}, "revision": 3}),
            &picker,
        )
        .unwrap();
        assert_eq!(empty["settings"]["tasks"], json!([]));
        assert_eq!(empty["settings"]["paused"], true);
        restarted.shutdown();
    }
}
