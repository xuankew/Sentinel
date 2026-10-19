// 暴露给前端的 Tauri 命令：保险库生命周期 + 条目 CRUD + 剪贴板 + TOTP + GitHub 同步。
// 重活（Argon2id 派生 / 加解密）放到 spawn_blocking，避免阻塞 UI。
use crate::crypto::{KdfParams, KEY_LEN, SALT_LEN};
use crate::merge::{self, MergeConflict, Side};
use crate::settings::Settings;
use crate::state::{AppState, PendingSync, Session};
use crate::sync::{self, SyncConfig};
use crate::vault::{self, Item, ItemDraft, VaultData};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use zeroize::Zeroizing;

const VAULT_FILE: &str = "vault.sentinel";
const SYNC_CONFIG_FILE: &str = "sync.json";
const BASE_FILE: &str = "base.sentinel";
const SETTINGS_FILE: &str = "settings.json";
const DEFAULT_CLIPBOARD_CLEAR_SECS: u64 = 30;

fn vault_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(VAULT_FILE))
}

fn sync_config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(SYNC_CONFIG_FILE))
}

fn base_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(BASE_FILE))
}

fn settings_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join(SETTINGS_FILE))
}

/// 把当前会话的明文保险库重新加密并落盘（每次改动后调用）。
fn persist(app: &AppHandle, sess: &Session) -> Result<(), String> {
    let bytes = vault::seal(&sess.data, &*sess.key, &sess.params, &sess.salt)?;
    std::fs::write(vault_path(app)?, bytes).map_err(|e| e.to_string())
}

// ---------- 保险库生命周期 ----------

/// 保险库文件是否已存在（决定前端进创建流程还是解锁流程）。
#[tauri::command]
pub async fn vault_exists(app: AppHandle) -> Result<bool, String> {
    Ok(vault_path(&app)?.exists())
}

/// 创建新保险库：派生密钥、加密空库、落盘，并置为已解锁。返回条目数。
#[tauri::command]
pub async fn create_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> Result<usize, String> {
    let path = vault_path(&app)?;
    if path.exists() {
        return Err("保险库已存在，无法重复创建".into());
    }
    let pw_for_session = password.clone();
    let (opened, bytes) = tauri::async_runtime::spawn_blocking(move || vault::create(&password))
        .await
        .map_err(|e| e.to_string())??;
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let count = opened.data.items.len();
    *state.session.lock().unwrap() = Some(Session {
        key: opened.key,
        password: Zeroizing::new(pw_for_session),
        data: opened.data,
        params: opened.params,
        salt: opened.salt,
    });
    Ok(count)
}

/// 用主密码解锁已存在的保险库。返回条目数。
#[tauri::command]
pub async fn unlock(
    app: AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> Result<usize, String> {
    let path = vault_path(&app)?;
    let bytes = std::fs::read(&path).map_err(|_| "保险库不存在".to_string())?;
    let pw_for_session = password.clone();
    let opened = tauri::async_runtime::spawn_blocking(move || vault::open(&bytes, &password))
        .await
        .map_err(|e| e.to_string())??;
    let count = opened.data.items.len();
    *state.session.lock().unwrap() = Some(Session {
        key: opened.key,
        password: Zeroizing::new(pw_for_session),
        data: opened.data,
        params: opened.params,
        salt: opened.salt,
    });
    Ok(count)
}

/// 锁定：清空内存中的密钥与明文（Zeroizing 自动清零）。
#[tauri::command]
pub fn lock(state: State<'_, AppState>) -> Result<(), String> {
    *state.session.lock().unwrap() = None;
    Ok(())
}

/// 当前是否处于已解锁状态。
#[tauri::command]
pub fn is_unlocked(state: State<'_, AppState>) -> bool {
    state.session.lock().unwrap().is_some()
}

/// 修改主密码：校验旧密码 → 用新密码（全新 salt）重新加密整库。
///
/// 方案 A（与云端同步联动）：若已配置同步，先拉取远端并要求**内容与本机一致**
/// （不一致则拒绝，提示先同步），随后把新密码密文推送云端，使远端一并轮换。
/// 本地文件仅在（如需）推送成功后才写入——推送失败则整体不改动，可安全重试，
/// 避免出现「本地新密码 / 云端旧密码」导致后续同步解不开的死锁。
/// 最后同步更新内存会话，并在 Touch ID 已启用时用新密码重写钥匙串项。
#[tauri::command]
pub async fn change_master_password(
    app: AppHandle,
    state: State<'_, AppState>,
    old_password: String,
    new_password: String,
) -> Result<(), String> {
    if new_password.chars().count() < 8 {
        return Err("新主密码至少 8 位".into());
    }
    if new_password == old_password {
        return Err("新主密码不能与原密码相同".into());
    }

    // 校验旧密码，取出重封所需的会话材料
    let (old_key, ours) = {
        let guard = state.session.lock().unwrap();
        let sess = guard.as_ref().ok_or("保险库未解锁")?;
        if sess.password.as_str() != old_password {
            return Err("原主密码不正确".into());
        }
        (sess.key.clone(), sess.data.items.clone())
    };

    // 用新密码重新派生 + 封盘（Argon2id 重活放阻塞线程池）
    let data = VaultData { version: 1, items: ours.clone() };
    let np = new_password.clone();
    let (opened, new_bytes) = tauri::async_runtime::spawn_blocking(move || {
        vault::reseal_with_password(&data, &np)
    })
    .await
    .map_err(|e| e.to_string())??;

    // 已配置同步 → 对齐检查 + 推送（在写本地文件之前完成，失败则本地不动）
    let cfg = load_sync_config(&app)?;
    let token = sync::get_token_cached()?;
    let remote_synced = cfg.is_complete() && token.is_some();
    if let Some(token) = token.filter(|_| cfg.is_complete()) {
        match sync::pull(&cfg, &token).await? {
            Some(rf) => {
                let bytes = rf.bytes.clone();
                let op = old_password.clone();
                let npc = new_password.clone();
                // 远端可能是旧密码（本机首个轮换）或新密码（他机已轮换），两者都试
                let theirs = tauri::async_runtime::spawn_blocking(move || {
                    vault::open(&bytes, &op)
                        .or_else(|_| vault::open(&bytes, &npc))
                        .map(|o| o.data.items)
                })
                .await
                .map_err(|e| e.to_string())?
                .map_err(|_| "无法解密云端保险库（新旧主密码都不匹配）".to_string())?;
                if !merge::same_content(&theirs, &ours) {
                    return Err("本机与云端数据不一致，请先完成同步后再修改主密码".into());
                }
                sync::push(&cfg, &token, &new_bytes, rf.sha.clone()).await?;
            }
            None => {
                // 云端尚无文件：直接推送新密码库
                sync::push(&cfg, &token, &new_bytes, None).await?;
            }
        }
    }

    // 写本地保险库
    std::fs::write(vault_path(&app)?, &new_bytes).map_err(|e| e.to_string())?;

    // 重封同步基线快照：用旧密钥读出内容，再用新密钥写回（保持基线语义）
    if let Ok(bp) = base_path(&app) {
        if bp.exists() {
            if let Ok(base_bytes) = std::fs::read(&bp) {
                if let Ok(base_data) = vault::open_with_key(&base_bytes, &old_key) {
                    if let Ok(resealed) =
                        vault::seal(&base_data, &opened.key, &opened.params, &opened.salt)
                    {
                        let _ = std::fs::write(&bp, resealed);
                    }
                }
            }
        } else if remote_synced {
            let _ = std::fs::write(&bp, &new_bytes);
        }
    }

    // 更新内存会话为新密钥材料（条目未变，data 保留）
    {
        let mut guard = state.session.lock().unwrap();
        if let Some(sess) = guard.as_mut() {
            sess.params = opened.params;
            sess.salt = opened.salt;
            sess.key = opened.key;
            sess.password = Zeroizing::new(new_password.clone());
        }
    }

    // Touch ID 已启用 → 用新密码重写受保护钥匙串项（尽力而为，失败不回滚已完成的轮换）
    if crate::touchid::enabled() {
        let _ = crate::touchid::store(&new_password);
    }

    Ok(())
}

// ---------- 条目 CRUD ----------

#[tauri::command]
pub fn list_items(state: State<'_, AppState>) -> Result<Vec<Item>, String> {
    let guard = state.session.lock().unwrap();
    let sess = guard.as_ref().ok_or("保险库未解锁")?;
    Ok(sess.data.items.clone())
}

#[tauri::command]
pub fn create_item(
    app: AppHandle,
    state: State<'_, AppState>,
    draft: ItemDraft,
) -> Result<Item, String> {
    let mut guard = state.session.lock().unwrap();
    let sess = guard.as_mut().ok_or("保险库未解锁")?;
    let item = sess.data.add(draft);
    persist(&app, sess)?;
    Ok(item)
}

#[tauri::command]
pub fn update_item(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    draft: ItemDraft,
) -> Result<Item, String> {
    let mut guard = state.session.lock().unwrap();
    let sess = guard.as_mut().ok_or("保险库未解锁")?;
    let item = sess.data.update(&id, draft)?;
    persist(&app, sess)?;
    Ok(item)
}

#[tauri::command]
pub fn delete_item(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<(), String> {
    let mut guard = state.session.lock().unwrap();
    let sess = guard.as_mut().ok_or("保险库未解锁")?;
    sess.data.delete(&id)?;
    persist(&app, sess)?;
    Ok(())
}

// ---------- 剪贴板（敏感字段复制后自动清除） ----------

/// 复制敏感值到剪贴板；`clear_after` 秒后若剪贴板仍是该值则自动清除。
#[tauri::command]
pub async fn copy_secret(
    app: AppHandle,
    value: String,
    clear_after: Option<u64>,
) -> Result<(), String> {
    app.clipboard().write_text(&value).map_err(|e| e.to_string())?;
    let secs = clear_after.unwrap_or(DEFAULT_CLIPBOARD_CLEAR_SECS);
    if secs > 0 {
        let app2 = app.clone();
        let expected = value.clone();
        // 用阻塞线程池 sleep，避免引入额外异步定时器依赖。
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_secs(secs));
            // 仅当剪贴板内容仍是我们写入的值时才清除，避免误删用户后来复制的内容。
            if let Ok(cur) = app2.clipboard().read_text() {
                if cur == expected {
                    let _ = app2.clipboard().clear();
                }
            }
        });
    }
    Ok(())
}

// ---------- TOTP ----------

#[derive(Serialize)]
pub struct TotpResult {
    pub code: String,
    pub remaining_secs: u64,
    pub period: u64,
}

/// 由 otpauth:// 链接或裸 base32 密钥生成当前验证码。
#[tauri::command]
pub fn generate_totp(secret: String) -> Result<TotpResult, String> {
    let r = crate::totp::generate(&secret)?;
    Ok(TotpResult {
        code: r.code,
        remaining_secs: r.remaining_secs,
        period: r.period,
    })
}

// ---------- GitHub 同步 ----------

/// 返回给前端的同步配置视图（绝不含 token 本身，只报告是否已设置）。
#[derive(Serialize)]
pub struct SyncConfigView {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub path: String,
    pub has_token: bool,
    pub auto_sync_minutes: u32,
    pub last_sync_at: Option<i64>,
}

/// 前端提交的同步配置。token 语义：None=不改动；Some(空)=清除；Some(值)=写入 Keychain。
#[derive(Deserialize)]
pub struct SyncConfigInput {
    #[serde(default)]
    pub owner: String,
    #[serde(default)]
    pub repo: String,
    #[serde(default)]
    pub branch: String,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub token: Option<String>,
    #[serde(default)]
    pub auto_sync_minutes: u32,
}

/// 一次同步的结果。
#[derive(Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SyncResult {
    /// 远端与本地一致，无需推送。
    UpToDate,
    /// 合并成功并已推送。
    Synced { total: usize },
    /// 存在需用户裁决的冲突，已暂存待 resolve_sync。
    Conflict { conflicts: Vec<MergeConflict> },
}

fn load_sync_config(app: &AppHandle) -> Result<SyncConfig, String> {
    let p = sync_config_path(app)?;
    if !p.exists() {
        return Ok(SyncConfig::default());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    serde_json::from_str(&s).map_err(|e| e.to_string())
}

fn write_sync_config(app: &AppHandle, cfg: &SyncConfig) -> Result<(), String> {
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(sync_config_path(app)?, json).map_err(|e| e.to_string())
}

fn load_base(app: &AppHandle, key: &[u8; KEY_LEN]) -> Vec<Item> {
    // 基线快照解密失败（如尚无快照）时按空处理，首次同步即把所有条目视作各自新增。
    let Ok(path) = base_path(app) else { return Vec::new() };
    match std::fs::read(path) {
        Ok(bytes) => vault::open_with_key(&bytes, key).map(|d| d.items).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

/// 记录本次同步成功的时间戳（侧栏「上次同步」展示用）。
fn record_sync_success(app: &AppHandle) -> Result<(), String> {
    let mut cfg = load_sync_config(app)?;
    cfg.last_sync_at = Some(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0),
    );
    let json = serde_json::to_string_pretty(&cfg).map_err(|e| e.to_string())?;
    std::fs::write(sync_config_path(app)?, json).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_sync_config(app: AppHandle) -> Result<SyncConfigView, String> {
    let mut cfg = load_sync_config(&app)?;
    // 旧配置无 has_token 标记时回填一次；之后只读标记、不读钥匙串，避免 ACL 弹窗。
    if sync::backfill_has_token(&mut cfg) {
        let _ = write_sync_config(&app, &cfg);
    }
    Ok(SyncConfigView {
        owner: cfg.owner,
        repo: cfg.repo,
        branch: cfg.branch,
        path: cfg.path,
        has_token: cfg.has_token,
        auto_sync_minutes: cfg.auto_sync_minutes,
        last_sync_at: cfg.last_sync_at,
    })
}

#[tauri::command]
pub fn save_sync_config(app: AppHandle, input: SyncConfigInput) -> Result<(), String> {
    // last_sync_at 由同步流程维护，前端保存配置时原样保留。
    let prev = load_sync_config(&app)?;
    let has_token = match &input.token {
        None => prev.has_token,
        Some(t) => !t.trim().is_empty(),
    };
    let cfg = SyncConfig {
        owner: input.owner.trim().to_string(),
        repo: input.repo.trim().to_string(),
        branch: if input.branch.trim().is_empty() {
            "main".into()
        } else {
            input.branch.trim().to_string()
        },
        path: {
            let p = input.path.trim().trim_start_matches('/');
            if p.is_empty() {
                "vault.sentinel".into()
            } else {
                p.to_string()
            }
        },
        auto_sync_minutes: input.auto_sync_minutes,
        last_sync_at: prev.last_sync_at,
        has_token,
    };
    write_sync_config(&app, &cfg)?;

    match input.token {
        None => {} // 不改动已存 token
        Some(t) if t.trim().is_empty() => sync::delete_token()?,
        Some(t) => sync::set_token(t.trim())?,
    }
    Ok(())
}

/// 测试连接：校验 token 与仓库可访问，返回仓库全名。
#[tauri::command]
pub async fn test_sync_connection(app: AppHandle) -> Result<String, String> {
    let cfg = load_sync_config(&app)?;
    if !cfg.is_complete() {
        return Err("请先填写仓库 owner 与名称".into());
    }
    let token = sync::get_token_cached()?.ok_or("请先填写 GitHub Token")?;
    sync::verify(&cfg, &token).await
}

/// 用会话材料把最终条目封盘、推送远端，成功后落本地 vault + base 快照并更新会话。
/// 先推送再落盘：推送失败（如 409 远端已变）时本地保持不变，用户可重新同步。
#[allow(clippy::too_many_arguments)]
async fn finalize_sync(
    app: &AppHandle,
    state: &State<'_, AppState>,
    final_items: Vec<Item>,
    remote_sha: Option<String>,
    cfg: &SyncConfig,
    token: &str,
    key: &Zeroizing<[u8; KEY_LEN]>,
    params: &KdfParams,
    salt: &[u8; SALT_LEN],
) -> Result<SyncResult, String> {
    let total = final_items.len();
    let data = VaultData { version: 1, items: final_items };
    let bytes = vault::seal(&data, key, params, salt)?;

    sync::push(cfg, token, &bytes, remote_sha).await?;

    std::fs::write(vault_path(app)?, &bytes).map_err(|e| e.to_string())?;
    std::fs::write(base_path(app)?, &bytes).map_err(|e| e.to_string())?;
    record_sync_success(app)?;
    {
        let mut guard = state.session.lock().unwrap();
        if let Some(sess) = guard.as_mut() {
            sess.data.items = data.items.clone();
        }
    }
    *state.pending_sync.lock().unwrap() = None;
    Ok(SyncResult::Synced { total })
}

/// 手动同步：拉取远端密文 → 解密 → 三方合并 →（无冲突则）推送。
/// 有冲突时暂存并返回冲突列表，等待 resolve_sync。
#[tauri::command]
pub async fn sync_now(app: AppHandle, state: State<'_, AppState>) -> Result<SyncResult, String> {
    let (key, password, params, salt, ours) = {
        let guard = state.session.lock().unwrap();
        let sess = guard.as_ref().ok_or("保险库未解锁")?;
        (
            sess.key.clone(),
            sess.password.clone(),
            sess.params,
            sess.salt,
            sess.data.items.clone(),
        )
    };

    let cfg = load_sync_config(&app)?;
    if !cfg.is_complete() {
        return Err("尚未配置同步仓库".into());
    }
    let token = sync::get_token_cached()?.ok_or("尚未配置 GitHub Token")?;

    // 拉取并解密远端（远端由另一台机器用不同 salt 加密，需用主密码重新派生）。
    let remote = sync::pull(&cfg, &token).await?;
    let (theirs, remote_sha) = match remote {
        None => (Vec::new(), None),
        Some(rf) => {
            let pw = password.to_string();
            let bytes = rf.bytes.clone();
            let opened = tauri::async_runtime::spawn_blocking(move || vault::open(&bytes, &pw))
                .await
                .map_err(|e| e.to_string())?
                .map_err(|_| "无法解密远端保险库：两端主密码是否一致？".to_string())?;
            (opened.data.items, rf.sha)
        }
    };

    let base = load_base(&app, &key);

    // 远端已与本地一致 → 无需推送，但仍确保 base 快照对齐。
    if remote_sha.is_some() && merge::same_content(&theirs, &ours) {
        let data = VaultData { version: 1, items: ours };
        let bytes = vault::seal(&data, &key, &params, &salt)?;
        std::fs::write(base_path(&app)?, &bytes).map_err(|e| e.to_string())?;
        record_sync_success(&app)?;
        *state.pending_sync.lock().unwrap() = None;
        return Ok(SyncResult::UpToDate);
    }

    let outcome = merge::merge3(&base, &ours, &theirs);
    if !outcome.conflicts.is_empty() {
        *state.pending_sync.lock().unwrap() = Some(PendingSync {
            merged: outcome.merged,
            conflicts: outcome.conflicts.clone(),
            remote_sha,
        });
        return Ok(SyncResult::Conflict { conflicts: outcome.conflicts });
    }

    let final_items = merge::apply_resolutions(outcome.merged, Vec::new(), &HashMap::new());
    finalize_sync(
        &app,
        &state,
        final_items,
        remote_sha,
        &cfg,
        &token,
        &key,
        &params,
        &salt,
    )
    .await
}

/// 提交用户对冲突的选择，完成合并并推送。choices: 条目 id → "ours" | "theirs"。
#[tauri::command]
pub async fn resolve_sync(
    app: AppHandle,
    state: State<'_, AppState>,
    choices: HashMap<String, Side>,
) -> Result<SyncResult, String> {
    let pending = state
        .pending_sync
        .lock()
        .unwrap()
        .take()
        .ok_or("没有待解决的同步冲突")?;

    let (key, params, salt) = {
        let guard = state.session.lock().unwrap();
        let sess = guard.as_ref().ok_or("保险库未解锁")?;
        (sess.key.clone(), sess.params, sess.salt)
    };

    let cfg = load_sync_config(&app)?;
    let token = sync::get_token_cached()?.ok_or("尚未配置 GitHub Token")?;

    let final_items = merge::apply_resolutions(pending.merged, pending.conflicts, &choices);
    finalize_sync(
        &app,
        &state,
        final_items,
        pending.remote_sha,
        &cfg,
        &token,
        &key,
        &params,
        &salt,
    )
    .await
}

/// 放弃当前待解决的冲突（不推送，本地保持同步前状态）。
#[tauri::command]
pub fn cancel_sync(state: State<'_, AppState>) -> Result<(), String> {
    *state.pending_sync.lock().unwrap() = None;
    Ok(())
}

// ---------- 偏好设置 ----------

/// 读取偏好设置；文件不存在或损坏时返回默认值（不阻断启动）。
#[tauri::command]
pub fn get_settings(app: AppHandle) -> Result<Settings, String> {
    let p = settings_path(&app)?;
    if !p.exists() {
        return Ok(Settings::default());
    }
    let s = std::fs::read_to_string(&p).map_err(|e| e.to_string())?;
    let parsed: Settings = serde_json::from_str(&s).map_err(|e| e.to_string())?;
    Ok(parsed.sanitize())
}

/// 保存偏好设置（规整非法值后落盘）。
#[tauri::command]
pub fn save_settings(app: AppHandle, settings: Settings) -> Result<Settings, String> {
    let clean = settings.sanitize();
    let json = serde_json::to_string_pretty(&clean).map_err(|e| e.to_string())?;
    std::fs::write(settings_path(&app)?, json).map_err(|e| e.to_string())?;
    Ok(clean)
}

// ---------- 加密导出 ----------

/// 把当前明文保险库用一个**独立的导出密码**重新加密（全新随机 salt），
/// 经系统保存对话框写入 .sentinel 文件。绝不导出明文。
/// 返回用户选择的路径字符串；取消则返回 None。
#[tauri::command]
pub async fn export_vault(
    app: AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> Result<Option<String>, String> {
    if password.is_empty() {
        return Err("导出密码不能为空".into());
    }
    let data = {
        let guard = state.session.lock().unwrap();
        let sess = guard.as_ref().ok_or("保险库未解锁")?;
        sess.data.clone()
    };

    // 用导出密码重新派生密钥并封盘（新 salt），与主库解耦。
    let pw = Zeroizing::new(password);
    let bytes = tauri::async_runtime::spawn_blocking(move || {
        let salt = crate::crypto::random_bytes::<SALT_LEN>();
        let params = KdfParams::default();
        let key = crate::crypto::derive_key(&pw, &salt, &params)?;
        vault::seal(&data, &key, &params, &salt)
    })
    .await
    .map_err(|e| e.to_string())??;

    use tauri_plugin_dialog::DialogExt;
    let target = app
        .dialog()
        .file()
        .add_filter("Sentinel 保险库", &["sentinel"])
        .set_file_name("sentinel-export.sentinel")
        .blocking_save_file();

    let Some(path) = target else { return Ok(None) };
    let path_str = path.to_string();
    std::fs::write(&path_str, &bytes).map_err(|e| e.to_string())?;
    Ok(Some(path_str))
}

// ---------- Touch ID 解锁 ----------

/// 设备是否支持生物识别解锁（探测能力，不弹窗）。
#[tauri::command]
pub fn touch_id_available() -> bool {
    crate::touchid::available()
}

/// 是否已存入受生物识别保护的主密码条目（只查属性，不触发 Touch ID）。
#[tauri::command]
pub fn touch_id_enabled() -> bool {
    crate::touchid::enabled()
}

/// 启用 Touch ID：校验主密码与当前已解锁会话一致后，把它写入受保护的钥匙串条目。
/// 写入动作本身会要求一次生物识别确认。
#[tauri::command]
pub fn enable_touch_id(state: State<'_, AppState>, password: String) -> Result<(), String> {
    {
        let guard = state.session.lock().unwrap();
        let sess = guard.as_ref().ok_or("保险库未解锁")?;
        if sess.password.as_str() != password {
            return Err("主密码不正确".into());
        }
    }
    crate::touchid::store(&password)
}

/// 关闭 Touch ID：删除受保护的钥匙串条目。
#[tauri::command]
pub fn disable_touch_id() -> Result<(), String> {
    crate::touchid::remove()
}

/// 用 Touch ID 解锁：从受保护条目读回主密码（触发系统生物识别校验），
/// 随后走与普通解锁完全相同的 vault::open 流程。返回条目数。
#[tauri::command]
pub async fn unlock_with_touch_id(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<usize, String> {
    let path = vault_path(&app)?;
    let bytes = std::fs::read(&path).map_err(|_| "保险库不存在".to_string())?;
    // 读钥匙串会弹出 Touch ID，放到阻塞线程池避免卡住异步运行时。
    let pw = tauri::async_runtime::spawn_blocking(crate::touchid::read)
        .await
        .map_err(|e| e.to_string())??;
    let password = pw.to_string();
    let pw_for_session = pw;
    let opened = tauri::async_runtime::spawn_blocking(move || vault::open(&bytes, &password))
        .await
        .map_err(|e| e.to_string())??;
    let count = opened.data.items.len();
    *state.session.lock().unwrap() = Some(Session {
        key: opened.key,
        password: pw_for_session,
        data: opened.data,
        params: opened.params,
        salt: opened.salt,
    });
    Ok(count)
}
