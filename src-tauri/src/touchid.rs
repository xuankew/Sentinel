//! Touch ID 解锁：把主密码存进一个「生物识别门禁」的 macOS 钥匙串条目，
//! 读取该条目时系统自动触发 Touch ID/Face ID 校验。
//!
//! 安全模型：主密码本身仍**永不**明文落盘、永不上云。这里存的是钥匙串里一条
//! 受 `kSecAccessControlBiometryCurrentSet` 保护的数据（数据保护钥匙串），
//! 只有当前已登记的生物特征通过才能取回。取回后走与普通解锁完全相同的
//! `vault::open` 流程派生密钥。
//!
//! 重要限制：数据保护钥匙串（`kSecUseDataProtectionKeychain`）要求应用具备
//! **有效代码签名**。未签名的 `tauri dev` 下写入/读取会失败——这是预期行为，
//! 所有入口都优雅降级为错误字符串，不会 panic；真正的 Touch ID 验证需在
//! 已签名的构建 + 真机上完成。
use objc2_local_authentication::{LAContext, LAPolicy};
use security_framework::item::{ItemClass, ItemSearchOptions};
use security_framework::passwords::{
    delete_generic_password_options, generic_password, set_generic_password_options,
    AccessControlOptions, PasswordOptions,
};
use zeroize::Zeroizing;

const SERVICE: &str = "com.sentinel.app";
const ACCOUNT: &str = "touch-id-master-password";

/// 构造针对该受保护条目的查询：生物识别门禁 + 数据保护钥匙串。
fn options() -> PasswordOptions {
    let mut o = PasswordOptions::new_generic_password(SERVICE, ACCOUNT);
    o.set_access_control_options(AccessControlOptions::BIOMETRY_CURRENT_SET);
    o.use_protected_keychain();
    o
}

/// 设备是否支持生物识别解锁（已登记 Touch ID/Face ID 且当前可用）。
/// 只探测能力，不弹窗、不触碰钥匙串条目。
pub fn available() -> bool {
    // SAFETY: LAContext::new + canEvaluatePolicy_error 均为只读能力探测，无副作用。
    unsafe {
        let ctx = LAContext::new();
        ctx.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
            .is_ok()
    }
}

/// 是否已存入受生物识别保护的主密码条目。
/// 只查询属性（不请求 data），因此**不会**触发 Touch ID 弹窗。
pub fn enabled() -> bool {
    let mut search = ItemSearchOptions::new();
    search
        .class(ItemClass::generic_password())
        .service(SERVICE)
        .account(ACCOUNT)
        .load_attributes(true)
        .ignore_legacy_keychains()
        .limit(1);
    matches!(search.search(), Ok(items) if !items.is_empty())
}

/// 存入（或更新）受生物识别保护的主密码。写入时系统会要求一次生物识别确认。
pub fn store(password: &str) -> Result<(), String> {
    if password.is_empty() {
        return Err("主密码不能为空".into());
    }
    set_generic_password_options(password.as_bytes(), options()).map_err(keychain_err)
}

/// 删除受生物识别保护的主密码条目（关闭 Touch ID 解锁）。
pub fn remove() -> Result<(), String> {
    match delete_generic_password_options(options()) {
        Ok(()) => Ok(()),
        // 条目本就不存在视作成功（幂等关闭）。
        Err(e) if e.code() == security_framework_sys::base::errSecItemNotFound => Ok(()),
        Err(e) => Err(keychain_err(e)),
    }
}

/// 读取受保护的主密码——此调用会触发 Touch ID/Face ID 校验弹窗。
/// 校验通过后返回明文主密码（Zeroizing，用完即清零）。
pub fn read() -> Result<Zeroizing<String>, String> {
    let bytes = generic_password(options()).map_err(keychain_err)?;
    let pw = Zeroizing::new(
        String::from_utf8(bytes).map_err(|_| "钥匙串中的主密码已损坏".to_string())?,
    );
    Ok(pw)
}

fn keychain_err(e: security_framework::base::Error) -> String {
    use security_framework_sys::base as sec;
    match e.code() {
        sec::errSecItemNotFound => "尚未启用 Touch ID 解锁".into(),
        sec::errSecAuthFailed => "生物识别校验未通过".into(),
        // 未签名构建访问数据保护钥匙串、用户取消、生物识别不可用等都落到这里：
        // 给出可读提示而非裸错误码。
        _ => format!(
            "钥匙串操作失败（{}）。若在未签名开发构建中运行，Touch ID 需已签名的应用才能使用。",
            e
        ),
    }
}
