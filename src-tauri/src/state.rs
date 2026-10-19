// 解锁后的会话状态：主密钥 + 明文保险库，保存在内存中，锁定时 drop 清零。
use crate::crypto::{KdfParams, KEY_LEN, SALT_LEN};
use crate::merge::MergeConflict;
use crate::vault::{Item, VaultData};
use std::sync::Mutex;
use zeroize::Zeroizing;

/// 解锁会话：主密钥 + 明文保险库 + 重新加密所需的头部参数。
///
/// `password` 仅在解锁期间驻留内存（Zeroizing 锁定即清零），用于同步时
/// 针对**远端机器的不同 salt** 重新派生密钥来解密远端密文——每台 Mac 创建
/// 保险库时 salt 随机，同一主密码在不同机器派生出不同密钥。
pub struct Session {
    pub key: Zeroizing<[u8; KEY_LEN]>,
    pub password: Zeroizing<String>,
    pub data: VaultData,
    pub params: KdfParams,
    pub salt: [u8; SALT_LEN],
}

/// 一次同步中检测到冲突、等待用户裁决时的暂存状态。
/// 保存自动合并结果 + 冲突列表 + 拉取时的远端 sha（用于推送时的乐观并发）。
pub struct PendingSync {
    pub merged: Vec<Item>,
    pub conflicts: Vec<MergeConflict>,
    pub remote_sha: Option<String>,
}

#[derive(Default)]
pub struct AppState {
    pub session: Mutex<Option<Session>>,
    pub pending_sync: Mutex<Option<PendingSync>>,
}
