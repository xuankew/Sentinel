//! 应用层三方合并（base ↔ ours ↔ theirs），不依赖 git merge。
//!
//! 对应 PRD §2.4：
//! - 只有一边改了某条 → 采用改动的一边；
//! - 两边新增不同条 → 都保留（id 不同，天然不冲突）；
//! - 两边都改了同一条且内容不同 → 真冲突，交给 UI 让用户选版本；
//! - 删除会向未改动的一边传播；删除 vs 编辑 → 冲突。
//!
//! 「内容是否相同」只比较逻辑字段（title/kind/tags/favorite），忽略时间戳，
//! 避免两台机器做出相同修改时因 updated_at 不同而误报冲突。
use crate::vault::Item;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Ours,
    Theirs,
}

/// 一条需要用户裁决的冲突。ours/theirs 为 None 表示该侧删除了此条。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MergeConflict {
    pub id: String,
    pub base: Option<Item>,
    pub ours: Option<Item>,
    pub theirs: Option<Item>,
    /// 默认预选：时间较新的一侧；删除 vs 编辑时预选保留编辑（不丢数据）。
    pub suggested: Side,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MergeOutcome {
    /// 已自动合并、无冲突的条目。
    pub merged: Vec<Item>,
    /// 需要用户选择的冲突条目。
    pub conflicts: Vec<MergeConflict>,
}

/// 逻辑内容相等（忽略 created_at / updated_at）。
fn content_eq(a: &Item, b: &Item) -> bool {
    a.id == b.id
        && a.title == b.title
        && a.kind == b.kind
        && a.tags == b.tags
        && a.favorite == b.favorite
}

/// 两个条目集合是否逻辑等价（同 id 集合、每条内容相同，忽略顺序与时间戳）。
/// 用于判断「远端是否已是本地内容」以跳过无谓推送。
pub fn same_content(a: &[Item], b: &[Item]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut av: Vec<&Item> = a.iter().collect();
    let mut bv: Vec<&Item> = b.iter().collect();
    av.sort_by_key(|i| &i.id);
    bv.sort_by_key(|i| &i.id);
    av.iter().zip(bv.iter()).all(|(x, y)| content_eq(x, y))
}

fn index<'a>(items: &'a [Item]) -> BTreeMap<&'a str, &'a Item> {
    items.iter().map(|i| (i.id.as_str(), i)).collect()
}

fn suggest(ours: Option<&Item>, theirs: Option<&Item>) -> Side {
    match (ours, theirs) {
        (Some(o), Some(t)) => {
            if t.updated_at > o.updated_at {
                Side::Theirs
            } else {
                Side::Ours
            }
        }
        // 一边删除、一边编辑：默认保留编辑，避免误删数据。
        (Some(_), None) => Side::Ours,
        (None, Some(_)) => Side::Theirs,
        (None, None) => Side::Ours,
    }
}

/// 三方合并。base 为上次同步成功的快照（首次同步可为空）。
pub fn merge3(base: &[Item], ours: &[Item], theirs: &[Item]) -> MergeOutcome {
    let bm = index(base);
    let om = index(ours);
    let tm = index(theirs);

    let mut ids: BTreeSet<&str> = BTreeSet::new();
    ids.extend(om.keys().copied());
    ids.extend(tm.keys().copied());
    ids.extend(bm.keys().copied());

    let mut merged = Vec::new();
    let mut conflicts = Vec::new();

    for id in ids {
        let b = bm.get(id).copied();
        let o = om.get(id).copied();
        let t = tm.get(id).copied();

        match (b, o, t) {
            // --- base 中存在 ---
            (Some(b), Some(o), Some(t)) => {
                let o_changed = !content_eq(o, b);
                let t_changed = !content_eq(t, b);
                match (o_changed, t_changed) {
                    (false, false) => merged.push(o.clone()),
                    (true, false) => merged.push(o.clone()),
                    (false, true) => merged.push(t.clone()),
                    (true, true) => {
                        if content_eq(o, t) {
                            merged.push(o.clone());
                        } else {
                            conflicts.push(MergeConflict {
                                id: id.to_string(),
                                base: Some(b.clone()),
                                ours: Some(o.clone()),
                                theirs: Some(t.clone()),
                                suggested: suggest(Some(o), Some(t)),
                            });
                        }
                    }
                }
            }
            // 本地删除
            (Some(b), None, Some(t)) => {
                if content_eq(&t, b) {
                    // 远端未改 → 传播删除（不加入 merged）
                } else {
                    conflicts.push(MergeConflict {
                        id: id.to_string(),
                        base: Some(b.clone()),
                        ours: None,
                        theirs: Some(t.clone()),
                        suggested: suggest(None, Some(t)),
                    });
                }
            }
            // 远端删除
            (Some(b), Some(o), None) => {
                if content_eq(o, b) {
                    // 本地未改 → 传播删除
                } else {
                    conflicts.push(MergeConflict {
                        id: id.to_string(),
                        base: Some(b.clone()),
                        ours: Some(o.clone()),
                        theirs: None,
                        suggested: suggest(Some(o), None),
                    });
                }
            }
            // 两边都删 → 消失
            (Some(_), None, None) => {}

            // --- base 中不存在（新增）---
            (None, Some(o), Some(t)) => {
                if content_eq(o, t) {
                    merged.push(o.clone());
                } else {
                    conflicts.push(MergeConflict {
                        id: id.to_string(),
                        base: None,
                        ours: Some(o.clone()),
                        theirs: Some(t.clone()),
                        suggested: suggest(Some(o), Some(t)),
                    });
                }
            }
            (None, Some(o), None) => merged.push(o.clone()),
            (None, None, Some(t)) => merged.push(t.clone()),
            (None, None, None) => unreachable!("id 来自三个索引之一"),
        }
    }

    MergeOutcome { merged, conflicts }
}

/// 应用用户对冲突的选择，得到最终条目集合。未在 choices 中的冲突采用 suggested。
pub fn apply_resolutions(
    merged: Vec<Item>,
    conflicts: Vec<MergeConflict>,
    choices: &HashMap<String, Side>,
) -> Vec<Item> {
    let mut out = merged;
    for c in conflicts {
        let side = choices.get(&c.id).copied().unwrap_or(c.suggested);
        let chosen = match side {
            Side::Ours => c.ours,
            Side::Theirs => c.theirs,
        };
        // 选中删除的一侧 → 条目消失。
        if let Some(item) = chosen {
            out.push(item);
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::ItemKind;

    fn item(id: &str, title: &str, updated: i64) -> Item {
        Item {
            id: id.into(),
            title: title.into(),
            kind: ItemKind::Login {
                username: "u".into(),
                password: title.into(),
                url: "".into(),
                totp: None,
                notes: String::new(),
            },
            tags: vec![],
            favorite: false,
            created_at: 1,
            updated_at: updated,
        }
    }

    fn ids(items: &[Item]) -> Vec<String> {
        let mut v: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        v.sort();
        v
    }

    #[test]
    fn no_changes_keeps_everything() {
        let base = vec![item("a", "A", 10), item("b", "B", 10)];
        let out = merge3(&base, &base.clone(), &base.clone());
        assert!(out.conflicts.is_empty());
        assert_eq!(ids(&out.merged), vec!["a", "b"]);
    }

    #[test]
    fn only_ours_changed_takes_ours() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![item("a", "A-ours", 20)];
        let theirs = vec![item("a", "A", 10)];
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert_eq!(out.merged[0].title, "A-ours");
    }

    #[test]
    fn only_theirs_changed_takes_theirs() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![item("a", "A", 10)];
        let theirs = vec![item("a", "A-theirs", 20)];
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert_eq!(out.merged[0].title, "A-theirs");
    }

    #[test]
    fn both_added_different_items_keeps_both() {
        let base = vec![];
        let ours = vec![item("a", "A", 10)];
        let theirs = vec![item("b", "B", 10)];
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert_eq!(ids(&out.merged), vec!["a", "b"]);
    }

    #[test]
    fn both_changed_same_item_differently_conflicts() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![item("a", "A-ours", 20)];
        let theirs = vec![item("a", "A-theirs", 30)];
        let out = merge3(&base, &ours, &theirs);
        assert_eq!(out.merged.len(), 0);
        assert_eq!(out.conflicts.len(), 1);
        let c = &out.conflicts[0];
        assert_eq!(c.id, "a");
        // theirs 更新（30 > 20）→ 预选 theirs
        assert_eq!(c.suggested, Side::Theirs);
    }

    #[test]
    fn both_changed_same_item_identically_no_conflict() {
        let base = vec![item("a", "A", 10)];
        // 内容相同，仅 updated_at 不同 → 不算冲突
        let ours = vec![item("a", "A-same", 20)];
        let theirs = vec![item("a", "A-same", 99)];
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert_eq!(out.merged[0].title, "A-same");
    }

    #[test]
    fn our_delete_propagates_when_theirs_unchanged() {
        let base = vec![item("a", "A", 10), item("b", "B", 10)];
        let ours = vec![item("b", "B", 10)]; // 删了 a
        let theirs = vec![item("a", "A", 10), item("b", "B", 10)];
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert_eq!(ids(&out.merged), vec!["b"]);
    }

    #[test]
    fn their_delete_propagates_when_ours_unchanged() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![item("a", "A", 10)];
        let theirs = vec![]; // 远端删了 a
        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty());
        assert!(out.merged.is_empty());
    }

    #[test]
    fn delete_vs_edit_conflicts_and_prefers_edit() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![]; // 本地删除
        let theirs = vec![item("a", "A-theirs", 20)]; // 远端编辑
        let out = merge3(&base, &ours, &theirs);
        assert_eq!(out.conflicts.len(), 1);
        let c = &out.conflicts[0];
        assert!(c.ours.is_none());
        assert_eq!(c.suggested, Side::Theirs); // 保留编辑，不丢数据
    }

    #[test]
    fn both_deleted_item_disappears() {
        let base = vec![item("a", "A", 10)];
        let out = merge3(&base, &[], &[]);
        assert!(out.conflicts.is_empty());
        assert!(out.merged.is_empty());
    }

    #[test]
    fn apply_resolutions_honors_choice_and_suggestion() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![item("a", "A-ours", 20)];
        let theirs = vec![item("a", "A-theirs", 30)];
        let out = merge3(&base, &ours, &theirs);

        // 默认（无 choices）→ 用 suggested=theirs
        let final_default = apply_resolutions(out.merged.clone(), out.conflicts.clone(), &HashMap::new());
        assert_eq!(final_default[0].title, "A-theirs");

        // 显式选 ours
        let mut choices = HashMap::new();
        choices.insert("a".to_string(), Side::Ours);
        let final_ours = apply_resolutions(out.merged, out.conflicts, &choices);
        assert_eq!(final_ours[0].title, "A-ours");
    }

    #[test]
    fn resolve_delete_conflict_by_choosing_delete_drops_item() {
        let base = vec![item("a", "A", 10)];
        let ours = vec![]; // 删除
        let theirs = vec![item("a", "A-theirs", 20)]; // 编辑
        let out = merge3(&base, &ours, &theirs);
        let mut choices = HashMap::new();
        choices.insert("a".to_string(), Side::Ours); // 用户坚持删除
        let final_items = apply_resolutions(out.merged, out.conflicts, &choices);
        assert!(final_items.is_empty());
    }

    #[test]
    fn mixed_scenario_multiple_items() {
        // base: a,b,c
        let base = vec![item("a", "A", 10), item("b", "B", 10), item("c", "C", 10)];
        // ours: 改 a、删 b、保留 c、新增 d
        let ours = vec![item("a", "A-ours", 20), item("c", "C", 10), item("d", "D", 20)];
        // theirs: 保留 a、改 c、保留 b、新增 e
        let theirs = vec![item("a", "A", 10), item("b", "B", 10), item("c", "C-theirs", 20), item("e", "E", 20)];

        let out = merge3(&base, &ours, &theirs);
        assert!(out.conflicts.is_empty(), "不同条目各自合并，不应冲突");
        let m: BTreeMap<String, String> =
            out.merged.iter().map(|i| (i.id.clone(), i.title.clone())).collect();
        assert_eq!(m.get("a").unwrap(), "A-ours"); // 只本地改
        assert_eq!(m.get("c").unwrap(), "C-theirs"); // 只远端改
        assert!(!m.contains_key("b"), "本地删、远端未改 → 删除传播");
        assert_eq!(m.get("d").unwrap(), "D"); // 本地新增
        assert_eq!(m.get("e").unwrap(), "E"); // 远端新增
    }
}
