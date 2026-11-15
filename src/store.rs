use crate::{
    i18n::{Language, Theme},
    model::{Group, Pattern, State},
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(value) = std::env::var_os("WEBSITEBLOCKER_DATA_DIR") {
        let path = PathBuf::from(value);
        if !path.is_absolute() {
            return Err("WEBSITEBLOCKER_DATA_DIR must be absolute.".into());
        }
        return Ok(path);
    }
    #[cfg(windows)]
    let root = std::env::var_os("LOCALAPPDATA").ok_or("LOCALAPPDATA is unavailable.")?;
    #[cfg(not(windows))]
    let root = std::env::var_os("XDG_DATA_HOME")
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share").into_os_string())
        })
        .ok_or("Data directory unavailable.")?;
    Ok(PathBuf::from(root).join("WebsiteBlocker"))
}

#[derive(Clone)]
pub struct Store {
    pub dir: PathBuf,
}

#[derive(Clone)]
pub enum Change {
    Rules {
        revision: u64,
        groups: Vec<Group>,
        allowlist: Vec<Pattern>,
    },
    Start {
        group_id: u32,
        minutes: u32,
        locked: bool,
    },
    Stop(String),
    Break(String),
    StartupUpdates(bool),
    Appearance {
        language: Language,
        theme: Theme,
    },
    Resume,
}

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct Connection {
    pub at: i64,
    pub revision: u64,
}

impl Store {
    /// Compare-and-swap a browser edit. Conflicts return the untouched desktop copy.
    pub fn synchronize(
        &self,
        expected_revision: Option<u64>,
        incoming: Option<State>,
        now: i64,
    ) -> Result<(State, bool), String> {
        let _lock = self.lock()?;
        let current = self.read_unlocked()?;
        let Some(mut next) = incoming else {
            return Ok((current, false));
        };
        next.validate()?;
        if next.pause_until < 0
            || next.next_break < 0
            || next.focus.as_ref().is_some_and(|f| f.end < 0)
        {
            return Err("Invalid browser timer.".into());
        }
        if expected_revision != Some(current.revision) {
            return Ok((current, true));
        }
        if next.focus.as_ref().is_some_and(|f| f.end > now + 240 * 60)
            || next.pause_until > now + 300
            || next.next_break > now + 1800
        {
            return Err("Browser timer exceeds the allowed duration.".into());
        }
        if current.locked(now) {
            let protected = |s: &State| {
                serde_json::to_value((
                    &s.groups,
                    &s.allowlist,
                    &s.focus,
                    s.pause_until,
                    s.next_break,
                ))
                .map_err(|e| e.to_string())
            };
            if protected(&current)? != protected(&next)? {
                return Err("Rules are protected until the locked focus session ends.".into());
            }
        }
        next.startup_updates = current.startup_updates;
        next.revision = current
            .revision
            .checked_add(1)
            .ok_or("Settings revision overflow.")?;
        if next.focus.as_ref().is_some_and(|f| f.end <= now) {
            next.focus = None;
        }
        atomic_write(
            &self.dir.join("state.json"),
            &serde_json::to_vec_pretty(&next).map_err(|e| e.to_string())?,
        )?;
        Ok((next, false))
    }

    fn lock(&self) -> Result<File, String> {
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.dir.join("state.lock"))
            .map_err(|e| e.to_string())?;
        // Never hang the GUI worker indefinitely behind an abandoned or slow writer.
        file.try_lock_exclusive()
            .map_err(|e| format!("Settings are busy. Try again: {e}"))?;
        Ok(file)
    }

    fn read_unlocked(&self) -> Result<State, String> {
        let path = self.dir.join("state.json");
        if !path.exists() {
            return Ok(State::default());
        }
        let bytes = read_limited(&path, 1_048_576)?;
        let state: State = serde_json::from_slice(&bytes)
            .map_err(|e| format!("Settings cannot be read; original file was preserved: {e}"))?;
        state.validate()?;
        Ok(state)
    }

    pub fn load(&self) -> Result<State, String> {
        let _lock = self.lock()?;
        self.read_unlocked()
    }

    pub fn apply(&self, change: Change, now: i64) -> Result<State, String> {
        let _lock = self.lock()?;
        let mut state = self.read_unlocked()?;
        if state.focus.as_ref().is_some_and(|f| f.end <= now) {
            state.focus = None;
        }
        match change {
            Change::Rules {
                revision,
                groups,
                allowlist,
            } => {
                if state.locked(now) {
                    return Err("Rules are protected until the locked focus session ends.".into());
                }
                if revision != state.revision {
                    return Err("Settings changed in another window. Reload before saving.".into());
                }
                state.groups = groups;
                state.allowlist = allowlist;
            }
            Change::Start {
                group_id,
                minutes,
                locked,
            } => state.start_focus(group_id, minutes, locked, now)?,
            Change::Stop(phrase) => state.stop_focus(&phrase, now)?,
            Change::Break(phrase) => state.take_break(&phrase, now)?,
            Change::StartupUpdates(value) => state.startup_updates = value,
            Change::Appearance { language, theme } => {
                state.language = language;
                state.theme = theme;
            }
            Change::Resume => state.pause_until = 0,
        }
        state.validate()?;
        state.revision = state
            .revision
            .checked_add(1)
            .ok_or("Settings revision overflow.")?;
        atomic_write(
            &self.dir.join("state.json"),
            &serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
        )?;
        Ok(state)
    }

    pub fn connection(&self) -> Connection {
        read_limited(&self.dir.join("connection.json"), 4096)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn mark_connection(&self, connection: Connection) -> Result<(), String> {
        atomic_write(
            &self.dir.join("connection.json"),
            &serde_json::to_vec(&connection).map_err(|e| e.to_string())?,
        )
    }
}

pub fn read_limited(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("File exceeds the allowed size.".into());
    }
    Ok(bytes)
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut temp =
        tempfile::NamedTempFile::new_in(path.parent().ok_or("Missing parent directory.")?)
            .map_err(|e| e.to_string())?;
    temp.write_all(bytes)
        .and_then(|_| temp.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    temp.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
