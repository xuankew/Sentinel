import { Show, For } from "solid-js";
import { Item, avatarColor, initials, subtitle, kindLabel } from "../lib/api";
import { SearchIcon, PlusIcon, StarIcon, SyncIcon } from "./icons";

interface Props {
  items: Item[];
  query: string;
  setQuery: (q: string) => void;
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreate: () => void;
  syncing: boolean;
  syncConfigured: boolean;
  onSync: () => void;
  onOpenSyncSettings: () => void;
  registerSearch?: (el: HTMLInputElement) => void;
}

export default function ItemList(props: Props) {
  return (
    <div class="list-col">
      <div class="search-wrap">
        <div class="search">
          <SearchIcon size={16} />
          <input
            ref={(el) => props.registerSearch?.(el)}
            placeholder="搜索标题、用户名、网址…"
            value={props.query}
            onInput={(e) => props.setQuery(e.currentTarget.value)}
          />
          <Show when={props.query} fallback={<span class="kbd search-kbd">⌘K</span>}>
            <button class="kbd" style={{ border: "0", cursor: "pointer" }} onClick={() => props.setQuery("")}>
              ✕
            </button>
          </Show>
        </div>
      </div>

      <div class="list-head">
        <span>{props.items.length} 条</span>
        <div style={{ display: "flex", gap: "8px", "align-items": "center" }}>
          <button
            class="mini-btn"
            classList={{ spinning: props.syncing }}
            title={props.syncConfigured ? "立即同步" : "配置 GitHub 同步"}
            disabled={props.syncing}
            onClick={() => (props.syncConfigured ? props.onSync() : props.onOpenSyncSettings())}
          >
            <SyncIcon size={15} />
          </button>
          <button class="new-btn" onClick={props.onCreate}>
            <PlusIcon size={14} /> 新建
          </button>
        </div>
      </div>

      <div class="items">
        <Show
          when={props.items.length > 0}
          fallback={
            <div class="empty-list">
              {props.query ? "没有匹配的条目" : "还没有条目\n点击右上角「新建」添加第一条"}
            </div>
          }
        >
          <For each={props.items}>
            {(it) => (
              <button
                class="item-row"
                classList={{ active: it.id === props.selectedId }}
                onClick={() => props.onSelect(it.id)}
              >
                <div class="avatar" style={{ background: avatarColor(it.id) }}>
                  {initials(it.title)}
                </div>
                <div class="meta">
                  <div class="t">{it.title}</div>
                  <div class="s">
                    {subtitle(it)} · {kindLabel(it.kind)}
                  </div>
                </div>
                <Show when={it.favorite}>
                  <span class="star on">
                    <StarIcon size={15} filled />
                  </span>
                </Show>
              </button>
            )}
          </For>
        </Show>
      </div>
    </div>
  );
}
