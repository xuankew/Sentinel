// TOTP（RFC 6238）：从 otpauth:// 链接或裸 base32 密钥生成当前验证码。
use totp_rs::{Algorithm, Secret, TOTP};

pub struct TotpCode {
    pub code: String,
    pub remaining_secs: u64,
    pub period: u64,
}

/// 解析输入（otpauth:// URL 或裸 base32 secret）构造 TOTP。
/// 用 *_unchecked 变体：跳过「secret 必须 ≥128 位」的强校验，以兼容部分
/// 服务商签发的较短密钥（如经典 80 位示例）；base32 合法性与 URL 结构仍会校验。
fn build(input: &str) -> Result<TOTP, String> {
    let trimmed = input.trim();
    if trimmed.starts_with("otpauth://") {
        TOTP::from_url_unchecked(trimmed).map_err(|e| e.to_string())
    } else {
        let secret = Secret::Encoded(trimmed.to_string())
            .to_bytes()
            .map_err(|e| e.to_string())?;
        Ok(TOTP::new_unchecked(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret,
            None,
            String::new(),
        ))
    }
}

pub fn generate(input: &str) -> Result<TotpCode, String> {
    let totp = build(input)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();
    let code = totp.generate(now);
    let period = totp.step;
    let remaining = period - (now % period);
    Ok(TotpCode { code, remaining_secs: remaining, period })
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 6238 附录 B 向量：secret = ASCII "12345678901234567890"
    // 其 base32 = GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ
    // 8 位码 T=59 → 94287082，T=1111111109 → 07081804；6 位码 = 8 位码后 6 位。
    #[test]
    fn known_rfc6238_vectors() {
        let totp = build("GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ").unwrap();
        assert_eq!(totp.generate(59), "287082");
        assert_eq!(totp.generate(1111111109), "081804");
        assert_eq!(totp.generate(1111111111), "050471");
        assert_eq!(totp.generate(1234567890), "005924");
        assert_eq!(totp.generate(2000000000), "279037");
    }

    #[test]
    fn parses_otpauth_url() {
        let url = "otpauth://totp/ACME:alice@example.com?secret=JBSWY3DPEHPK3PXP&issuer=ACME&algorithm=SHA1&digits=6&period=30";
        let totp = build(url).unwrap();
        assert_eq!(totp.digits, 6);
        assert_eq!(totp.step, 30);
        assert_eq!(totp.generate(1234567890).len(), 6);
    }

    #[test]
    fn generate_returns_remaining_within_period() {
        let r = generate("JBSWY3DPEHPK3PXP").unwrap();
        assert_eq!(r.code.len(), 6);
        assert!((1..=r.period).contains(&r.remaining_secs));
    }

    #[test]
    fn invalid_secret_rejected() {
        assert!(generate("!!!!not-base32!!!!").is_err());
    }
}
