import { For, Show } from "solid-js";
import { ShieldIcon, GridIcon, StarIcon, KeyIcon, NoteIcon, LockIcon, SyncIcon, GearIcon, TagIcon } from "./icons";

export type Filter = "all" | "favorite" | "login" | "api_key" | "note";

interface Props {
  filter: Filter;
  setFilter: (f: Filter) => void;
  counts: Record<Filter, number>;
  tags: { name: string; count: number }[];
  selectedTag: string | null;
  onSelectTag: (t: string) => void;
  onLock: () => void;
  syncConfigured: boolean;
  lastSyncAt: number | null;
  onOpenSync: () => void;
  onOpenSettings: () => void;
}

function fmtSyncTime(ts: number): string {
  const d = new Date(ts * 1000);
  const now = new Date();
  const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  if (d.toDateString() === now.toDateString()) return `今天 ${hm}`;
  return `${d.getMonth() + 1}/${d.getDate()} ${hm}`;
}

const NAV: { key: Filter; label: string; icon: any }[] = [
  { key: "all", label: "全部条目", icon: GridIcon },
  { key: "favorite", label: "收藏", icon: StarIcon },
];
const TYPES: { key: Filter; label: string; icon: any }[] = [
  { key: "login", label: "登录密码", icon: KeyIcon },
  { key: "api_key", label: "API Key", icon: ShieldIcon },
  { key: "note", label: "安全笔记", icon: NoteIcon },
];

function NavBtn(props: { item: (typeof NAV)[number]; active: boolean; count: number; onClick: () => void }) {
  const Icon = props.item.icon;
  return (
    <button
      class="nav-item"
      classList={{ active: props.active }}
      onClick={props.onClick}
    >
      <span class="nic">
        <Icon size={17} filled={props.item.key === "favorite" && props.active} />
      </span>
      {props.item.label}
      <Show when={props.count > 0}>
        <span class="count">{props.count}</span>
      </Show>
    </button>
  );
}

export default function Sidebar(props: Props) {
  return (
    <aside class="sidebar">
      <div class="side-brand">
        <div class="side-logo">
          <ShieldIcon size={17} />
        </div>
        <b>
          Senti<span style={{ color: "var(--accent)" }}>nel</span>
        </b>
      </div>

      <div class="nav-label">视图</div>
      {NAV.map((item) => (
        <NavBtn
          item={item}
          active={!props.selectedTag && props.filter === item.key}
          count={props.counts[item.key]}
          onClick={() => props.setFilter(item.key)}
        />
      ))}

      <div class="nav-label">类型</div>
      {TYPES.map((item) => (
        <NavBtn
          item={item}
          active={!props.selectedTag && props.filter === item.key}
          count={props.counts[item.key]}
          onClick={() => props.setFilter(item.key)}
        />
      ))}

      <Show when={props.tags.length > 0}>
        <div class="nav-label">标签</div>
        <div class="tag-list">
          <For each={props.tags}>
            {(t) => (
              <button
                class="nav-item"
                classList={{ active: props.selectedTag === t.name }}
                title={t.name}
                onClick={() => props.onSelectTag(t.name)}
              >
                <span class="nic">
                  <TagIcon size={16} />
                </span>
                <span class="tag-name">{t.name}</span>
                <span class="count">{t.count}</span>
              </button>
            )}
          </For>
        </div>
      </Show>

      <div class="side-foot">
        <button
          class="nav-item sync-entry"
          classList={{ "sync-configured": props.syncConfigured }}
          onClick={props.onOpenSync}
        >
          <span class="nic">
            <SyncIcon size={17} />
          </span>
          <span class="sync-text">
            <span>{props.syncConfigured ? "GitHub 同步" : "配置同步"}</span>
            <Show when={props.syncConfigured}>
              <span class="sync-sub">
                {props.lastSyncAt ? `上次同步 ${fmtSyncTime(props.lastSyncAt)}` : "尚未同步"}
              </span>
            </Show>
          </span>
        </button>
        <button class="nav-item" onClick={props.onOpenSettings}>
          <span class="nic">
            <GearIcon size={17} />
          </span>
          设置
        </button>
        <button class="nav-item" onClick={props.onLock}>
          <span class="nic">
            <LockIcon size={17} />
          </span>
          锁定保险库
        </button>
      </div>
    </aside>
  );
}
