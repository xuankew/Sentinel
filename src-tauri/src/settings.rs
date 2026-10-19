//! 应用偏好设置（非敏感，明文存 app 数据目录）。
//! 绝不在此存放主密码、密钥或 token——那些分别只在内存、Keychain 中。
use serde::{Deserialize, Serialize};

/// 强调色预设 key，对应前端 CSS 变量组。
pub const ACCENTS: &[&str] = &["teal", "blue", "violet", "amber", "rose", "green"];
/// 密度预设 key。
pub const DENSITIES: &[&str] = &["comfortable", "compact"];

fn default_auto_lock_minutes() -> u32 {
    15
}
fn default_accent() -> String {
    "teal".into()
}
fn default_density() -> String {
    "comfortable".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Settings {
    /// 无操作多少分钟后自动锁定；0 表示永不。
    #[serde(default = "default_auto_lock_minutes")]
    pub auto_lock_minutes: u32,
    /// 强调色 key。
    #[serde(default = "default_accent")]
    pub accent: String,
    /// 密度 key。
    #[serde(default = "default_density")]
    pub density: String,
    /// 是否启用 Touch ID 解锁。
    #[serde(default)]
    pub touch_id_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_lock_minutes: default_auto_lock_minutes(),
            accent: default_accent(),
            density: default_density(),
            touch_id_enabled: false,
        }
    }
}

impl Settings {
    /// 规整非法值到白名单，避免前端传入未知 key 破坏 CSS 变量。
    pub fn sanitize(mut self) -> Self {
        if !ACCENTS.contains(&self.accent.as_str()) {
            self.accent = default_accent();
        }
        if !DENSITIES.contains(&self.density.as_str()) {
            self.density = default_density();
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_sane() {
        let s = Settings::default();
        assert_eq!(s.auto_lock_minutes, 15);
        assert_eq!(s.accent, "teal");
        assert_eq!(s.density, "comfortable");
        assert!(!s.touch_id_enabled);
    }

    #[test]
    fn sanitize_rejects_unknown_keys() {
        let s = Settings {
            auto_lock_minutes: 5,
            accent: "neon-pink".into(),
            density: "huge".into(),
            touch_id_enabled: true,
        }
        .sanitize();
        assert_eq!(s.accent, "teal");
        assert_eq!(s.density, "comfortable");
        assert_eq!(s.auto_lock_minutes, 5); // 数值不动
        assert!(s.touch_id_enabled);
    }

    #[test]
    fn sanitize_keeps_valid_keys() {
        let s = Settings {
            auto_lock_minutes: 0,
            accent: "violet".into(),
            density: "compact".into(),
            touch_id_enabled: false,
        }
        .sanitize();
        assert_eq!(s.accent, "violet");
        assert_eq!(s.density, "compact");
        assert_eq!(s.auto_lock_minutes, 0);
    }

    #[test]
    fn missing_fields_use_defaults() {
        // 旧版本 settings.json 缺字段时应回退到默认，而非报错。
        let s: Settings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, Settings::default());
        let s2: Settings = serde_json::from_str(r#"{"accent":"blue"}"#).unwrap();
        assert_eq!(s2.accent, "blue");
        assert_eq!(s2.auto_lock_minutes, 15);
    }
}
