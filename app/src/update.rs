//! Looks for a newer FreeSheet on GitHub, on a background thread, so the window never waits
//! for the network and being offline is simply no answer. Nothing about the learner is
//! sent: it is one plain request for the public list of releases.

use std::sync::{Arc, Mutex};

const LATEST: &str = "https://api.github.com/repos/Varmite/freesheet/releases/latest";
/// Release pages are only ever opened from here.
const RELEASES: &str = "https://github.com/Varmite/freesheet/releases/";
pub const CURRENT: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone, Debug, PartialEq, Default)]
pub enum State {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available {
        version: String,
        url: String,
    },
    Failed,
}

#[derive(Default, Clone)]
pub struct Checker {
    state: Arc<Mutex<State>>,
}

fn parts(v: &str) -> Option<(u64, u64, u64)> {
    let v = v.trim().trim_start_matches(['v', 'V']);
    let core = v.split(['-', '+']).next()?;
    let mut it = core.split('.').map(|n| n.parse::<u64>().ok());
    Some((it.next()??, it.next()??, it.next()??))
}

/// Is `other` a later version than `current`?
pub fn is_newer(other: &str, current: &str) -> bool {
    matches!((parts(other), parts(current)), (Some(a), Some(b)) if a > b)
}

/// What a reply from GitHub means for this version.
pub fn interpret(body: &str, current: &str) -> State {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else { return State::Failed };
    let Some(tag) = v["tag_name"].as_str() else { return State::Failed };
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) || !is_newer(tag, current) {
        return State::UpToDate;
    }
    let url = v["html_url"].as_str().filter(|u| u.starts_with(RELEASES)).unwrap_or("https://github.com/Varmite/freesheet/releases/latest");
    State::Available { version: tag.trim_start_matches(['v', 'V']).to_string(), url: url.to_string() }
}

#[cfg(not(target_os = "android"))]
fn fetch() -> Result<String, String> {
    use std::time::Duration;
    let agent: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(15))).build().into();
    let mut resp = agent
        .get(LATEST)
        .header("User-Agent", format!("FreeSheet/{CURRENT}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?;
    resp.body_mut().read_to_string().map_err(|e| e.to_string())
}

#[cfg(target_os = "android")]
fn fetch() -> Result<String, String> {
    let _ = LATEST;
    Err("not on phones yet".into())
}

impl Checker {
    pub fn state(&self) -> State {
        self.state.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Starts a check unless one is already running.
    pub fn start(&self, ctx: &egui::Context) {
        {
            let Ok(mut s) = self.state.lock() else { return };
            if *s == State::Checking {
                return;
            }
            *s = State::Checking;
        }
        let state = self.state.clone();
        let ctx = ctx.clone();
        let spawned = std::thread::Builder::new().name("update-check".into()).spawn(move || {
            let result = match fetch() {
                Ok(body) => interpret(&body, CURRENT),
                Err(e) => {
                    log::info!("update check: {e}");
                    State::Failed
                }
            };
            if let Ok(mut s) = state.lock() {
                *s = result;
            }
            ctx.request_repaint();
        });
        if spawned.is_err() {
            if let Ok(mut s) = self.state.lock() {
                *s = State::Failed;
            }
        }
    }

    /// Testing: act as if this version had been found.
    pub fn pretend(&self, version: &str, url: &str) {
        if let Ok(mut s) = self.state.lock() {
            *s = State::Available { version: version.into(), url: url.into() };
        }
    }
}

/// Opens a release page in the browser.
pub fn open_page(url: &str) {
    if !url.starts_with(RELEASES) {
        return;
    }
    #[cfg(not(target_os = "android"))]
    if let Err(e) = open::that(url) {
        log::warn!("could not open {url}: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_as_numbers() {
        assert!(is_newer("v1.10.0", "1.9.9"));
        assert!(is_newer("2.0.0", "1.99.99"));
        assert!(!is_newer("v1.2.0", "1.2.0"));
        assert!(!is_newer("1.1.0", "1.2.0"));
        assert!(!is_newer("android-v9.0.0", "1.2.0"));
        assert!(!is_newer("garbage", "1.2.0"));
    }

    #[test]
    fn reads_githubs_answer() {
        let body = r#"{"tag_name":"v1.3.0","html_url":"https://github.com/Varmite/freesheet/releases/tag/v1.3.0","draft":false,"prerelease":false}"#;
        assert_eq!(interpret(body, "1.2.0"), State::Available { version: "1.3.0".into(), url: "https://github.com/Varmite/freesheet/releases/tag/v1.3.0".into() });
        assert_eq!(interpret(body, "1.3.0"), State::UpToDate);
        assert_eq!(interpret("{nope", "1.2.0"), State::Failed);
        assert_eq!(interpret(r#"{"message":"rate limited"}"#, "1.2.0"), State::Failed);
    }

    #[test]
    fn a_link_elsewhere_is_replaced() {
        let body = r#"{"tag_name":"v1.3.0","html_url":"https://example.com/evil"}"#;
        let State::Available { url, .. } = interpret(body, "1.2.0") else { panic!() };
        assert!(url.starts_with(RELEASES));
    }
}
