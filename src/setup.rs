//! Explicit user-initiated setup. No browser preference edits, policies or silent extension install.
use crate::{
    native::HOST_NAME,
    store::{Store, atomic_write, read_limited},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    process::Command,
};

pub fn extension_id() -> &'static str {
    include_str!("../extension/identity.txt").trim()
}
pub const BUNDLE_FILES: &[(&str, &[u8])] = &[
    (
        "manifest.json",
        include_bytes!("../extension/manifest.json"),
    ),
    (
        "background.js",
        include_bytes!("../extension/background.js"),
    ),
    ("engine.js", include_bytes!("../extension/engine.js")),
    ("popup.html", include_bytes!("../extension/popup.html")),
    ("popup.js", include_bytes!("../extension/popup.js")),
    ("blocked.html", include_bytes!("../extension/blocked.html")),
    ("blocked.js", include_bytes!("../extension/blocked.js")),
    ("blocked.css", include_bytes!("../extension/blocked.css")),
    ("style.css", include_bytes!("../extension/style.css")),
    ("welcome.html", include_bytes!("../extension/welcome.html")),
    ("welcome.js", include_bytes!("../extension/welcome.js")),
    ("icon16.png", include_bytes!("../extension/icon16.png")),
    ("icon32.png", include_bytes!("../extension/icon32.png")),
    ("icon48.png", include_bytes!("../extension/icon48.png")),
    ("icon128.png", include_bytes!("../extension/icon128.png")),
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Edge,
}
impl Browser {
    pub fn label(self) -> &'static str {
        match self {
            Self::Chrome => "Chrome",
            Self::Edge => "Edge",
        }
    }
    pub fn slug(self) -> &'static str {
        match self {
            Self::Chrome => "chrome",
            Self::Edge => "edge",
        }
    }
    fn registry_path(self) -> String {
        format!(
            "Software\\{}\\NativeMessagingHosts\\{HOST_NAME}",
            match self {
                Self::Chrome => "Google\\Chrome",
                Self::Edge => "Microsoft\\Edge",
            }
        )
    }
    pub fn extensions_url(self) -> &'static str {
        match self {
            Self::Chrome => "chrome://extensions/",
            Self::Edge => "edge://extensions/",
        }
    }
    pub fn executable(self) -> Option<PathBuf> {
        let relative = match self {
            Self::Chrome => "Google/Chrome/Application/chrome.exe",
            Self::Edge => "Microsoft/Edge/Application/msedge.exe",
        };
        ["PROGRAMFILES", "PROGRAMFILES(X86)", "LOCALAPPDATA"]
            .into_iter()
            .filter_map(std::env::var_os)
            .map(|root| PathBuf::from(root).join(relative))
            .find(|path| path.is_file())
    }
}

pub trait Registration {
    fn read(&self, browser: Browser) -> Result<Option<PathBuf>, String>;
    fn write(&self, browser: Browser, manifest: &Path) -> Result<(), String>;
}

pub struct UserRegistry;
impl Registration for UserRegistry {
    fn read(&self, browser: Browser) -> Result<Option<PathBuf>, String> {
        platform::read(&browser.registry_path())
    }
    fn write(&self, browser: Browser, manifest: &Path) -> Result<(), String> {
        platform::write(&browser.registry_path(), manifest)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeManifest {
    name: String,
    description: String,
    path: PathBuf,
    #[serde(rename = "type")]
    transport: String,
    allowed_origins: Vec<String>,
}

pub fn extension_dir(store: &Store) -> PathBuf {
    store.dir.join("browser-companion").join("extension")
}

pub fn companion_folder_text(store: &Store) -> Result<String, String> {
    let folder = extension_dir(store);
    if !folder.join("manifest.json").is_file() {
        return Err("The companion folder is not ready. Choose Connect browser first.".into());
    }
    let canonical = folder
        .canonicalize()
        .map_err(|e| format!("Cannot find the companion folder: {e}"))?;
    let path = canonical
        .to_str()
        .ok_or("The companion path cannot be copied as text.")?;
    // Canonical paths have a verbatim prefix that Chromium's folder picker does
    // not accept reliably. Return a normal drive/UNC path with native separators.
    #[cfg(windows)]
    let path = if let Some(share) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{share}")
    } else {
        path.strip_prefix(r"\\?\")
            .unwrap_or(path)
            .replace('/', r"\")
    };
    #[cfg(not(windows))]
    let path = path.to_owned();
    Ok(path)
}
pub fn manifest_path(store: &Store, browser: Browser) -> PathBuf {
    store
        .dir
        .join("native")
        .join(format!("{}.json", browser.label()))
}

pub fn prepare(
    store: &Store,
    browser: Browser,
    executable: &Path,
    registry: &impl Registration,
) -> Result<PathBuf, String> {
    if !executable.is_absolute()
        || !executable.is_file()
        || executable
            .extension()
            .is_none_or(|ext| !ext.to_string_lossy().eq_ignore_ascii_case("exe"))
    {
        return Err("The WebsiteBlocker executable is unavailable. Keep the app in a permanent folder and try again.".into());
    }
    let native = store.dir.join("native");
    std::fs::create_dir_all(&native)
        .map_err(|e| format!("Cannot prepare the browser connection: {e}"))?;
    let mut origins = vec![format!("chrome-extension://{}/", extension_id())];
    if let Some(previous) = registry.read(browser)? {
        let canonical = previous.canonicalize().map_err(|_| "An existing browser connection points to an unavailable file. It was preserved; reconnect using the previous WebsiteBlocker installation.")?;
        if !canonical.starts_with(native.canonicalize().map_err(|e| e.to_string())?) {
            return Err("Another WebsiteBlocker connection is registered outside this data folder. It was preserved; use that installation to reconnect.".into());
        }
        let old: NativeManifest = serde_json::from_slice(&read_limited(&canonical, 16384)?)
            .map_err(|_| "The existing browser connection is invalid and was preserved.")?;
        if old.name != HOST_NAME || old.transport != "stdio" {
            return Err(
                "The existing registration belongs to a different host and was preserved.".into(),
            );
        }
        for origin in old.allowed_origins {
            if crate::native::extension_origin(&origin)
                && !origins.contains(&origin)
                && origins.len() < 8
            {
                origins.push(origin);
            }
        }
    }
    let folder = extension_dir(store);
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    for (name, bytes) in BUNDLE_FILES {
        let path = folder.join(name);
        if read_limited(&path, 1_048_576).ok().as_deref() != Some(*bytes) {
            atomic_write(&path, bytes)?;
        }
    }
    let manifest = NativeManifest {
        name: HOST_NAME.into(),
        description: "WebsiteBlocker browser connection".into(),
        path: executable.into(),
        transport: "stdio".into(),
        allowed_origins: origins,
    };
    let path = manifest_path(store, browser);
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    registry.write(browser, &path)?;
    Ok(folder)
}

pub fn open_extensions(browser: Browser) -> Result<(), String> {
    let exe = browser.executable().ok_or_else(|| {
        format!(
            "{} was not found. Install it, then try again.",
            browser.label()
        )
    })?;
    Command::new(exe)
        .arg(browser.extensions_url())
        .spawn()
        .map_err(|e| format!("Cannot open {}: {e}", browser.label()))?;
    Ok(())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Applied {
    pub at: i64,
    pub revision: u64,
    pub extension_id: String,
    pub version: String,
    pub error: Option<String>,
}

pub fn record_applied(store: &Store, browser: Browser, applied: &Applied) -> Result<(), String> {
    if applied.extension_id != extension_id()
        || applied.version.len() > 32
        || applied.error.as_ref().is_some_and(|e| e.len() > 512)
    {
        return Err("Invalid browser connection report.".into());
    }
    if applied.error.is_none() && applied.revision != store.load()?.revision {
        return Err("Rules changed during sync. Check the connection again.".into());
    }
    let folder = store.dir.join("connections");
    std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
    atomic_write(
        &folder.join(format!("{}.json", browser.slug())),
        &serde_json::to_vec(applied).map_err(|e| e.to_string())?,
    )
}

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct BrowserStatus {
    pub available: bool,
    pub prepared: bool,
    pub connected: bool,
    pub up_to_date: bool,
    pub error: String,
}
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct Overview {
    pub chrome: BrowserStatus,
    pub edge: BrowserStatus,
}
impl Overview {
    pub fn get(&self, browser: Browser) -> &BrowserStatus {
        match browser {
            Browser::Chrome => &self.chrome,
            Browser::Edge => &self.edge,
        }
    }
    pub fn connected(&self) -> bool {
        self.chrome.connected || self.edge.connected
    }
}

pub fn inspect(store: &Store, registry: &impl Registration, now: i64) -> Result<Overview, String> {
    let revision = store.load()?.revision;
    let status = |browser: Browser| -> Result<BrowserStatus, String> {
        let prepared = registry
            .read(browser)?
            .is_some_and(|path| path == manifest_path(store, browser))
            && read_limited(&extension_dir(store).join("manifest.json"), 16384)
                .ok()
                .as_deref()
                == Some(BUNDLE_FILES[0].1);
        let report: Option<Applied> = read_limited(
            &store
                .dir
                .join("connections")
                .join(format!("{}.json", browser.slug())),
            4096,
        )
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
        let recent = report.as_ref().is_some_and(|r| {
            r.at <= now + 60
                && now.saturating_sub(r.at) < 100
                && r.extension_id == extension_id()
                && r.version == env!("CARGO_PKG_VERSION")
        });
        Ok(BrowserStatus {
            available: browser.executable().is_some(),
            prepared,
            connected: prepared && recent && report.as_ref().is_some_and(|r| r.error.is_none()),
            up_to_date: prepared
                && recent
                && report
                    .as_ref()
                    .is_some_and(|r| r.revision == revision && r.error.is_none()),
            error: if recent {
                report.and_then(|r| r.error).unwrap_or_default()
            } else {
                String::new()
            },
        })
    };
    Ok(Overview {
        chrome: status(Browser::Chrome)?,
        edge: status(Browser::Edge)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, collections::HashMap};
    #[derive(Default)]
    struct Registry {
        values: RefCell<HashMap<&'static str, PathBuf>>,
        fail: bool,
    }
    impl Registration for Registry {
        fn read(&self, browser: Browser) -> Result<Option<PathBuf>, String> {
            Ok(self.values.borrow().get(browser.slug()).cloned())
        }
        fn write(&self, browser: Browser, path: &Path) -> Result<(), String> {
            if self.fail {
                return Err("Fixture access denied".into());
            }
            self.values.borrow_mut().insert(browser.slug(), path.into());
            Ok(())
        }
    }
    fn fixture() -> (tempfile::TempDir, Store, PathBuf) {
        std::fs::create_dir_all(".test-output").unwrap();
        let temp = tempfile::tempdir_in(".test-output").unwrap();
        let root = temp.path().canonicalize().unwrap();
        let exe = root.join("WebsiteBlocker.exe");
        std::fs::write(&exe, b"fixture").unwrap();
        let store = Store {
            dir: root.join("data"),
        };
        (temp, store, exe)
    }
    #[test]
    fn embedded_identity_and_assets_match_the_companion() {
        use base64::Engine;
        use sha2::{Digest, Sha256};
        let manifest: serde_json::Value = serde_json::from_slice(BUNDLE_FILES[0].1).unwrap();
        let key = base64::engine::general_purpose::STANDARD
            .decode(manifest["key"].as_str().unwrap())
            .unwrap();
        let id: String = Sha256::digest(key)[..16]
            .iter()
            .flat_map(|v| [v >> 4, v & 15])
            .map(|v| (b'a' + v) as char)
            .collect();
        assert_eq!(id, extension_id());
        assert!(
            BUNDLE_FILES
                .iter()
                .all(|(name, _)| !name.contains(['/', '\\']))
        );
        assert!(BUNDLE_FILES.iter().any(|(name, _)| *name == "welcome.html"));
        for icons in [&manifest["icons"], &manifest["action"]["default_icon"]] {
            for (size, path) in icons.as_object().unwrap() {
                let bytes = BUNDLE_FILES
                    .iter()
                    .find(|(name, _)| Some(*name) == path.as_str())
                    .expect("Manifest icon is missing from the embedded companion")
                    .1;
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
                let dimension: u32 = size.parse().unwrap();
                assert_eq!(
                    u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
                    dimension
                );
                assert_eq!(
                    u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
                    dimension
                );
            }
        }
    }
    #[test]
    fn preparation_is_idempotent_per_browser_and_does_not_change_rules() {
        let (_temp, store, exe) = fixture();
        let registry = Registry::default();
        let before = serde_json::to_value(store.load().unwrap()).unwrap();
        let folder = prepare(&store, Browser::Chrome, &exe, &registry).unwrap();
        assert_eq!(folder, extension_dir(&store));
        for (name, bytes) in BUNDLE_FILES {
            assert_eq!(std::fs::read(folder.join(name)).unwrap(), *bytes);
        }
        std::fs::write(folder.join("manifest.json"), b"{\"version\":\"0.0.1\"}").unwrap();
        assert!(!inspect(&store, &registry, 1000).unwrap().chrome.prepared);
        prepare(&store, Browser::Chrome, &exe, &registry).unwrap();
        assert!(inspect(&store, &registry, 1000).unwrap().chrome.prepared);
        let mut manifest: NativeManifest =
            serde_json::from_slice(&std::fs::read(manifest_path(&store, Browser::Chrome)).unwrap())
                .unwrap();
        manifest
            .allowed_origins
            .push("chrome-extension://abcdefghijklmnopabcdefghijklmnop/".into());
        atomic_write(
            &manifest_path(&store, Browser::Chrome),
            &serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        prepare(&store, Browser::Chrome, &exe, &registry).unwrap();
        let result: NativeManifest =
            serde_json::from_slice(&std::fs::read(manifest_path(&store, Browser::Chrome)).unwrap())
                .unwrap();
        assert_eq!(result.allowed_origins.len(), 2);
        assert!(registry.read(Browser::Edge).unwrap().is_none());
        assert_eq!(serde_json::to_value(store.load().unwrap()).unwrap(), before);
        let status = inspect(&store, &registry, 1000).unwrap();
        assert!(status.chrome.prepared);
        assert!(!status.chrome.connected);
    }
    #[test]
    fn copied_folder_is_a_real_picker_path_with_spaces_and_unicode() {
        let (_temp, mut store, exe) = fixture();
        store.dir = store.dir.join("Profile with spaces Ω");
        assert!(companion_folder_text(&store).is_err());
        prepare(&store, Browser::Chrome, &exe, &Registry::default()).unwrap();
        let text = companion_folder_text(&store).unwrap();
        assert!(text.contains("Profile with spaces Ω"));
        assert!(Path::new(&text).join("manifest.json").is_file());
        #[cfg(windows)]
        {
            assert!(!text.starts_with(r"\\?\"));
            assert!(!text.contains('/'));
            assert!(text.ends_with(r"\browser-companion\extension"));
        }
        std::fs::remove_file(extension_dir(&store).join("manifest.json")).unwrap();
        assert!(companion_folder_text(&store).is_err());
    }
    #[test]
    fn failed_or_foreign_registration_is_preserved() {
        let (_temp, store, exe) = fixture();
        let registry = Registry::default();
        let foreign = exe.with_file_name("foreign.json");
        std::fs::write(&foreign, b"keep").unwrap();
        registry
            .values
            .borrow_mut()
            .insert("chrome", foreign.clone());
        assert!(prepare(&store, Browser::Chrome, &exe, &registry).is_err());
        assert_eq!(std::fs::read(&foreign).unwrap(), b"keep");
        assert_eq!(registry.read(Browser::Chrome).unwrap(), Some(foreign));
        let failure = Registry {
            fail: true,
            ..Registry::default()
        };
        assert!(prepare(&store, Browser::Edge, &exe, &failure).is_err());
        assert!(!inspect(&store, &failure, 1000).unwrap().edge.prepared);
    }
    #[test]
    fn only_applied_current_recent_rules_confirm_connection() {
        let (_temp, store, exe) = fixture();
        let registry = Registry::default();
        prepare(&store, Browser::Chrome, &exe, &registry).unwrap();
        let mut applied = Applied {
            at: 1000,
            revision: 0,
            extension_id: extension_id().into(),
            version: env!("CARGO_PKG_VERSION").into(),
            error: None,
        };
        record_applied(&store, Browser::Chrome, &applied).unwrap();
        assert!(inspect(&store, &registry, 1001).unwrap().chrome.connected);
        assert!(!inspect(&store, &registry, 1100).unwrap().chrome.connected);
        applied.revision = 9;
        assert!(record_applied(&store, Browser::Chrome, &applied).is_err());
        applied.revision = 0;
        applied.error = Some("Browser rejected the rule".into());
        record_applied(&store, Browser::Chrome, &applied).unwrap();
        let status = inspect(&store, &registry, 1001).unwrap();
        assert!(!status.chrome.connected);
        assert_eq!(status.chrome.error, "Browser rejected the rule");
        applied.extension_id = "x".repeat(32);
        assert!(record_applied(&store, Browser::Chrome, &applied).is_err());
    }
}

#[cfg(windows)]
mod platform {
    use std::path::{Path, PathBuf};
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_PATH_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::*,
    };
    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
    pub fn read(path: &str) -> Result<Option<PathBuf>, String> {
        let path = wide(path);
        let mut bytes = 0;
        // SAFETY: Read only the bounded string default of the exact per-user native host key.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut bytes,
            )
        };
        if result == ERROR_FILE_NOT_FOUND || result == ERROR_PATH_NOT_FOUND {
            return Ok(None);
        }
        if result != ERROR_SUCCESS || bytes > 65536 || bytes % 2 != 0 {
            return Err("Cannot read the per-user browser connection.".into());
        }
        let mut buffer = vec![0u16; bytes as usize / 2];
        // SAFETY: The allocated buffer matches the size returned above; only a REG_SZ is accepted.
        let result = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                std::ptr::null(),
                RRF_RT_REG_SZ,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        };
        if result != ERROR_SUCCESS {
            return Err("The browser connection changed while reading it. Try again.".into());
        }
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        Ok(Some(PathBuf::from(
            String::from_utf16(&buffer).map_err(|e| e.to_string())?,
        )))
    }
    pub fn write(path: &str, manifest: &Path) -> Result<(), String> {
        use std::os::windows::ffi::OsStrExt;
        let path = wide(path);
        let value: Vec<u16> = manifest.as_os_str().encode_wide().chain(Some(0)).collect();
        let mut key = std::ptr::null_mut();
        // SAFETY: Creates/opens only this app's native-host key in HKCU, with no elevation or policies.
        let result = unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                path.as_ptr(),
                0,
                std::ptr::null(),
                0,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        };
        if result != ERROR_SUCCESS {
            return Err(format!(
                "Windows did not allow this browser connection (error {result}). No administrator access is requested."
            ));
        }
        // SAFETY: The handle is valid and the value is a NUL-terminated UTF-16 string.
        let result = unsafe {
            RegSetValueExW(
                key,
                std::ptr::null(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        };
        unsafe {
            RegCloseKey(key);
        }
        if result != ERROR_SUCCESS {
            return Err(format!(
                "Cannot save the browser connection (Windows error {result})."
            ));
        }
        Ok(())
    }
}
#[cfg(not(windows))]
mod platform {
    use std::path::{Path, PathBuf};
    pub fn read(_: &str) -> Result<Option<PathBuf>, String> {
        Ok(None)
    }
    pub fn write(_: &str, _: &Path) -> Result<(), String> {
        Err("Browser setup currently supports Windows only.".into())
    }
}
