#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod desktop;
mod ui;

use website_blocker::{
    native,
    store::{Store, data_dir},
    updater,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("WebsiteBlocker: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    // The downloaded executable serves as the shared helper before normal startup.
    if args.first().is_some_and(|arg| arg == "--apply-update") {
        return updater::apply_helper();
    }
    let cleanup = if args.first().is_some_and(|arg| arg == "--cleanup-update") {
        if args.len() != 2 {
            return Err("Invalid update cleanup arguments.".into());
        }
        Some(std::path::PathBuf::from(&args[1]))
    } else {
        None
    };
    let store = Store { dir: data_dir()? };
    if args
        .first()
        .is_some_and(|arg| native::extension_origin(arg))
    {
        return native::serve(std::io::stdin().lock(), std::io::stdout().lock(), &store);
    }
    if let Some(directory) = cleanup {
        updater::cleanup(directory)?;
    } else if !args.is_empty() {
        return Err(
            "Unknown arguments. Native messaging accepts a browser extension origin only.".into(),
        );
    }
    desktop::run(store)
}
