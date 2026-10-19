// Sentinel 保险库：文件格式（SENTINEL 头 + 密文）与数据模型。
// 明文 JSON 只存在于内存，落盘 / 上传前一定经 crypto::encrypt 加密。
use crate::crypto::{self, KdfParams, KEY_LEN, NONCE_LEN, SALT_LEN};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAGIC: &[u8; 8] = b"SENTINEL";
pub const FORMAT_VERSION: u8 = 1;
/// 头部前缀长度：magic(8) + version(1) + m/t/p(12) + salt(16) = 37。
const PREFIX_LEN: usize = 8 + 1 + 12 + SALT_LEN;
const TAG_LEN: usize = 16; // Poly1305 认证标签

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ItemKind {
    Login {
        username: String,
        password: String,
        #[serde(default)]
        url: String,
        #[serde(default)]
        totp: Option<String>,
    },
    ApiKey {
        key_name: String,
        secret: String,
        #[serde(default)]
        url: String,
    },
    Note {
        body: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub kind: ItemKind,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
    pub created_at: i64,
    pub updated_at: i64,
}

/// 前端提交的条目草稿（不含 id / 时间戳，由后端生成）。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ItemDraft {
    pub title: String,
    pub kind: ItemKind,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub favorite: bool,
}

fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VaultData {
    pub version: u32,
    pub items: Vec<Item>,
}

impl VaultData {
    pub fn new() -> Self {
        Self { version: 1, items: Vec::new() }
    }

    pub fn add(&mut self, draft: ItemDraft) -> Item {
        let t = now_secs();
        let item = Item {
            id: Uuid::new_v4().to_string(),
            title: draft.title,
            kind: draft.kind,
            tags: draft.tags,
            favorite: draft.favorite,
            created_at: t,
            updated_at: t,
        };
        self.items.push(item.clone());
        item
    }

    pub fn update(&mut self, id: &str, draft: ItemDraft) -> Result<Item, String> {
        let it = self.items.iter_mut().find(|i| i.id == id).ok_or("条目不存在")?;
        it.title = draft.title;
        it.kind = draft.kind;
        it.tags = draft.tags;
        it.favorite = draft.favorite;
        it.updated_at = now_secs();
        Ok(it.clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), String> {
        let before = self.items.len();
        self.items.retain(|i| i.id != id);
        if self.items.len() == before {
            Err("条目不存在".into())
        } else {
            Ok(())
        }
    }
}

/// 解锁后的会话数据：主密钥 + 明文保险库 + 用于重新加密的头部参数。
pub struct OpenedVault {
    pub key: Zeroizing<[u8; KEY_LEN]>,
    pub data: VaultData,
    pub params: KdfParams,
    pub salt: [u8; SALT_LEN],
}

fn encode_prefix(params: &KdfParams, salt: &[u8; SALT_LEN]) -> Vec<u8> {
    let mut v = Vec::with_capacity(PREFIX_LEN);
    v.extend_from_slice(MAGIC);
    v.push(FORMAT_VERSION);
    v.extend_from_slice(&params.m_cost.to_le_bytes());
    v.extend_from_slice(&params.t_cost.to_le_bytes());
    v.extend_from_slice(&params.p_cost.to_le_bytes());
    v.extend_from_slice(salt);
    v
}

fn decode_prefix(bytes: &[u8]) -> Result<(KdfParams, [u8; SALT_LEN]), String> {
    if bytes.len() < PREFIX_LEN + NONCE_LEN + TAG_LEN {
        return Err("文件过短或已损坏".into());
    }
    if &bytes[..8] != MAGIC {
        return Err("不是有效的 Sentinel 保险库文件".into());
    }
    if bytes[8] != FORMAT_VERSION {
        return Err("不支持的保险库版本".into());
    }
    let m = u32::from_le_bytes(bytes[9..13].try_into().unwrap());
    let t = u32::from_le_bytes(bytes[13..17].try_into().unwrap());
    let p = u32::from_le_bytes(bytes[17..21].try_into().unwrap());
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&bytes[21..PREFIX_LEN]);
    Ok((KdfParams { m_cost: m, t_cost: t, p_cost: p }, salt))
}

/// 把明文保险库加密成完整文件字节（每次生成新 nonce）。
pub fn seal(
    data: &VaultData,
    key: &[u8; KEY_LEN],
    params: &KdfParams,
    salt: &[u8; SALT_LEN],
) -> Result<Vec<u8>, String> {
    let prefix = encode_prefix(params, salt);
    let nonce = crypto::random_bytes::<NONCE_LEN>();
    let plaintext = serde_json::to_vec(data).map_err(|e| e.to_string())?;
    let ct = crypto::encrypt(key, &nonce, &prefix, &plaintext)?;
    let mut out = prefix;
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// 创建全新保险库：生成随机 salt、派生密钥、加密空库，返回会话数据 + 文件字节。
pub fn create(password: &str) -> Result<(OpenedVault, Vec<u8>), String> {
    if password.is_empty() {
        return Err("主密码不能为空".into());
    }
    let salt = crypto::random_bytes::<SALT_LEN>();
    let params = KdfParams::default();
    let key = crypto::derive_key(password, &salt, &params)?;
    let data = VaultData::new();
    let bytes = seal(&data, &key, &params, &salt)?;
    Ok((OpenedVault { key, data, params, salt }, bytes))
}

/// 用**新主密码**重新加密既有明文保险库：生成全新随机 salt、派生新密钥、封盘。
/// 返回新的会话材料（OpenedVault）与完整文件字节。用于「修改主密码」——
/// 旧密钥/旧 salt 一律作废，密文整体重写（云端只经手密文，故也需同步重推）。
pub fn reseal_with_password(
    data: &VaultData,
    new_password: &str,
) -> Result<(OpenedVault, Vec<u8>), String> {
    if new_password.is_empty() {
        return Err("主密码不能为空".into());
    }
    let salt = crypto::random_bytes::<SALT_LEN>();
    let params = KdfParams::default();
    let key = crypto::derive_key(new_password, &salt, &params)?;
    let bytes = seal(data, &key, &params, &salt)?;
    Ok((
        OpenedVault { key, data: data.clone(), params, salt },
        bytes,
    ))
}

/// 从文件字节 + 主密码解密打开保险库。密码错误或文件被改都会 Err。
pub fn open(bytes: &[u8], password: &str) -> Result<OpenedVault, String> {
    let (params, salt) = decode_prefix(bytes)?;
    let prefix = &bytes[..PREFIX_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[PREFIX_LEN..PREFIX_LEN + NONCE_LEN]);
    let ct = &bytes[PREFIX_LEN + NONCE_LEN..];
    let key = crypto::derive_key(password, &salt, &params)?;
    let plaintext = crypto::decrypt(&key, &nonce, prefix, ct)?;
    let data: VaultData = serde_json::from_slice(&plaintext).map_err(|e| e.to_string())?;
    Ok(OpenedVault { key, data, params, salt })
}

/// 用已知密钥解密（本机 seal 的数据，如同步基线快照），省去重新派生。
pub fn open_with_key(bytes: &[u8], key: &[u8; KEY_LEN]) -> Result<VaultData, String> {
    let (_params, _salt) = decode_prefix(bytes)?;
    if bytes.len() < PREFIX_LEN + NONCE_LEN + TAG_LEN {
        return Err("数据过短或已损坏".into());
    }
    let prefix = &bytes[..PREFIX_LEN];
    let mut nonce = [0u8; NONCE_LEN];
    nonce.copy_from_slice(&bytes[PREFIX_LEN..PREFIX_LEN + NONCE_LEN]);
    let ct = &bytes[PREFIX_LEN + NONCE_LEN..];
    let plaintext = crypto::decrypt(key, &nonce, prefix, ct)?;
    serde_json::from_slice(&plaintext).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // 测试用低开销参数（默认 64MiB/3 会让 cargo test 变慢）。
    fn fast_create(pw: &str) -> (OpenedVault, Vec<u8>) {
        let salt = crypto::random_bytes::<SALT_LEN>();
        let params = KdfParams { m_cost: 1 << 12, t_cost: 2, p_cost: 1 };
        let key = crypto::derive_key(pw, &salt, &params).unwrap();
        let data = VaultData::new();
        let bytes = seal(&data, &key, &params, &salt).unwrap();
        (OpenedVault { key, data, params, salt }, bytes)
    }

    #[test]
    fn create_then_open_roundtrip() {
        let (_o, bytes) = fast_create("correct horse");
        let opened = open(&bytes, "correct horse").unwrap();
        assert_eq!(opened.data.version, 1);
        assert!(opened.data.items.is_empty());
    }

    #[test]
    fn wrong_password_rejected() {
        let (_o, bytes) = fast_create("correct horse");
        assert!(open(&bytes, "wrong horse").is_err());
    }

    #[test]
    fn tampered_file_rejected() {
        let (_o, mut bytes) = fast_create("pw123456");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xFF; // 改密文/标签一个字节
        assert!(open(&bytes, "pw123456").is_err());
    }

    #[test]
    fn bad_magic_rejected() {
        let (_o, mut bytes) = fast_create("pw123456");
        bytes[0] = b'X';
        assert!(open(&bytes, "pw123456").is_err());
    }

    #[test]
    fn item_roundtrip_preserves_data() {
        let mut data = VaultData::new();
        data.items.push(Item {
            id: "abc".into(),
            title: "GitHub".into(),
            kind: ItemKind::Login {
                username: "me@x.com".into(),
                password: "s3cr3t!".into(),
                url: "https://github.com".into(),
                totp: None,
            },
            tags: vec!["dev".into()],
            favorite: true,
            created_at: 1,
            updated_at: 2,
        });
        let salt = crypto::random_bytes::<SALT_LEN>();
        let params = KdfParams { m_cost: 1 << 12, t_cost: 2, p_cost: 1 };
        let key = crypto::derive_key("pw", &salt, &params).unwrap();
        let bytes = seal(&data, &key, &params, &salt).unwrap();
        let opened = open(&bytes, "pw").unwrap();
        assert_eq!(opened.data, data);
    }

    /// 端到端：真实默认参数（64MiB Argon2id）+ 磁盘持久化 + 密文不泄露明文。
    #[test]
    fn e2e_real_params_file_persistence() {
        use std::io::{Read, Write};

        let path = std::env::temp_dir().join(format!(
            "sentinel_e2e_{}_{}.sentinel",
            std::process::id(),
            crypto::random_bytes::<4>()[0]
        ));
        let password = "Tr0ub4dor&3-correct-horse";
        let secret_value = "SUPER-SECRET-API-KEY-9f8e7d";

        let contains = |hay: &[u8], needle: &str| {
            !needle.is_empty() && hay.windows(needle.len()).any(|w| w == needle.as_bytes())
        };

        // 1) 创建（真实默认参数）并落盘
        let (_opened, bytes) = create(password).unwrap();
        assert_eq!(&bytes[..8], MAGIC, "文件应以 SENTINEL magic 开头");
        std::fs::File::create(&path).unwrap().write_all(&bytes).unwrap();

        // 2) 密文绝不能包含明文主密码
        assert!(!contains(&bytes, password), "密文不应包含明文主密码");

        // 3) 从磁盘读回，用正确密码打开
        let mut disk = Vec::new();
        std::fs::File::open(&path).unwrap().read_to_end(&mut disk).unwrap();
        let mut vault = open(&disk, password).unwrap();
        assert!(vault.data.items.is_empty());

        // 4) 错误密码 / 篡改字节 → 拒绝
        assert!(open(&disk, "wrong-password").is_err(), "错误密码应被拒绝");
        let mut tampered = disk.clone();
        let li = tampered.len() - 1;
        tampered[li] ^= 0x01;
        assert!(open(&tampered, password).is_err(), "篡改文件应被拒绝");

        // 5) 加一条含 secret 的条目 → 重新加密落盘 → 密文不含 secret 明文
        vault.data.items.push(Item {
            id: "id-1".into(),
            title: "Stripe".into(),
            kind: ItemKind::ApiKey {
                key_name: "live".into(),
                secret: secret_value.into(),
                url: "https://stripe.com".into(),
            },
            tags: vec!["finance".into()],
            favorite: true,
            created_at: 10,
            updated_at: 10,
        });
        let resealed = seal(&vault.data, &vault.key, &vault.params, &vault.salt).unwrap();
        assert!(!contains(&resealed, secret_value), "密文不应包含条目 secret 明文");
        assert!(!contains(&resealed, "Stripe"), "密文不应包含条目标题明文");
        std::fs::File::create(&path).unwrap().write_all(&resealed).unwrap();

        // 6) 重新从磁盘打开（模拟重启后解锁），数据完整保留
        let mut disk2 = Vec::new();
        std::fs::File::open(&path).unwrap().read_to_end(&mut disk2).unwrap();
        let reopened = open(&disk2, password).unwrap();
        assert_eq!(reopened.data.items.len(), 1);
        assert_eq!(reopened.data.items[0].title, "Stripe");
        match &reopened.data.items[0].kind {
            ItemKind::ApiKey { secret, .. } => assert_eq!(secret, secret_value),
            _ => panic!("expected ApiKey"),
        }

        // 7) 每次 seal 用新随机 nonce → 相同数据两次密文不同
        let again = seal(&vault.data, &vault.key, &vault.params, &vault.salt).unwrap();
        assert_ne!(resealed[PREFIX_LEN..], again[PREFIX_LEN..], "nonce 应随机，密文不应重复");

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn crud_add_update_delete() {
        let mut data = VaultData::new();

        let login = ItemKind::Login {
            username: "alice".into(),
            password: "pw1".into(),
            url: "https://a.com".into(),
            totp: None,
        };
        let created = data.add(ItemDraft {
            title: "A".into(),
            kind: login.clone(),
            tags: vec!["x".into()],
            favorite: false,
        });
        assert_eq!(data.items.len(), 1);
        assert!(!created.id.is_empty());
        assert_eq!(created.created_at, created.updated_at);

        // update：改标题 + 收藏，id/created_at 不变，updated_at 递增
        let updated = data
            .update(
                &created.id,
                ItemDraft {
                    title: "A2".into(),
                    kind: login,
                    tags: vec![],
                    favorite: true,
                },
            )
            .unwrap();
        assert_eq!(updated.id, created.id);
        assert_eq!(updated.title, "A2");
        assert!(updated.favorite);
        assert_eq!(updated.created_at, created.created_at);
        assert!(updated.updated_at >= created.updated_at);

        // update 不存在的 id → 报错
        assert!(data
            .update(
                "nope",
                ItemDraft { title: "z".into(), kind: ItemKind::Note { body: "".into() }, tags: vec![], favorite: false }
            )
            .is_err());

        // delete
        data.delete(&created.id).unwrap();
        assert!(data.items.is_empty());
        assert!(data.delete(&created.id).is_err());
    }

    /// 修改主密码：用新密码重封后，新密码可开、旧密码被拒、salt 更换、数据完整保留。
    #[test]
    fn reseal_with_new_password_rotates_key() {
        let (opened, _old_bytes) = fast_create("old-pass-123");
        let mut data = opened.data.clone();
        data.items.push(Item {
            id: "id-1".into(),
            title: "GitHub".into(),
            kind: ItemKind::Login {
                username: "me@x.com".into(),
                password: "s3cr3t!".into(),
                url: "https://github.com".into(),
                totp: None,
            },
            tags: vec!["dev".into()],
            favorite: true,
            created_at: 1,
            updated_at: 2,
        });

        let (rotated, new_bytes) = reseal_with_password(&data, "new-pass-456").unwrap();
        // 新 salt（密钥随之更换）
        assert_ne!(rotated.salt, opened.salt, "重封应使用全新随机 salt");

        // 新密码可打开且数据完整
        let via_new = open(&new_bytes, "new-pass-456").unwrap();
        assert_eq!(via_new.data, data);
        // 旧密码被拒
        assert!(open(&new_bytes, "old-pass-123").is_err(), "旧主密码应不再能解密");
    }
}
