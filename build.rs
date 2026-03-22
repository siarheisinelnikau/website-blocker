fn main() {
    println!("cargo:rerun-if-changed=assets/website-blocker.ico");
    println!("cargo:rerun-if-changed=Cargo.toml");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/website-blocker.ico")
            .set("ProductName", "WebsiteBlocker")
            .set("FileDescription", "WebsiteBlocker")
            .set(
                "OriginalFilename",
                &format!("WebsiteBlocker-v{}.exe", env!("CARGO_PKG_VERSION")),
            )
            .compile_for("website-blocker")
            .expect("WebsiteBlocker icon/version resource compilation failed");
    }
}
