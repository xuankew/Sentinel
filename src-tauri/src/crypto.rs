// Sentinel 加密内核：Argon2id 密钥派生 + XChaCha20-Poly1305 认证加密。
// 主密码永不被存储；派生出的主密钥用 Zeroizing 包裹，drop 时自动清零。
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    aead::{Aead, Payload},
    KeyInit, XChaCha20Poly1305, XNonce,
};
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;
pub const SALT_LEN: usize = 16;
pub const NONCE_LEN: usize = 24;

/// Argon2id 派生参数。默认 64 MiB 内存 / 3 迭代 / 1 并行度。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KdfParams {
    pub m_cost: u32, // KiB
    pub t_cost: u32,
    pub p_cost: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self {
            m_cost: 64 * 1024,
            t_cost: 3,
            p_cost: 1,
        }
    }
}

/// 用系统 CSPRNG 生成 N 字节随机数。
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).expect("system RNG failure");
    buf
}

/// 从主密码 + salt 派生 256-bit 主密钥（Argon2id）。
pub fn derive_key(
    password: &str,
    salt: &[u8],
    params: &KdfParams,
) -> Result<Zeroizing<[u8; KEY_LEN]>, String> {
    let p = Params::new(params.m_cost, params.t_cost, params.p_cost, Some(KEY_LEN))
        .map_err(|e| e.to_string())?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, p);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(|e| e.to_string())?;
    Ok(key)
}

/// XChaCha20-Poly1305 加密。`aad` 绑定文件头（防参数/盐被篡改），返回 密文||tag。
pub fn encrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher
        .encrypt(XNonce::from_slice(nonce), Payload { msg: plaintext, aad })
        .map_err(|_| "加密失败".to_string())
}

/// XChaCha20-Poly1305 解密并校验认证标签；密码错误或文件被改都会失败。
pub fn decrypt(
    key: &[u8; KEY_LEN],
    nonce: &[u8; NONCE_LEN],
    aad: &[u8],
    ciphertext: &[u8],
) -> Result<Vec<u8>, String> {
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|e| e.to_string())?;
    cipher
        .decrypt(XNonce::from_slice(nonce), Payload { msg: ciphertext, aad })
        .map_err(|_| "解密失败：主密码错误或文件已损坏".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derive_is_deterministic_and_salt_sensitive() {
        let p = KdfParams { m_cost: 1 << 12, t_cost: 2, p_cost: 1 }; // 测试用低参数，快
        let k1 = derive_key("hunter2", b"0123456789abcdef", &p).unwrap();
        let k2 = derive_key("hunter2", b"0123456789abcdef", &p).unwrap();
        let k3 = derive_key("hunter2", b"fedcba9876543210", &p).unwrap();
        assert_eq!(*k1, *k2);
        assert_ne!(*k1, *k3);
    }

    #[test]
    fn aead_roundtrip_and_tamper_detection() {
        let key = [7u8; KEY_LEN];
        let nonce = random_bytes::<NONCE_LEN>();
        let aad = b"SENTINEL-header";
        let msg = b"{\"secret\":\"value\"}";
        let ct = encrypt(&key, &nonce, aad, msg).unwrap();
        assert_eq!(decrypt(&key, &nonce, aad, &ct).unwrap(), msg);

        // 篡改密文一个比特 → 认证失败
        let mut bad = ct.clone();
        bad[0] ^= 0x01;
        assert!(decrypt(&key, &nonce, aad, &bad).is_err());

        // 篡改 AAD → 认证失败
        assert!(decrypt(&key, &nonce, b"other-header", &ct).is_err());

        // 错误密钥 → 失败
        let wrong = [8u8; KEY_LEN];
        assert!(decrypt(&wrong, &nonce, aad, &ct).is_err());
    }
}
