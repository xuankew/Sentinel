// 类型化的后端调用封装，类型与 Rust 端 serde 序列化保持一致。
import { invoke } from "@tauri-apps/api/core";

export type ItemKind =
  | { type: "login"; username: string; password: string; url: string; totp: string | null }
  | { type: "api_key"; key_name: string; secret: string; url: string }
  | { type: "note"; body: string };

export interface Item {
  id: string;
  title: string;
  kind: ItemKind;
  tags: string[];
  favorite: boolean;
  created_at: number;
  updated_at: number;
}

export interface ItemDraft {
  title: string;
  kind: ItemKind;
  tags: string[];
  favorite: boolean;
}

export interface TotpResult {
  code: string;
  remaining_secs: number;
  period: number;
}

// ---- 同步（M3）----

export interface SyncConfigView {
  owner: string;
  repo: string;
  branch: string;
  path: string;
  has_token: boolean;
  /** 自动同步间隔（分钟）；0 = 关闭 */
  auto_sync_minutes: number;
  /** 上次同步成功的 unix 秒；从未同步为 null */
  last_sync_at: number | null;
}

export interface SyncConfigInput {
  owner: string;
  repo: string;
  branch: string;
  path: string;
  /** null=不改动；""=清除；值=写入 Keychain */
  token: string | null;
  auto_sync_minutes: number;
}

export type Side = "ours" | "theirs";

export interface MergeConflict {
  id: string;
  base: Item | null;
  ours: Item | null;
  theirs: Item | null;
  suggested: Side;
}

export type SyncResult =
  | { status: "up_to_date" }
  | { status: "synced"; total: number }
  | { status: "conflict"; conflicts: MergeConflict[] };

// ---- 偏好设置（M4）----

export type Accent = "teal" | "blue" | "violet" | "amber" | "rose" | "green";
export type Density = "comfortable" | "compact";

export interface Settings {
  /** 无操作多少分钟后自动锁定；0=永不 */
  auto_lock_minutes: number;
  accent: Accent;
  density: Density;
  touch_id_enabled: boolean;
}

export const api = {
  vaultExists: () => invoke<boolean>("vault_exists"),
  createVault: (password: string) => invoke<number>("create_vault", { password }),
  unlock: (password: string) => invoke<number>("unlock", { password }),
  lock: () => invoke<void>("lock"),
  isUnlocked: () => invoke<boolean>("is_unlocked"),
  changeMasterPassword: (oldPassword: string, newPassword: string) =>
    invoke<void>("change_master_password", { oldPassword, newPassword }),

  listItems: () => invoke<Item[]>("list_items"),
  createItem: (draft: ItemDraft) => invoke<Item>("create_item", { draft }),
  updateItem: (id: string, draft: ItemDraft) => invoke<Item>("update_item", { id, draft }),
  deleteItem: (id: string) => invoke<void>("delete_item", { id }),

  copySecret: (value: string, clearAfter?: number) =>
    invoke<void>("copy_secret", { value, clearAfter }),
  generateTotp: (secret: string) => invoke<TotpResult>("generate_totp", { secret }),

  getSyncConfig: () => invoke<SyncConfigView>("get_sync_config"),
  saveSyncConfig: (input: SyncConfigInput) => invoke<void>("save_sync_config", { input }),
  testSyncConnection: () => invoke<string>("test_sync_connection"),
  syncNow: () => invoke<SyncResult>("sync_now"),
  resolveSync: (choices: Record<string, Side>) => invoke<SyncResult>("resolve_sync", { choices }),
  cancelSync: () => invoke<void>("cancel_sync"),

  getSettings: () => invoke<Settings>("get_settings"),
  saveSettings: (settings: Settings) => invoke<Settings>("save_settings", { settings }),
  exportVault: (password: string) => invoke<string | null>("export_vault", { password }),

  touchIdAvailable: () => invoke<boolean>("touch_id_available"),
  touchIdEnabled: () => invoke<boolean>("touch_id_enabled"),
  enableTouchId: (password: string) => invoke<void>("enable_touch_id", { password }),
  disableTouchId: () => invoke<void>("disable_touch_id"),
  unlockWithTouchId: () => invoke<number>("unlock_with_touch_id"),
};

// ---- 展示辅助 ----

export function kindLabel(k: ItemKind): string {
  return { login: "登录密码", api_key: "API Key", note: "安全笔记" }[k.type];
}

export function subtitle(it: Item): string {
  switch (it.kind.type) {
    case "login":
      return it.kind.username || it.kind.url || "登录";
    case "api_key":
      return it.kind.key_name || maskSecret(it.kind.secret) || "API Key";
    case "note":
      return "安全笔记";
  }
}

export function maskSecret(v: string): string {
  if (!v) return "";
  if (v.length <= 4) return "•".repeat(v.length);
  return `${v.slice(0, 2)}${"•".repeat(Math.min(v.length - 2, 12))}${v.slice(-2)}`;
}

export function initials(title: string): string {
  const words = title
    .replace(/[^0-9A-Za-z\u4e00-\u9fa5 ]+/g, " ")
    .trim()
    .split(" ")
    .filter(Boolean);
  return (words.slice(0, 2).map((w) => w[0]).join("") || "S").toUpperCase();
}

// 按标题稳定分配一个头像渐变色
const AVATAR_GRADIENTS = [
  "linear-gradient(135deg,#4FD1C5,#3B82F6)",
  "linear-gradient(135deg,#F6AD55,#ED8936)",
  "linear-gradient(135deg,#9F7AEA,#6B46C1)",
  "linear-gradient(135deg,#F687B3,#D53F8C)",
  "linear-gradient(135deg,#63B3ED,#3182CE)",
  "linear-gradient(135deg,#68D391,#38A169)",
];
export function avatarColor(seed: string): string {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) >>> 0;
  return AVATAR_GRADIENTS[h % AVATAR_GRADIENTS.length];
}

// ---- 同步 / 冲突展示辅助 ----

export function sideLabel(s: Side): string {
  return s === "ours" ? "本机版本" : "远端版本";
}

/** 相对时间：刚刚 / N 分钟前 / N 小时前 / N 天前 / 日期。 */
export function relTime(secs: number): string {
  if (!secs) return "未知";
  const diff = Math.floor(Date.now() / 1000) - secs;
  if (diff < 60) return "刚刚";
  if (diff < 3600) return `${Math.floor(diff / 60)} 分钟前`;
  if (diff < 86400) return `${Math.floor(diff / 3600)} 小时前`;
  if (diff < 86400 * 30) return `${Math.floor(diff / 86400)} 天前`;
  return new Date(secs * 1000).toLocaleDateString("zh-CN");
}

/** 该条目的「主敏感值」，用于冲突对比时打码展示。 */
export function primarySecret(it: Item): string {
  switch (it.kind.type) {
    case "login":
      return it.kind.password;
    case "api_key":
      return it.kind.secret;
    case "note":
      return it.kind.body;
  }
}

/** 列出两版本之间发生变化的字段（中文标签），用于冲突卡片摘要。 */
export function diffFields(a: Item | null, b: Item | null): string[] {
  if (!a || !b) return [];
  const changed: string[] = [];
  if (a.title !== b.title) changed.push("标题");
  if (a.favorite !== b.favorite) changed.push("收藏");
  if ((a.tags ?? []).join(",") !== (b.tags ?? []).join(",")) changed.push("标签");
  const ka = a.kind;
  const kb = b.kind;
  if (ka.type === "login" && kb.type === "login") {
    if (ka.username !== kb.username) changed.push("用户名");
    if (ka.password !== kb.password) changed.push("密码");
    if (ka.url !== kb.url) changed.push("网址");
    if ((ka.totp ?? "") !== (kb.totp ?? "")) changed.push("TOTP");
  } else if (ka.type === "api_key" && kb.type === "api_key") {
    if (ka.key_name !== kb.key_name) changed.push("Key 名称");
    if (ka.secret !== kb.secret) changed.push("Secret");
    if (ka.url !== kb.url) changed.push("网址");
  } else if (ka.type === "note" && kb.type === "note") {
    if (ka.body !== kb.body) changed.push("内容");
  } else {
    changed.push("类型");
  }
  return changed;
}

/** 冲突性质：编辑 vs 编辑 / 本机删除 / 远端删除 / 双方新增。 */
export function conflictKind(c: MergeConflict): string {
  if (!c.ours && c.theirs) return "本机已删除，远端有修改";
  if (c.ours && !c.theirs) return "远端已删除，本机有修改";
  if (!c.base) return "双方新增了同一条目";
  return "双方都修改了此条目";
}
