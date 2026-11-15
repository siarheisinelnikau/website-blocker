//! Bounded per-profile counters. Requested URLs and page contents are never accepted.
use crate::store::{Store, atomic_write, read_limited};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Activity {
    pub total: u64,
    pub sites: BTreeMap<String, u64>,
    pub recent: Vec<Attempt>,
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub site: String,
    pub at: i64,
}
impl Activity {
    fn validate(&self) -> Result<(), String> {
        let valid_site = |s: &str| crate::model::Pattern::parse(s).is_ok_and(|p| p.domain == s);
        if self.total > 9_007_199_254_740_991
            || self.sites.len() > 100
            || self.recent.len() > 30
            || self
                .sites
                .iter()
                .any(|(s, n)| !valid_site(s) || *n > self.total)
            || self.recent.iter().any(|a| !valid_site(&a.site) || a.at < 0)
        {
            return Err("Invalid browser activity counters.".into());
        }
        Ok(())
    }
}
pub fn record(store: &Store, client: &str, activity: &Activity) -> Result<(), String> {
    if client.len() != 36 || !client.bytes().all(|c| c.is_ascii_hexdigit() || c == b'-') {
        return Err("Invalid browser profile identifier.".into());
    }
    activity.validate()?;
    let dir = store.dir.join("activity");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("{client}.json"));
    if !path.exists()
        && std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .take(33)
            .count()
            >= 32
    {
        return Err("Too many browser activity profiles.".into());
    }
    let bytes = serde_json::to_vec(activity).map_err(|e| e.to_string())?;
    if read_limited(&path, 32768).ok().as_ref() != Some(&bytes) {
        atomic_write(&path, &bytes)?;
    }
    Ok(())
}
pub fn summary(store: &Store) -> Activity {
    let mut result = Activity::default();
    if let Ok(entries) = std::fs::read_dir(store.dir.join("activity")) {
        for entry in entries.flatten().take(32) {
            let Ok(bytes) = read_limited(&entry.path(), 32768) else {
                continue;
            };
            let Ok(activity) = serde_json::from_slice::<Activity>(&bytes) else {
                continue;
            };
            if activity.validate().is_err() {
                continue;
            }
            result.total = result.total.saturating_add(activity.total);
            for (site, count) in activity.sites {
                let value = result.sites.entry(site).or_default();
                *value = value.saturating_add(count);
            }
            result.recent.extend(activity.recent);
        }
    }
    result.recent.sort_by_key(|a| std::cmp::Reverse(a.at));
    result.recent.truncate(30);
    result
}
