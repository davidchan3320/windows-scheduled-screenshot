#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        scheduled_screenshot::platform::message(&format!("{error:#}"));
    }
}
#[cfg(windows)]
fn run() -> anyhow::Result<()> {
    use scheduled_screenshot::{engine::App, http::Server, platform, storage::Store};
    let exe = std::env::current_exe()?;
    let base = exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot locate executable directory"))?
        .to_path_buf();
    let Some(instance) = platform::single_instance()? else {
        return Ok(());
    };
    let app = App::start(Store::new(base)?)?;
    let server = Server::start(app.clone())?;
    if std::env::args().any(|s| s == "--print-url") {
        println!("{}", server.url);
    }
    let result = platform::run_tray(
        app.clone(),
        server.url,
        &instance,
        std::env::args().any(|s| s == "--background"),
    );
    app.shutdown();
    let restart = result?;
    // Release the session-wide instance guard only after captures have stopped.
    drop(instance);
    if restart {
        use std::os::windows::process::CommandExt;
        let mut command = std::process::Command::new(exe);
        if std::env::args().any(|arg| arg == "--print-url") {
            command.arg("--print-url");
        }
        command.creation_flags(0x08000000).spawn()?; // CREATE_NO_WINDOW
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Scheduled Screenshot requires Windows 10/11 x64.");
}
