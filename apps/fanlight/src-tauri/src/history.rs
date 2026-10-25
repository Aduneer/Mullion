//! What you've opened from the panel, so it learns: things you open often and lately rank higher,
//! and what you picked for a query comes first the next time you type it (as Spotlight does).
//! It also keeps the searches you opened something from, newest first, for ↑ in the panel.
//! Kept in %APPDATA%\Fanlight\history.json; only the results' keys and the queries typed for them.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

/// How many items and queries are remembered; the least used go first.
const MAX_ITEMS: usize = 1000;
const MAX_QUERIES: usize = 500;
const MAX_RECENT: usize = 100;
/// A day, in seconds: how quickly an old pick counts for less.
const DAY: f64 = 86400.0;

#[derive(Serialize, Deserialize, Default, Clone, Debug, PartialEq)]
struct Use {
    count: u32,
    /// Unix seconds.
    last: u64,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
struct Data {
    items: HashMap<String, Use>,
    /// A query, lowercased, and the key picked for it last.
    queries: HashMap<String, (String, u64)>,
    /// Searches as typed, newest first.
    recent: Vec<String>,
}

pub struct History {
    path: PathBuf,
    data: Mutex<Data>,
}

fn now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

impl History {
    pub fn load(path: PathBuf) -> Self {
        let data = crate::settings::read_json(&path).unwrap_or_default();
        History { path, data: Mutex::new(data) }
    }

    /// `key` was opened after typing `query`.
    pub fn picked(&self, query: &str, key: &str) {
        let t = now();
        let mut d = self.data.lock().unwrap();
        let u = d.items.entry(key.to_string()).or_default();
        u.count = u.count.saturating_add(1);
        u.last = t;
        let q = query.trim().to_lowercase();
        if !q.is_empty() {
            d.queries.insert(q, (key.to_string(), t));
        }
        trim(&mut d);
        let _ = crate::settings::write_json(&self.path, &*d, false);
    }

    /// Something was opened from the search `query`: it goes to the top of the recent searches.
    pub fn searched(&self, query: &str) {
        let q = query.trim();
        if q.is_empty() {
            return;
        }
        let mut d = self.data.lock().unwrap();
        d.recent.retain(|r| r != q);
        d.recent.insert(0, q.to_string());
        d.recent.truncate(MAX_RECENT);
        let _ = crate::settings::write_json(&self.path, &*d, false);
    }

    /// The recent searches, newest first.
    pub fn recent(&self) -> Vec<String> {
        self.data.lock().unwrap().recent.clone()
    }

    /// Points to add to a result's score: up to about 300 for something used a lot and lately, and
    /// 2000 for the pick made last time for this very query.
    pub fn boost(&self, query: &str, key: &str) -> i32 {
        let d = self.data.lock().unwrap();
        let q = query.trim().to_lowercase();
        let learned = d.queries.get(&q).is_some_and(|(k, _)| k == key);
        let mut b = if learned { 2000 } else { 0 };
        if let Some(u) = d.items.get(key) {
            b += frecency(u, now());
        }
        b
    }

    /// Forget a result (the panel's "Forget" on a clipboard entry, or one that's gone).
    pub fn forget(&self, key: &str) {
        let mut d = self.data.lock().unwrap();
        d.items.remove(key);
        d.queries.retain(|_, (k, _)| k != key);
        let _ = crate::settings::write_json(&self.path, &*d, false);
    }

    pub fn clear(&self) {
        let mut d = self.data.lock().unwrap();
        *d = Data::default();
        let _ = std::fs::remove_file(&self.path);
    }
}

/// More uses count for more, with diminishing returns, and each day since the last halves it a
/// little: a week-old favourite still beats something opened once yesterday.
fn frecency(u: &Use, now: u64) -> i32 {
    let days = now.saturating_sub(u.last) as f64 / DAY;
    let uses = (1.0 + u.count as f64).ln() * 90.0;
    (uses / (1.0 + days / 7.0)).min(300.0) as i32
}

fn trim(d: &mut Data) {
    let t = now();
    if d.items.len() > MAX_ITEMS {
        let mut v: Vec<(String, i32)> = d.items.iter().map(|(k, u)| (k.clone(), frecency(u, t))).collect();
        v.sort_by_key(|(_, s)| *s);
        for (k, _) in v.into_iter().take(d.items.len() - MAX_ITEMS) {
            d.items.remove(&k);
        }
    }
    if d.queries.len() > MAX_QUERIES {
        let mut v: Vec<(String, u64)> = d.queries.iter().map(|(q, (_, at))| (q.clone(), *at)).collect();
        v.sort_by_key(|(_, at)| *at);
        for (q, _) in v.into_iter().take(d.queries.len() - MAX_QUERIES) {
            d.queries.remove(&q);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frequent_and_recent_rank_higher() {
        let t = 100 * DAY as u64;
        let often = frecency(&Use { count: 20, last: t - DAY as u64 }, t);
        let once = frecency(&Use { count: 1, last: t }, t);
        let old = frecency(&Use { count: 20, last: t - 60 * DAY as u64 }, t);
        assert!(often > once && often > old, "{often} {once} {old}");
        assert!(often <= 300);
    }

    #[test]
    fn learns_the_pick_for_a_query() {
        let path = std::env::temp_dir().join(format!("fanlight-history-test-{}.json", std::process::id()));
        let h = History::load(path.clone());
        h.picked("  Term ", "app:terminal");
        assert!(h.boost("term", "app:terminal") >= 2000);
        assert!(h.boost("term", "app:other") < 2000);
        assert!(h.boost("", "app:terminal") > 0);
        // It's saved, and read back.
        assert!(History::load(path.clone()).boost("term", "app:terminal") >= 2000);
        h.searched("term");
        h.searched("calc");
        h.searched(" term ");
        assert_eq!(h.recent(), ["term", "calc"]);
        h.forget("app:terminal");
        assert_eq!(h.boost("term", "app:terminal"), 0);
        h.clear();
        assert!(!path.exists() && h.recent().is_empty());
    }
}
