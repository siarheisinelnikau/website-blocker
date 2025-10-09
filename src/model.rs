use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

pub const MAX_GROUPS: usize = 20;
pub const MAX_PATTERNS: usize = 50;
pub const HORIZON_SECONDS: i64 = 8 * 24 * 3600;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Pattern {
    pub domain: String,
    pub subdomains: bool,
    pub path_prefix: String,
}

impl Pattern {
    /// Plain domains include subdomains; '=' restricts to the exact host.
    pub fn parse(input: &str) -> Result<Self, String> {
        let input = input.trim();
        let (subdomains, input) = match input.strip_prefix('=') {
            Some(value) => (false, value),
            None => (true, input.strip_prefix("*.").unwrap_or(input)),
        };
        if input.len() > 512 || input.contains(['?', '#', '@', ':', '\\']) {
            return Err("Use a domain and optional /path prefix, without scheme, port, query or fragment.".into());
        }
        let url = Url::parse(&format!("https://{input}")).map_err(|e| e.to_string())?;
        let domain = url.host_str().ok_or("A domain is required.")?.to_ascii_lowercase();
        if domain.len() > 253 || !domain.contains('.') || domain.ends_with('.')
            || domain.split('.').any(|label| label.is_empty() || label.len() > 63
                || label.starts_with('-') || label.ends_with('-')
                || !label.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-'))
            || domain.parse::<std::net::IpAddr>().is_ok()
        {
            return Err("Use a public-style domain name, for example example.com.".into());
        }
        Ok(Self { domain, subdomains, path_prefix: url.path().into() })
    }

    pub fn text(&self) -> String {
        format!("{}{}{}", if self.subdomains { "" } else { "=" }, self.domain,
            if self.path_prefix == "/" { "" } else { &self.path_prefix })
    }

    pub fn matches(&self, url: &Url) -> bool {
        if !matches!(url.scheme(), "http" | "https") { return false; }
        let host = url.host_str().unwrap_or_default();
        (host == self.domain || (self.subdomains && host.ends_with(&format!(".{}", self.domain))))
            && url.path().starts_with(&self.path_prefix)
    }

    pub fn regex(&self) -> String {
        fn escape(value: &str) -> String {
            value.chars().flat_map(|c| {
                if ".+*?()[]{}^$|\\".contains(c) { vec!['\\', c] } else { vec![c] }
            }).collect()
        }
        format!("^https?://{}{}(:[0-9]+)?{}", if self.subdomains { "([a-z0-9-]+\\.)*" } else { "" }, escape(&self.domain), escape(&self.path_prefix))
    }
}

pub fn parse_patterns(text: &str) -> Result<Vec<Pattern>, String> {
    let patterns: Vec<_> = text.lines().filter(|line| !line.trim().is_empty())
        .map(Pattern::parse).collect::<Result<_, _>>()?;
    if patterns.len() > MAX_PATTERNS { return Err(format!("At most {MAX_PATTERNS} patterns are allowed per list.")); }
    Ok(patterns)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Schedule {
    /// Monday = 0, Sunday = 6. Overnight windows belong to their start day.
    pub days: Vec<u8>,
    pub start_minute: u16,
    pub end_minute: u16,
    pub timezone: String,
}

impl Schedule {
    pub fn active_wall(&self, day: u8, minute: u16) -> bool {
        if self.start_minute < self.end_minute {
            self.days.contains(&day) && minute >= self.start_minute && minute < self.end_minute
        } else {
            (self.days.contains(&day) && minute >= self.start_minute)
                || (self.days.contains(&((day + 6) % 7)) && minute < self.end_minute)
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.days.is_empty() || self.days.iter().any(|d| *d > 6)
            || self.start_minute > 1439 || self.end_minute > 1439 || self.start_minute == self.end_minute
            || self.timezone.is_empty() || self.timezone.len() > 80
            || !self.timezone.bytes().all(|c| c.is_ascii_alphanumeric() || b"/_+-".contains(&c))
        { return Err("Choose days, distinct start/end times, and a valid IANA timezone (e.g. Europe/Minsk).".into()); }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Group {
    pub id: u32,
    pub name: String,
    pub enabled: bool,
    pub patterns: Vec<Pattern>,
    pub schedules: Vec<Schedule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Focus {
    pub group_id: u32,
    pub end: i64,
    pub locked: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    pub revision: u64,
    pub groups: Vec<Group>,
    pub allowlist: Vec<Pattern>,
    pub focus: Option<Focus>,
    pub pause_until: i64,
    pub next_break: i64,
    pub startup_updates: bool,
}

impl Default for State {
    fn default() -> Self {
        Self { schema: 1, revision: 0, groups: vec![Group { id: 1, name: "Distractions".into(), enabled: true,
            patterns: parse_patterns("youtube.com\nreddit.com").unwrap(), schedules: vec![] }],
            allowlist: vec![], focus: None, pause_until: 0, next_break: 0, startup_updates: true }
    }
}

impl State {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != 1 { return Err("Unsupported settings schema. Keep this file for a compatible version.".into()); }
        if self.groups.len() > MAX_GROUPS || self.allowlist.len() > MAX_PATTERNS { return Err("Too many rules.".into()); }
        let mut ids = std::collections::HashSet::new();
        for group in &self.groups {
            if group.id == 0 || !ids.insert(group.id) || group.name.trim().is_empty() || group.name.len() > 80
                || group.patterns.is_empty() || group.patterns.len() > MAX_PATTERNS || group.schedules.len() > 14
            { return Err("Each rule set needs a unique ID, a name (1–80 bytes), 1–50 websites, and at most 14 schedules.".into()); }
            for schedule in &group.schedules { schedule.validate()?; }
        }
        for pattern in self.allowlist.iter().chain(self.groups.iter().flat_map(|g| &g.patterns)) {
            if Pattern::parse(&pattern.text())? != *pattern { return Err("Invalid saved website pattern.".into()); }
        }
        if let Some(focus) = &self.focus
            && !self.groups.iter().any(|g| g.id == focus.group_id && g.enabled)
        { return Err("The focus rule set must exist and be enabled.".into()); }
        Ok(())
    }

    pub fn locked(&self, now: i64) -> bool {
        self.focus.as_ref().is_some_and(|f| f.locked && f.end > now)
    }

    pub fn start_focus(&mut self, group_id: u32, minutes: u32, locked: bool, now: i64) -> Result<(), String> {
        if self.focus.as_ref().is_some_and(|f| f.end > now) { return Err("A focus session is already running.".into()); }
        if !(1..=240).contains(&minutes) || !self.groups.iter().any(|g| g.id == group_id && g.enabled) {
            return Err("Choose an enabled rule set and a duration from 1 to 240 minutes.".into());
        }
        self.focus = Some(Focus { group_id, end: now + i64::from(minutes) * 60, locked });
        self.pause_until = 0;
        Ok(())
    }

    pub fn stop_focus(&mut self, phrase: &str, now: i64) -> Result<(), String> {
        if self.locked(now) { return Err("This session is locked until its timer ends.".into()); }
        if phrase != "END FOCUS" { return Err("Type END FOCUS to end the session deliberately.".into()); }
        self.focus = None;
        Ok(())
    }

    pub fn take_break(&mut self, phrase: &str, now: i64) -> Result<(), String> {
        if self.locked(now) { return Err("Breaks are unavailable during a locked session.".into()); }
        if now < self.next_break { return Err("A break is available only once every 30 minutes.".into()); }
        if phrase != "TAKE A BREAK" { return Err("Type TAKE A BREAK to pause all rules for five minutes.".into()); }
        self.pause_until = now + 300;
        self.next_break = now + 1800;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolicyGroup {
    pub id: u32,
    pub name: String,
    pub regexes: Vec<String>,
    pub schedules: Vec<Schedule>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Policy {
    pub protocol: u32,
    pub revision: u64,
    pub generated_at: i64,
    pub expires_at: i64,
    pub allow_regexes: Vec<String>,
    pub groups: Vec<PolicyGroup>,
    pub focus: Option<Focus>,
    pub pause_until: i64,
}

impl State {
    /// Browser Intl evaluates IANA timezones using its maintained timezone database.
    /// Keep the policy local and cached so closing the desktop does not stop schedules.
    pub fn policy(&self, now: DateTime<Utc>) -> Policy {
        Policy { protocol: 1, revision: self.revision, generated_at: now.timestamp(),
            expires_at: now.timestamp() + HORIZON_SECONDS,
            allow_regexes: self.allowlist.iter().map(Pattern::regex).collect(),
            groups: self.groups.iter().filter(|g| g.enabled).map(|g| PolicyGroup {
                id: g.id, name: g.name.clone(), regexes: g.patterns.iter().map(Pattern::regex).collect(),
                schedules: g.schedules.clone(),
            }).collect(), focus: self.focus.clone(), pause_until: self.pause_until }
    }
}
