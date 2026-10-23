use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

pub type Result<T> = std::result::Result<T, AppError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}
impl AppError {
    pub fn new(code: &str, message: &str) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}
impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}
impl std::error::Error for AppError {}
impl From<std::io::Error> for AppError {
    fn from(_: std::io::Error) -> Self {
        Self::new("filesystem", "文件无法访问，请检查路径和权限。")
    }
}
impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        Self::new(
            "database",
            "应用数据库操作失败，请检查数据目录权限与可用空间。",
        )
    }
}
pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Policy {
    pub name: Option<String>,
    pub email: Option<String>,
    pub domains: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub path: String,
    pub git_dir: String,
    pub common_dir: String,
    pub kind: String,
    pub parent: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub id: String,
    pub path: String,
    pub alias: String,
    pub group: Option<String>,
    pub kind: String,
    pub policy: Policy,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Module<T> {
    pub state: String,
    pub data: Option<T>,
    pub error: Option<AppError>,
    pub read_at: u64,
}
impl<T> Module<T> {
    pub fn success(data: T) -> Self {
        Self {
            state: "success".into(),
            data: Some(data),
            error: None,
            read_at: now(),
        }
    }
    pub fn failed(e: AppError) -> Self {
        Self {
            state: "failed".into(),
            data: None,
            error: Some(e),
            read_at: now(),
        }
    }
    pub fn empty(state: &str) -> Self {
        Self {
            state: state.into(),
            data: None,
            error: None,
            read_at: now(),
        }
    }
    pub fn from_result(r: Result<T>) -> Self {
        match r {
            Ok(v) => Self::success(v),
            Err(e) => Self::failed(e),
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Head {
    pub branch: Option<String>,
    pub oid: Option<String>,
    pub detached: bool,
    pub unborn: bool,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkStatus {
    pub staged: usize,
    pub unstaged: usize,
    pub untracked: usize,
    pub conflicts: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub oid: String,
    pub date: String,
    pub subject: String,
    pub author: String,
    pub email: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Upstream {
    pub state: String,
    pub name: Option<String>,
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConfigValue {
    pub value: String,
    pub scope: String,
    pub origin: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigKey {
    pub key: String,
    pub values: Vec<ConfigValue>,
}
impl ConfigKey {
    pub fn effective(&self) -> Option<&str> {
        self.values.last().map(|v| v.value.as_str())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub field: String,
    pub value: Option<String>,
    pub state: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub keys: Vec<ConfigKey>,
    pub findings: Vec<Finding>,
    pub status: String,
    pub checked: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Endpoint {
    pub remote: String,
    pub usage: String,
    pub address: String,
    pub configured_address: String,
    pub protocol: String,
    pub host: String,
    pub platform: Option<String>,
    pub username: Option<String>,
    pub metadata: Vec<String>,
    pub limitations: Vec<String>,
    pub config_state: String,
    pub verification: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub entry_id: String,
    pub git: String,
    pub read_at: u64,
    pub context: Module<Context>,
    pub head: Module<Head>,
    pub work: Module<WorkStatus>,
    pub history: Module<Commit>,
    pub upstream: Module<Upstream>,
    pub identity: Module<Identity>,
    pub remotes: Module<Vec<Endpoint>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub git: String,
    pub git_version: Option<String>,
    pub git_error: Option<AppError>,
    pub data_dir: String,
    pub version: String,
}

pub fn valid_text(s: &str, max: usize) -> bool {
    !s.trim().is_empty() && s.len() <= max && !s.chars().any(char::is_control)
}
