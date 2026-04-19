//! WebsiteBlocker configuration for the shared portable updater.
use rust_auto_updator::Config;
use std::path::PathBuf;

pub use rust_auto_updator::{Prepared, Release as Update};
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn config() -> Result<Config, String> {
    let mut config = Config::new(
        "siarheisinelnikau",
        "website-blocker",
        VERSION,
        "WebsiteBlocker",
    )?;
    config.checksum_name = "checksums.txt".into();
    config.max_executable_size = 200 * 1024 * 1024;
    config.expected_subsystem = Some(2); // Windows GUI executable.
    config.install.stage_prefix = ".websiteblocker-update-".into();
    Ok(config)
}

pub fn check() -> Result<Option<Update>, String> {
    rust_auto_updator::check_latest(&config()?)
}

pub fn download(
    update: &Update,
    progress: impl FnMut(u64, Option<u64>),
) -> Result<Prepared, String> {
    let mut progress = progress;
    rust_auto_updator::prepare_download(update, &config()?, |done, total| {
        progress(done, Some(total))
    })
}

pub fn prepare_install(mut prepared: Prepared) -> Result<(), String> {
    rust_auto_updator::handoff(&mut prepared)
}

pub fn apply_helper() -> Result<(), String> {
    rust_auto_updator::apply_update(&config()?)
}

pub fn cleanup(directory: PathBuf) -> Result<(), String> {
    rust_auto_updator::cleanup_after_update(directory, config()?.install);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_release_contract_accepts_checksums_txt_and_rejects_untrusted_assets() {
        let mut config = config().unwrap();
        config.current_version = "0.1.0".parse().unwrap();
        let base = config.repository_url();
        let name = "WebsiteBlocker-v0.4.0.exe";
        let mut release = serde_json::json!({"tag_name":"v0.4.0", "html_url":format!("{base}/releases/tag/v0.4.0"),
            "draft":false,"prerelease":false,"assets":[
                {"name":name,"size":2048,"browser_download_url":format!("{base}/releases/download/v0.4.0/{name}")},
                {"name":"checksums.txt","size":128,"browser_download_url":format!("{base}/releases/download/v0.4.0/checksums.txt")}]});
        assert!(
            rust_auto_updator::parse_release(&release.to_string(), &config)
                .unwrap()
                .is_some()
        );
        release["assets"][0]["browser_download_url"] = "https://example.com/app.exe".into();
        assert!(rust_auto_updator::parse_release(&release.to_string(), &config).is_err());
    }
}
