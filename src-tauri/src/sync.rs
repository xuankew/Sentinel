//! GitHub 同步传输层：细粒度 PAT 存 macOS Keychain，仓库配置存明文 JSON，
//! 保险库密文经 GitHub Contents API 拉取/推送。云端只经手密文（PRD §2.4）。
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};

const KEYCHAIN_SERVICE: &str = "com.sentinel.app";
const KEYCHAIN_USER: &str = "github-sync-token";
const API: &str = "https://api.github.com";

/// 仓库同步配置（非敏感，明文存 app 数据目录）。Token 不在此，走 Keychain。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct SyncConfig {
    pub owner: String,
    pub repo: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    #[serde(default = "default_path")]
    pub path: String,
    /// 自动同步间隔（分钟）；0 = 关闭自动同步。
    #[serde(default)]
    pub auto_sync_minutes: u32,
    /// 上次同步成功的 unix 秒；从未同步为 None。
    #[serde(default)]
    pub last_sync_at: Option<i64>,
}

fn default_branch() -> String {
    "main".into()
}
fn default_path() -> String {
    "vault.sentinel".into()
}

impl SyncConfig {
    pub fn is_complete(&self) -> bool {
        !self.owner.trim().is_empty() && !self.repo.trim().is_empty()
    }
}

// ---------- Keychain token ----------

fn entry() -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYCHAIN_SERVICE, KEYCHAIN_USER).map_err(|e| e.to_string())
}

pub fn set_token(token: &str) -> Result<(), String> {
    entry()?.set_password(token).map_err(|e| e.to_string())
}

pub fn get_token() -> Result<Option<String>, String> {
    match entry()?.get_password() {
        Ok(t) => Ok(Some(t)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn delete_token() -> Result<(), String> {
    match entry()?.delete_credential() {
        Ok(()) => Ok(()),
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

// ---------- GitHub Contents API ----------

/// 远端文件：密文字节 + 用于 PUT 更新的 sha（None 表示远端尚不存在）。
pub struct RemoteFile {
    pub bytes: Vec<u8>,
    pub sha: Option<String>,
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("sentinel-sync")
        .build()
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct GetResp {
    content: Option<String>,
    sha: Option<String>,
    encoding: Option<String>,
    message: Option<String>,
}

/// 拉取远端密文文件。远端不存在（404）→ Ok(None)。
pub async fn pull(cfg: &SyncConfig, token: &str) -> Result<Option<RemoteFile>, String> {
    let url = format!(
        "{}/repos/{}/{}/contents/{}?ref={}",
        API,
        cfg.owner,
        cfg.repo,
        cfg.path.trim_start_matches('/'),
        cfg.branch
    );
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("网络请求失败：{e}"))?;

    let status = resp.status();
    if status.as_u16() == 404 {
        return Ok(None);
    }
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(github_error(status.as_u16(), &body));
    }

    let parsed: GetResp = resp.json().await.map_err(|e| e.to_string())?;
    let content = match parsed.content {
        Some(c) => c,
        None => return Err(parsed.message.unwrap_or_else(|| "远端响应缺少 content".into())),
    };
    if parsed.encoding.as_deref() != Some("base64") {
        return Err("远端内容编码不是 base64".into());
    }
    // GitHub 返回的 base64 可能带换行，先剔除空白再解码。
    let cleaned: String = content.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = B64.decode(cleaned.as_bytes()).map_err(|e| format!("base64 解码失败：{e}"))?;
    Ok(Some(RemoteFile { bytes, sha: parsed.sha }))
}

#[derive(Deserialize)]
struct PutResp {
    message: Option<String>,
    content: Option<PutContent>,
}
#[derive(Deserialize)]
struct PutContent {
    sha: Option<String>,
}

/// 推送密文文件。`sha` 为远端当前版本（更新时必填，创建时为 None）。
/// 返回新的 blob sha。
pub async fn push(cfg: &SyncConfig, token: &str, bytes: &[u8], sha: Option<String>) -> Result<String, String> {
    let url = format!(
        "{}/repos/{}/{}/contents/{}",
        API,
        cfg.owner,
        cfg.repo,
        cfg.path.trim_start_matches('/')
    );
    let mut body = serde_json::json!({
        "message": "Sentinel sync",
        "content": B64.encode(bytes),
        "branch": cfg.branch,
    });
    if let Some(sha) = sha {
        body["sha"] = serde_json::Value::String(sha);
    }
    let resp = client()?
        .put(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("网络请求失败：{e}"))?;

    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if !status.is_success() {
        return Err(github_error(status.as_u16(), &text));
    }
    let parsed: PutResp = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    parsed
        .content
        .and_then(|c| c.sha)
        .ok_or_else(|| parsed.message.unwrap_or_else(|| "推送响应缺少 sha".into()))
}

/// 校验 token + 仓库可访问（用于「测试连接」）。返回仓库全名。
pub async fn verify(cfg: &SyncConfig, token: &str) -> Result<String, String> {
    let url = format!("{}/repos/{}/{}", API, cfg.owner, cfg.repo);
    let resp = client()?
        .get(&url)
        .bearer_auth(token)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| format!("网络请求失败：{e}"))?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(github_error(status.as_u16(), &body));
    }
    #[derive(Deserialize)]
    struct Repo {
        full_name: String,
    }
    let repo: Repo = resp.json().await.map_err(|e| e.to_string())?;
    Ok(repo.full_name)
}

fn github_error(status: u16, body: &str) -> String {
    // 尽量抽取 GitHub 的 message 字段，附带状态码。
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(msg) = v.get("message").and_then(|m| m.as_str()) {
            return match status {
                401 => format!("Token 无效或已过期（401）：{msg}"),
                403 => format!("权限不足（403）：{msg}"),
                404 => format!("仓库或路径不存在（404）：{msg}"),
                409 => format!("远端已变化，存在冲突（409）：{msg}"),
                _ => format!("GitHub 错误（{status}）：{msg}"),
            };
        }
    }
    match status {
        401 => "Token 无效或已过期（401）".into(),
        403 => "权限不足（403），请确认 Token 有该仓库 contents 读写权限".into(),
        404 => "仓库或路径不存在（404）".into(),
        409 => "远端已变化，存在冲突（409），请重新同步".into(),
        _ => format!("GitHub 错误（{status}）"),
    }
}
