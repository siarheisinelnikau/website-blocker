use crate::store::{Store, atomic_write};
use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{fs::{self, File}, io::{Read, Write}, path::{Path, PathBuf}, time::Duration};

pub const REPOSITORY: &str = "siarheisinelnikau/website-blocker";
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
const MAX_DOWNLOAD: u64 = 200 * 1024 * 1024;

#[derive(Clone, Debug)]
pub struct Update { pub version: String, pub name: String, pub url: String, pub checksums_url: String }

#[derive(Deserialize)]
struct Release { tag_name: String, draft: bool, prerelease: bool, assets: Vec<Asset> }
#[derive(Deserialize)]
struct Asset { name: String, browser_download_url: String }

fn agent(timeout: u64) -> ureq::Agent {
    ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(timeout))).build().into()
}

fn official_url(url: &str) -> bool {
    url.starts_with(&format!("https://github.com/{REPOSITORY}/releases/download/"))
        && !url.contains(['?', '#', '\\'])
}

pub fn parse_release(json: &str, installed: &str) -> Result<Option<Update>, String> {
    let release: Release = serde_json::from_str(json).map_err(|e| format!("Invalid release response: {e}"))?;
    if release.draft || release.prerelease { return Ok(None); }
    let version = Version::parse(release.tag_name.strip_prefix('v').unwrap_or(&release.tag_name)).map_err(|e| e.to_string())?;
    if !version.pre.is_empty() || version <= Version::parse(installed).map_err(|e| e.to_string())? { return Ok(None); }
    let name = format!("WebsiteBlocker-v{version}.exe");
    let asset = release.assets.iter().find(|a| a.name == name).ok_or("The release has no compatible Windows standalone executable.")?;
    let checksums = release.assets.iter().find(|a| a.name == "checksums.txt").ok_or("The release has no checksums.txt; installation is unavailable.")?;
    if !official_url(&asset.browser_download_url) || !official_url(&checksums.browser_download_url) { return Err("Release asset URL is outside the canonical repository.".into()); }
    Ok(Some(Update { version: version.to_string(), name, url: asset.browser_download_url.clone(), checksums_url: checksums.browser_download_url.clone() }))
}

pub fn check() -> Result<Option<Update>, String> {
    if !cfg!(all(windows, target_arch = "x86_64")) { return Err("Automatic installation currently supports Windows x64 only.".into()); }
    let mut response = agent(30).get(&format!("https://api.github.com/repos/{REPOSITORY}/releases/latest"))
        .header("User-Agent", "WebsiteBlocker-updater").header("Accept", "application/vnd.github+json")
        .call().map_err(|e| format!("Update check unavailable: {e}. A 404 means the repository or release is not published; 403/429 may mean rate limiting."))?;
    let text = response.body_mut().with_config().limit(2 * 1024 * 1024).read_to_string().map_err(|e| e.to_string())?;
    parse_release(&text, VERSION)
}

pub fn checksum(text: &str, name: &str) -> Result<String, String> {
    let mut found = None;
    for line in text.lines() {
        let parts: Vec<_> = line.split_whitespace().collect();
        if parts.len() == 2 && parts[1].trim_start_matches('*') == name {
            let hash = parts[0];
            if found.is_some() || hash.len() != 64 || !hash.bytes().all(|c| c.is_ascii_hexdigit()) { return Err("Invalid or duplicate checksum entry.".into()); }
            found = Some(hash.to_ascii_lowercase());
        }
    }
    found.ok_or("Executable checksum is missing.".into())
}

pub fn download(update: &Update, store: &Store, progress: impl Fn(u64, Option<u64>)) -> Result<PathBuf, String> {
    if !official_url(&update.url) || !official_url(&update.checksums_url) { return Err("Invalid update URL.".into()); }
    let client = agent(180);
    let text = client.get(&update.checksums_url).call().map_err(|e| e.to_string())?
        .body_mut().with_config().limit(65536).read_to_string().map_err(|e| e.to_string())?;
    let expected = checksum(&text, &update.name)?;
    let dir = store.dir.join("updates");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut partial = tempfile::NamedTempFile::new_in(&dir).map_err(|e| e.to_string())?;
    let mut response = client.get(&update.url).call().map_err(|e| e.to_string())?;
    let size = response.headers().get("Content-Length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse::<u64>().ok());
    if size.is_some_and(|v| v > MAX_DOWNLOAD) { return Err("Update exceeds the 200 MiB limit.".into()); }
    let mut reader = response.body_mut().as_reader();
    let mut buffer = [0u8; 65536];
    let mut hash = Sha256::new();
    let mut received = 0;
    let mut last_progress = std::time::Instant::now();
    loop {
        let count = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 { break; }
        received += count as u64;
        if received > MAX_DOWNLOAD { return Err("Update exceeds the 200 MiB limit.".into()); }
        partial.write_all(&buffer[..count]).map_err(|e| e.to_string())?;
        hash.update(&buffer[..count]);
        if last_progress.elapsed() >= Duration::from_millis(200) {
            progress(received, size); last_progress = std::time::Instant::now();
        }
    }
    if received < 2 || size.is_some_and(|size| received != size) || format!("{:x}", hash.finalize()) != expected { return Err("Incomplete download or SHA-256 mismatch. The installed executable was preserved.".into()); }
    partial.as_file().sync_all().map_err(|e| e.to_string())?;
    let mut file = File::open(partial.path()).map_err(|e| e.to_string())?;
    let mut magic = [0; 2]; file.read_exact(&mut magic).map_err(|e| e.to_string())?;
    if magic != *b"MZ" { return Err("The downloaded file is not a Windows executable.".into()); }
    let path = dir.join(&update.name);
    partial.persist(&path).map_err(|e| e.to_string())?;
    progress(received, size);
    Ok(path)
}

/// Called only after the user clicks Download and install and the verified file is ready.
pub fn prepare_install(staged: &Path, store: &Store) -> Result<(), String> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let target = std::env::current_exe().map_err(|e| e.to_string())?;
        let helper = store.dir.join("updates").join(format!("apply-{}.exe", std::process::id()));
        fs::copy(&target, &helper).map_err(|e| format!("Cannot prepare updater helper: {e}"))?;
        std::process::Command::new(helper).arg("--apply-update").arg(std::process::id().to_string()).arg(target).arg(staged)
            .creation_flags(0x08000000).spawn().map_err(|e| format!("Cannot start updater: {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    { let _ = (staged, store); Err("Automatic installation supports Windows only.".into()) }
}

pub fn replace_file(target: &Path, staged: &Path, backup: &Path) -> Result<(), String> {
    if backup.exists() { return Err("An update backup already exists. Preserve it and resolve the previous update first.".into()); }
    // Stage on the target volume before touching the working installation.
    let parent = target.parent().ok_or("Missing installation directory.")?;
    let mut replacement = tempfile::NamedTempFile::new_in(parent).map_err(|e| format!("Installation directory is not writable: {e}"))?;
    let mut source = File::open(staged).map_err(|e| e.to_string())?;
    std::io::copy(&mut source, &mut replacement).map_err(|e| e.to_string())?;
    replacement.as_file().sync_all().map_err(|e| e.to_string())?;
    fs::rename(target, backup).map_err(|e| e.to_string())?;
    if let Err(error) = replacement.persist(target) {
        let rollback = fs::rename(backup, target);
        return Err(format!("Update failed: {error}; rollback: {rollback:?}"));
    }
    Ok(())
}

pub fn apply_helper(pid: u32, target: &Path, staged: &Path, store: &Store) -> Result<(), String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::{Foundation::{CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, WAIT_OBJECT_0}, System::Threading::{OpenProcess, WaitForSingleObject, PROCESS_SYNCHRONIZE}};
        // SAFETY: Only wait on a process handle; never terminate or inject into it.
        unsafe {
            let handle = OpenProcess(PROCESS_SYNCHRONIZE, 0, pid);
            if !handle.is_null() {
                let result = WaitForSingleObject(handle, 120_000); CloseHandle(handle);
                if result != WAIT_OBJECT_0 { return Err("Application did not exit within two minutes. Close it and retry.".into()); }
            } else if GetLastError() != ERROR_INVALID_PARAMETER { return Err("Cannot wait for the running application to exit.".into()); }
        }
    }
    #[cfg(not(windows))]
    { let _ = (pid, target, staged, store); return Err("Windows updater helper only.".into()); }
    #[cfg(windows)]
    {
        let staged_root = store.dir.join("updates").canonicalize().map_err(|e| e.to_string())?;
        if staged.canonicalize().map_err(|e| e.to_string())?.parent() != Some(staged_root.as_path())
            || target.extension().is_none_or(|v| v != "exe") || target == staged
        { return Err("Invalid update paths.".into()); }
        let backup = target.with_extension("exe.update-backup");
        replace_file(target, staged, &backup)?;
        if let Err(error) = std::process::Command::new(target).spawn() {
            let _ = fs::remove_file(target);
            let rollback = fs::rename(&backup, target);
            return Err(format!("Cannot restart: {error}; rollback: {rollback:?}"));
        }
        let _ = fs::remove_file(backup);
        let _ = fs::remove_file(staged);
        let _ = atomic_write(&store.dir.join("updates/install-result.txt"), b"Update installed and restart requested.");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(tag: &str, name: &str) -> String {
        serde_json::json!({ "tag_name": tag, "draft": false, "prerelease": false, "assets": [
            {"name": name, "browser_download_url": format!("https://github.com/{REPOSITORY}/releases/download/{tag}/{name}")},
            {"name":"checksums.txt", "browser_download_url":format!("https://github.com/{REPOSITORY}/releases/download/{tag}/checksums.txt")}
        ]}).to_string()
    }
    #[test]
    fn semantic_versions_assets_and_checksums() {
        assert!(parse_release(&release("v0.10.0", "WebsiteBlocker-v0.10.0.exe"), "0.9.0").unwrap().is_some());
        assert!(parse_release(&release("v0.1.0", "WebsiteBlocker-v0.1.0.exe"), "0.2.0").unwrap().is_none());
        assert!(parse_release(&release("v0.2.0-beta.1", "x"), "0.1.0").unwrap().is_none());
        assert!(parse_release(&release("v0.2.0", "wrong.exe"), "0.1.0").is_err());
        let hash = "ab".repeat(32);
        assert_eq!(checksum(&format!("{hash}  file.exe"), "file.exe").unwrap(), hash);
        assert!(checksum("abc file.exe", "file.exe").is_err());
        assert!(checksum(&format!("{hash} file.exe\n{hash} file.exe"), "file.exe").is_err());
    }
    #[test]
    fn replacement_preserves_original_when_staging_fails() {
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("app.exe"); let staged = temp.path().join("new.exe"); let backup = temp.path().join("backup.exe");
        fs::write(&target, b"old").unwrap();
        assert!(replace_file(&target, &staged, &backup).is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        fs::write(&staged, b"new").unwrap(); replace_file(&target, &staged, &backup).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"new"); assert_eq!(fs::read(&backup).unwrap(), b"old");
    }
}
