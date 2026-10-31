//! Lightweight GitHub Releases check for the About window.

use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use serde::Deserialize;

pub const GITHUB_REPO: &str = "unipsycho/chart-bench";
pub const GITHUB_URL: &str = "https://github.com/unipsycho/chart-bench";
/// Company products page (same site TopicBench Pro uses).
pub const PRODUCTS_URL: &str = "https://www.agileautomation.ca/products";

const USER_AGENT: &str = concat!("Chart-Bench/", env!("CARGO_PKG_VERSION"));
const API_TIMEOUT_SECS: u64 = 20;

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub tag: String,
    pub version: String,
    pub html_url: String,
}

#[derive(Debug, Clone)]
pub enum UpdateEvent {
    Ok {
        info: ReleaseInfo,
        update_available: bool,
    },
    Failed(String),
}

#[derive(Debug, Clone, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate {
        latest: String,
    },
    Available {
        latest: String,
        html_url: String,
    },
    Error(String),
}

impl UpdateStatus {
    pub fn line(&self) -> String {
        match self {
            Self::Idle => String::new(),
            Self::Checking => "Checking for updates…".into(),
            Self::UpToDate { latest } => format!("Up to date (latest {latest})."),
            Self::Available { latest, .. } => format!("Update available: {latest}"),
            Self::Error(msg) => format!("Couldn't check: {msg}"),
        }
    }
}

/// Background check handle — poll from the UI thread.
pub struct UpdateService {
    tx: Sender<UpdateEvent>,
    rx: Receiver<UpdateEvent>,
}

impl UpdateService {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel();
        Self { tx, rx }
    }

    pub fn try_recv(&self) -> Option<UpdateEvent> {
        self.rx.try_recv().ok()
    }

    pub fn start_check(&self) {
        let tx = self.tx.clone();
        let _ = thread::Builder::new()
            .name("chart-bench-update-check".into())
            .spawn(move || {
                let event = match fetch_latest() {
                    Ok(info) => {
                        let update_available = is_newer(&info.version, env!("CARGO_PKG_VERSION"));
                        UpdateEvent::Ok {
                            info,
                            update_available,
                        }
                    }
                    Err(e) => UpdateEvent::Failed(e),
                };
                let _ = tx.send(event);
            });
    }
}

impl Default for UpdateService {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    html_url: String,
}

fn fetch_latest() -> Result<ReleaseInfo, String> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");
    let agent = ureq::AgentBuilder::new()
        .timeout(std::time::Duration::from_secs(API_TIMEOUT_SECS))
        .build();
    let resp = agent
        .get(&url)
        .set("User-Agent", USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(friendly_http_err)?;
    let status = resp.status();
    if status == 404 {
        return Err(format!(
            "no releases at github.com/{GITHUB_REPO}"
        ));
    }
    if !(200..300).contains(&status) {
        return Err(format!("GitHub API HTTP {status}"));
    }
    let api: ApiRelease = resp
        .into_json()
        .map_err(|e| format!("parse release JSON: {e}"))?;
    let tag = api.tag_name;
    let version = tag.strip_prefix('v').unwrap_or(&tag).to_string();
    Ok(ReleaseInfo {
        tag: tag.clone(),
        version,
        html_url: api.html_url,
    })
}

fn friendly_http_err(err: ureq::Error) -> String {
    match err {
        ureq::Error::Status(code, _) => format!("GitHub API HTTP {code}"),
        ureq::Error::Transport(t) => format!("network: {t}"),
    }
}

/// Compare dotted numeric versions (ignores pre-release suffixes after `-`).
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.split('-')
            .next()
            .unwrap_or(s)
            .split('.')
            .filter_map(|p| p.parse().ok())
            .collect()
    };
    let a = parse(latest);
    let b = parse(current);
    a > b
}

/// Open a URL in the default browser.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open browser: {e}"))
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open browser: {e}"))
    }
    #[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("open browser: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::is_newer;

    #[test]
    fn semver_compare() {
        assert!(!is_newer("0.8.0", "0.8.0"));
        assert!(is_newer("0.9.0", "0.8.0"));
        assert!(!is_newer("0.7.9", "0.8.0"));
        assert!(is_newer("v1.0.0".trim_start_matches('v'), "0.8.0"));
    }
}
