// 从 CSV 导入条目：表头驱动，一套逻辑覆盖 Chrome / 1Password / Bitwarden / 通用导出。
// CSV 是用户自己的明文导出，仅在此内存解析成 ItemDraft，随后由调用方写入加密库；
// 本模块不落盘、不复制明文，也不感知 Tauri。
use crate::vault::{Item, ItemDraft, ItemKind};
use serde::Serialize;
use std::collections::HashSet;

/// 导入结果摘要，回传前端展示。
#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct ImportSummary {
    /// 实际新增到保险库的条目数。
    pub imported: usize,
    /// 因与库中（或本批内）已有条目完全重复而跳过。
    pub skipped_duplicate: usize,
    /// 空行或缺少可用字段（无账号/密码/网址/备注）而跳过。
    pub skipped_invalid: usize,
}

/// CSV 中我们关心的列角色。
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Col {
    Title,
    Username,
    Password,
    Url,
    Totp,
    Notes,
    Type,
}

/// 把一个表头名归一到列角色。忽略大小写、空格、下划线，并剥掉 Bitwarden 的 `login.` 前缀。
fn classify(header: &str) -> Option<Col> {
    let mut h = header.trim().to_lowercase();
    if let Some(rest) = h.strip_prefix("login.") {
        h = rest.to_string();
    }
    let key: String = h.chars().filter(|c| !c.is_whitespace() && *c != '_').collect();
    match key.as_str() {
        "title" | "name" | "itemname" | "account" => Some(Col::Title),
        "username" | "user" | "login" | "email" => Some(Col::Username),
        "password" | "pass" | "passwd" => Some(Col::Password),
        "url" | "uri" | "website" | "site" | "signinurl" | "loginurl" => Some(Col::Url),
        "totp" | "otp" | "2fa" | "mfa" => Some(Col::Totp),
        "notes" | "note" | "comment" | "comments" | "extra" => Some(Col::Notes),
        "type" | "category" | "itemtype" | "kind" => Some(Col::Type),
        _ => None,
    }
}

fn norm(s: &str) -> String {
    s.trim().to_lowercase()
}

/// 从网址取主机名，用作缺标题时的兜底。
fn host_of(url: &str) -> String {
    let u = url.trim();
    let after = u.split("://").nth(1).unwrap_or(u);
    after.split('/').next().unwrap_or(after).trim().to_string()
}

/// 解析结果：待入库草稿 + 解析阶段跳过的无效行数（去重前）。
pub struct Parsed {
    pub drafts: Vec<ItemDraft>,
    pub skipped_invalid: usize,
}

/// 解析 CSV 文本为 ItemDraft 列表。表头无法识别（无 title/username/password 任一列）时报错。
pub fn parse_csv(content: &str) -> Result<Parsed, String> {
    // 剥掉可能存在的 UTF-8 BOM，否则首个表头会带 \u{feff} 导致识别失败。
    let content = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut rdr = csv::ReaderBuilder::new()
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.as_bytes());

    let headers = rdr.headers().map_err(|e| e.to_string())?.clone();
    let roles: Vec<Option<Col>> = headers.iter().map(classify).collect();

    let has_meaningful = roles.iter().any(|r| {
        matches!(r, Some(Col::Title) | Some(Col::Username) | Some(Col::Password))
    });
    if !has_meaningful {
        return Err("无法识别 CSV 列：未找到 title/username/password 任一列".into());
    }

    let idx = |role: Col| roles.iter().position(|r| *r == Some(role));
    let get = |rec: &csv::StringRecord, role: Col| -> String {
        idx(role).and_then(|i| rec.get(i)).unwrap_or("").to_string()
    };

    let mut drafts = Vec::new();
    let mut skipped_invalid = 0usize;

    for result in rdr.records() {
        let rec = result.map_err(|e| e.to_string())?;
        let title_raw = get(&rec, Col::Title);
        let username = get(&rec, Col::Username);
        let password = get(&rec, Col::Password);
        let url = get(&rec, Col::Url);
        let totp = get(&rec, Col::Totp);
        let notes = get(&rec, Col::Notes);
        let typ = get(&rec, Col::Type).to_lowercase();

        let all_empty = title_raw.is_empty()
            && username.is_empty()
            && password.is_empty()
            && url.is_empty()
            && totp.is_empty()
            && notes.is_empty();
        if all_empty {
            skipped_invalid += 1;
            continue;
        }

        let no_credentials =
            password.is_empty() && username.is_empty() && url.is_empty() && totp.is_empty();

        // 纯笔记：类型标注为 note，或没有任何凭据但有备注。
        if typ.contains("note") || (no_credentials && !notes.is_empty()) {
            let title = if title_raw.is_empty() { "未命名笔记".to_string() } else { title_raw };
            drafts.push(ItemDraft {
                title,
                kind: ItemKind::Note { body: notes },
                tags: vec![],
                favorite: false,
            });
            continue;
        }

        // 有标题但无任何凭据/备注 → 无法成为有用条目。
        if no_credentials && notes.is_empty() {
            skipped_invalid += 1;
            continue;
        }

        let title = if !title_raw.is_empty() {
            title_raw
        } else {
            let h = host_of(&url);
            if !h.is_empty() {
                h
            } else if !username.is_empty() {
                username.clone()
            } else {
                "未命名".to_string()
            }
        };

        drafts.push(ItemDraft {
            title,
            kind: ItemKind::Login {
                username,
                password,
                url,
                totp: if totp.is_empty() { None } else { Some(totp) },
                notes,
            },
            tags: vec![],
            favorite: false,
        });
    }

    Ok(Parsed { drafts, skipped_invalid })
}

/// 条目唯一键：同类且关键字段（含密码）全等视为重复。仅用于导入去重，非加密用途。
fn key_of(title: &str, kind: &ItemKind) -> String {
    match kind {
        ItemKind::Login { username, password, url, .. } => {
            format!("login|{}|{}|{}|{}", norm(title), norm(username), norm(password), norm(url))
        }
        ItemKind::Note { body } => format!("note|{}|{}", norm(title), norm(body)),
        ItemKind::ApiKey { key_name, secret, url, .. } => {
            format!("api|{}|{}|{}|{}", norm(title), norm(key_name), norm(secret), norm(url))
        }
    }
}

/// 去重：跳过与 `existing`（库中已有）或本批内前面条目完全重复的草稿。
/// 返回（保留的草稿, 跳过的重复数）。
pub fn dedup(drafts: Vec<ItemDraft>, existing: &[Item]) -> (Vec<ItemDraft>, usize) {
    let mut seen: HashSet<String> =
        existing.iter().map(|i| key_of(&i.title, &i.kind)).collect();
    let mut out = Vec::with_capacity(drafts.len());
    let mut skipped = 0usize;
    for d in drafts {
        let k = key_of(&d.title, &d.kind);
        if seen.contains(&k) {
            skipped += 1;
            continue;
        }
        seen.insert(k);
        out.push(d);
    }
    (out, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn logins(parsed: &Parsed) -> Vec<(&str, &str, &str, Option<&str>)> {
        parsed
            .drafts
            .iter()
            .filter_map(|d| match &d.kind {
                ItemKind::Login { username, password, url, totp, .. } => {
                    Some((username.as_str(), password.as_str(), url.as_str(), totp.as_deref()))
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn chrome_csv_maps_to_logins() {
        let csv = "name,url,username,password,note\n\
                   GitHub,https://github.com,alice,hunter2,\n\
                   Aliyun,https://aliyun.com,bob,pass-w0rd,remember to rotate\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts.len(), 2);
        assert_eq!(p.drafts[0].title, "GitHub");
        let l = logins(&p);
        assert_eq!(l[0], ("alice", "hunter2", "https://github.com", None));
        assert_eq!(l[1], ("bob", "pass-w0rd", "https://aliyun.com", None));
        // 第二行的备注写入 Login.notes（不再丢弃）
        match &p.drafts[1].kind {
            ItemKind::Login { notes, .. } => assert_eq!(notes, "remember to rotate"),
            other => panic!("expected Login, got {:?}", other),
        }
        assert_eq!(p.skipped_invalid, 0);
    }

    #[test]
    fn onepassword_csv_with_type_and_note() {
        let csv = "Title,Username,Password,URL,Notes,Type\n\
                   GitHub,alice,hunter2,https://github.com,,Login\n\
                   My Secret Note,,, ,pin is 1234,Secure Note\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts.len(), 2);
        assert!(matches!(p.drafts[0].kind, ItemKind::Login { .. }));
        match &p.drafts[1].kind {
            ItemKind::Note { body } => assert_eq!(body, "pin is 1234"),
            other => panic!("expected Note, got {:?}", other),
        }
        assert_eq!(p.drafts[1].title, "My Secret Note");
    }

    #[test]
    fn bitwarden_dotted_headers() {
        let csv = "folder,favorite,name,notes,login.uri,login.username,login.password,login.totp\n\
                   Work,1,GitLab,hi,https://gitlab.com,carol,p@ss,JBSWY3DPEHPK3PXP\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts.len(), 1);
        assert_eq!(p.drafts[0].title, "GitLab");
        let l = logins(&p);
        assert_eq!(l[0], ("carol", "p@ss", "https://gitlab.com", Some("JBSWY3DPEHPK3PXP")));
        match &p.drafts[0].kind {
            ItemKind::Login { notes, .. } => assert_eq!(notes, "hi"),
            other => panic!("expected Login, got {:?}", other),
        }
    }

    #[test]
    fn quoted_fields_with_comma_and_newline() {
        let csv = "Title,Username,Password,URL\n\
                   \"Acme, Inc.\",alice,\"p@ss,word\",\"https://acme.com\"\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts[0].title, "Acme, Inc.");
        let l = logins(&p);
        assert_eq!(l[0].1, "p@ss,word");
    }

    #[test]
    fn bom_is_stripped() {
        let csv = "\u{feff}Title,Username,Password\nGitHub,alice,hunter2\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts[0].title, "GitHub");
    }

    #[test]
    fn empty_and_credential_less_rows_skipped() {
        let csv = "Title,Username,Password,URL,Notes\n\
                   GitHub,alice,hunter2,https://github.com,\n\
                   ,,,,,\n\
                   OnlyTitle,,,,\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts.len(), 1);
        // 一行全空 + 一行只有标题无凭据无备注 = 2 无效
        assert_eq!(p.skipped_invalid, 2);
    }

    #[test]
    fn title_falls_back_to_url_host() {
        let csv = "Title,Username,Password,URL\n,alice,hunter2,https://gitlab.com/login\n";
        let p = parse_csv(csv).unwrap();
        assert_eq!(p.drafts[0].title, "gitlab.com");
    }

    #[test]
    fn unrecognized_columns_error() {
        let csv = "foo,bar,baz\n1,2,3\n";
        assert!(parse_csv(csv).is_err());
    }

    #[test]
    fn dedup_skips_existing_and_internal_duplicates() {
        let existing = vec![Item {
            id: "x".into(),
            title: "GitHub".into(),
            kind: ItemKind::Login {
                username: "alice".into(),
                password: "hunter2".into(),
                url: "https://github.com".into(),
                totp: None,
                notes: String::new(),
            },
            tags: vec![],
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }];
        let csv = "Title,Username,Password,URL\n\
                   GitHub,alice,hunter2,https://github.com\n\
                   GitHub,alice,hunter2,https://github.com\n\
                   GitLab,bob,secret,https://gitlab.com\n";
        let p = parse_csv(csv).unwrap();
        let (kept, skipped) = dedup(p.drafts, &existing);
        // 第一条撞库中已有、第二条撞本批第一条 → 都跳过；只剩 GitLab
        assert_eq!(skipped, 2);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title, "GitLab");
    }

    #[test]
    fn dedup_treats_changed_password_as_new() {
        let existing = vec![Item {
            id: "x".into(),
            title: "GitHub".into(),
            kind: ItemKind::Login {
                username: "alice".into(),
                password: "old".into(),
                url: "https://github.com".into(),
                totp: None,
                notes: String::new(),
            },
            tags: vec![],
            favorite: false,
            created_at: 0,
            updated_at: 0,
        }];
        let csv = "Title,Username,Password,URL\nGitHub,alice,new,https://github.com\n";
        let p = parse_csv(csv).unwrap();
        let (kept, skipped) = dedup(p.drafts, &existing);
        assert_eq!(skipped, 0);
        assert_eq!(kept.len(), 1);
    }
}
