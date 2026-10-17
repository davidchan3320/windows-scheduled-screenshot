fn main() {
    println!("cargo:rerun-if-changed=resources/app.manifest");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let manifest = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("resources/app.manifest");
        println!("cargo:rustc-link-arg-bin=ScheduledScreenshot=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=ScheduledScreenshot=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
