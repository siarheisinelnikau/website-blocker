//! Headless developer fixture for exercising the exact GUI setup backend.
//! Never run implicitly: this explicitly writes the per-user native-host registration.
use std::path::PathBuf;
use website_blocker::{
    setup::{self, Browser, UserRegistry},
    store::Store,
};
fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 && args.len() != 4 {
        return Err(
            "Expected Chrome|Edge, absolute fixture data directory, absolute target EXE.".into(),
        );
    }
    let browser = match args[0].as_str() {
        "Chrome" => Browser::Chrome,
        "Edge" => Browser::Edge,
        _ => return Err("Unknown browser".into()),
    };
    let store = Store {
        dir: PathBuf::from(&args[1]),
    };
    if !store.dir.is_absolute() {
        return Err("An absolute fixture directory is required.".into());
    }
    if args.get(3).is_some_and(|mode| mode == "verify") {
        let status = setup::inspect(&store, &UserRegistry, chrono::Utc::now().timestamp())?;
        let status = status.get(browser);
        if !status.prepared || !status.connected || !status.up_to_date {
            return Err(format!("Connection was not confirmed: {status:?}"));
        }
        println!("Verified applied connection for {}", browser.label());
        return Ok(());
    }
    let folder = setup::prepare(&store, browser, &PathBuf::from(&args[2]), &UserRegistry)?;
    println!("{}", folder.display());
    Ok(())
}
