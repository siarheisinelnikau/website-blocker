#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod ui;
mod window_loop;

use website_blocker::{
    native,
    store::{Store, data_dir},
    updater,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("WebsiteBlocker: {error}");
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
    let event_loop = winit::event_loop::EventLoop::<eframe::UserEvent>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    let app = eframe::create_native(
        "WebsiteBlocker",
        eframe::NativeOptions {
            viewport: eframe::egui::ViewportBuilder::default()
                .with_inner_size([980.0, 740.0])
                .with_min_inner_size([760.0, 580.0]),
            ..Default::default()
        },
        Box::new(move |cc| Ok(Box::new(ui::App::new(cc, store)))),
        &event_loop,
    );
    event_loop
        .run_app(&mut window_loop::Reactive(app))
        .map_err(|e| e.to_string())
}
