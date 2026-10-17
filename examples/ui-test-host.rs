//! Isolated HTTP/UI host for CI; no tray, startup changes, or desktop capture.
use scheduled_screenshot::{engine::App, http::Server, storage::Store};
use std::io::{self, Write};
fn main() -> anyhow::Result<()> {
    let directory = tempfile::tempdir()?;
    let app = App::start(Store::new(directory.path().into())?)?;
    app.notify(Some(false));
    let server = Server::start(app.clone())?;
    println!("{}", server.url);
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    app.shutdown();
    Ok(())
}
