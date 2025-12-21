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
    let store = Store { dir: data_dir()? };
    if args
        .first()
        .is_some_and(|arg| native::extension_origin(arg))
    {
        return native::serve(std::io::stdin().lock(), std::io::stdout().lock(), &store);
    }
    if args.first().is_some_and(|arg| arg == "--apply-update") {
        if args.len() != 4 {
            return Err("Invalid updater arguments.".into());
        }
        let result = updater::apply_helper(
            args[1].parse().map_err(|_| "Invalid process ID.")?,
            std::path::Path::new(&args[2]),
            std::path::Path::new(&args[3]),
            &store,
        );
        if let Err(error) = &result {
            let _ = website_blocker::store::atomic_write(
                &store.dir.join("updates/install-result.txt"),
                error.as_bytes(),
            );
        }
        return result;
    }
    if !args.is_empty() {
        return Err(
            "Unknown arguments. Native messaging accepts a browser extension origin only.".into(),
        );
    }
    desktop::run(store)
}
