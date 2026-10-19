import { createSignal, createMemo, onMount, onCleanup, Show } from "solid-js";
import { api, Item, ItemDraft, MergeConflict, Settings } from "../lib/api";
import { applyAppearance } from "../lib/theme";
import Sidebar, { Filter } from "./Sidebar";
import ItemList from "./ItemList";
import ItemDetail from "./ItemDetail";
import ItemEditor from "./ItemEditor";
import SyncSettings from "./SyncSettings";
import ConflictResolver from "./ConflictResolver";
import SettingsPanel from "./SettingsPanel";

interface Props {
  onLock: () => void;
}

export default function Vault(props: Props) {
  const [items, setItems] = createSignal<Item[]>([]);
  const [filter, setFilter] = createSignal<Filter>("all");
  const [query, setQuery] = createSignal("");
  const [selectedId, setSelectedId] = createSignal<string | null>(null);
  const [editorOpen, setEditorOpen] = createSignal(false);
  const [editing, setEditing] = createSignal<Item | null>(null);
  const [confirmDel, setConfirmDel] = createSignal<Item | null>(null);
  const [syncOpen, setSyncOpen] = createSignal(false);
  const [settingsOpen, setSettingsOpen] = createSignal(false);
  const [conflicts, setConflicts] = createSignal<MergeConflict[] | null>(null);
  const [syncConfigured, setSyncConfigured] = createSignal(false);
  const [syncing, setSyncing] = createSignal(false);
  const [syncInterval, setSyncInterval] = createSignal(0);
  const [lastSyncAt, setLastSyncAt] = createSignal<number | null>(null);
  const [settings, setSettings] = createSignal<Settings | null>(null);
  const [toast, setToast] = createSignal("");
  let toastTimer: ReturnType<typeof setTimeout> | undefined;

  function showToast(msg: string) {
    setToast(msg);
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => setToast(""), 2200);
  }

  async function reload(preferId?: string | null) {
    const list = await api.listItems();
    setItems(list);
    const target = preferId ?? selectedId();
    setSelectedId(target && list.some((i) => i.id === target) ? target : list[0]?.id ?? null);
  }

  async function refreshSyncState() {
    try {
      const c = await api.getSyncConfig();
      const ok = c.has_token && !!c.owner && !!c.repo;
      setSyncConfigured(ok);
      setSyncInterval(ok ? c.auto_sync_minutes || 0 : 0);
      setLastSyncAt(ok ? c.last_sync_at ?? null : null);
    } catch {
      setSyncConfigured(false);
      setSyncInterval(0);
      setLastSyncAt(null);
    }
    armAutoSync();
  }

  // ---- 同步：手动按钮与自动同步共用同一入口 ----
  async function doSync(auto = false) {
    if (syncing() || conflicts() || !syncConfigured()) return;
    setSyncing(true);
    try {
      const res = await api.syncNow();
      if (res.status === "conflict") {
        setConflicts(res.conflicts as MergeConflict[]);
        if (auto) showToast("自动同步遇到冲突，待裁决");
      } else if (res.status === "up_to_date") {
        if (!auto) showToast("已是最新，无需同步");
        refreshSyncState();
      } else {
        await reload();
        showToast(auto ? "自动同步完成" : `同步完成 · 共 ${res.total} 条`);
        refreshSyncState();
      }
    } catch (e) {
      if (!auto) showToast(`同步失败：${String(e)}`);
    } finally {
      setSyncing(false);
    }
  }

  // ---- 自动同步：按 sync.json 配置的间隔定时触发 ----
  let autoSyncTimer: ReturnType<typeof setInterval> | undefined;
  function armAutoSync() {
    clearInterval(autoSyncTimer);
    const mins = syncInterval();
    if (mins <= 0) return;
    autoSyncTimer = setInterval(() => {
      // 编辑/裁决/同步进行中不打断，跳过本轮
      if (editorOpen() || conflicts() || syncing()) return;
      doSync(true);
    }, mins * 60_000);
  }

  // ---- 自动锁定：监听用户活动，超过设定分钟数无操作则锁定 ----
  let idleTimer: ReturnType<typeof setTimeout> | undefined;
  let lastActivity = Date.now();
  const ACTIVITY_EVENTS: (keyof WindowEventMap)[] = [
    "mousedown",
    "keydown",
    "wheel",
    "touchstart",
  ];
  function bumpActivity() {
    lastActivity = Date.now();
  }
  function armIdle() {
    clearTimeout(idleTimer);
    const mins = settings()?.auto_lock_minutes ?? 0;
    if (mins <= 0) return; // 0 = 永不自动锁定
    idleTimer = setTimeout(() => {
      // 到点再核对一次，避免期间有活动被漏记
      if (Date.now() - lastActivity >= mins * 60_000) {
        props.onLock();
      } else {
        armIdle();
      }
    }, mins * 60_000);
  }

  onCleanup(() => {
    clearTimeout(idleTimer);
    clearInterval(autoSyncTimer);
    ACTIVITY_EVENTS.forEach((e) => window.removeEventListener(e, bumpActivity));
    window.removeEventListener("keydown", onGlobalKey);
  });

  // ---- 全局键盘快捷键 ----
  let searchEl: HTMLInputElement | undefined;

  // 选中条目的「主密钥」：登录→密码，API Key→Secret，笔记无主密钥。
  function primarySecret(it: Item | null): { value: string; label: string } | null {
    if (!it) return null;
    if (it.kind.type === "login" && it.kind.password) return { value: it.kind.password, label: "密码" };
    if (it.kind.type === "api_key" && it.kind.secret) return { value: it.kind.secret, label: "Secret" };
    return null;
  }

  function anyModalOpen() {
    return editorOpen() || confirmDel() !== null || conflicts() !== null || syncOpen() || settingsOpen();
  }
  function inEditable() {
    const el = document.activeElement as HTMLElement | null;
    if (!el) return false;
    const tag = el.tagName;
    return tag === "INPUT" || tag === "TEXTAREA" || el.isContentEditable;
  }
  function hasTextSelection() {
    const sel = window.getSelection();
    return !!sel && sel.toString().length > 0;
  }

  async function onGlobalKey(e: KeyboardEvent) {
    // Esc：关闭最上层弹层；无弹层时清空搜索。
    if (e.key === "Escape") {
      if (editorOpen()) {
        setEditorOpen(false);
      } else if (confirmDel()) {
        setConfirmDel(null);
      } else if (conflicts()) {
        setConflicts(null);
      } else if (settingsOpen()) {
        setSettingsOpen(false);
      } else if (syncOpen()) {
        setSyncOpen(false);
        refreshSyncState();
      } else if (query()) {
        setQuery("");
      } else {
        return;
      }
      e.preventDefault();
      return;
    }

    if (!(e.metaKey || e.ctrlKey)) return;
    const key = e.key.toLowerCase();

    // ⌘L 立即锁定：任何情况下都生效。
    if (key === "l") {
      e.preventDefault();
      props.onLock();
      return;
    }

    // 其余快捷键在弹层打开时不抢占。
    if (anyModalOpen()) return;

    if (key === "k" || key === "f") {
      e.preventDefault();
      searchEl?.focus();
      searchEl?.select();
      return;
    }
    if (key === "n") {
      e.preventDefault();
      openCreate();
      return;
    }
    if (key === "c") {
      // 有文本选区或焦点在输入控件时，放行原生复制。
      if (inEditable() || hasTextSelection()) return;
      const sec = primarySecret(selected());
      if (!sec) return;
      e.preventDefault();
      const secs = await api.copySecret(sec.value);
      showToast(
        secs > 0 ? `已复制${sec.label}，${secs} 秒后自动清除` : `已复制${sec.label}（按设置不自动清除）`
      );
    }
  }

  onMount(async () => {
    reload();
    refreshSyncState();
    try {
      const s = await api.getSettings();
      setSettings(s);
      applyAppearance(s.accent, s.density);
    } catch {
      /* 用默认外观 */
    }
    ACTIVITY_EVENTS.forEach((e) => window.addEventListener(e, bumpActivity, { passive: true }));
    window.addEventListener("keydown", onGlobalKey);
    armIdle();
  });


  function onSettingsSaved(s: Settings) {
    setSettings(s);
    applyAppearance(s.accent, s.density);
    armIdle();
  }

  const counts = createMemo(() => {
    const all = items();
    return {
      all: all.length,
      favorite: all.filter((i) => i.favorite).length,
      login: all.filter((i) => i.kind.type === "login").length,
      api_key: all.filter((i) => i.kind.type === "api_key").length,
      note: all.filter((i) => i.kind.type === "note").length,
    } as Record<Filter, number>;
  });

  const visible = createMemo(() => {
    const q = query().trim().toLowerCase();
    const f = filter();
    return items()
      .filter((i) => {
        if (f === "favorite") return i.favorite;
        if (f !== "all") return i.kind.type === f;
        return true;
      })
      .filter((i) => {
        if (!q) return true;
        const hay = [
          i.title,
          ...i.tags,
          i.kind.type === "login" ? i.kind.username + " " + i.kind.url + " " + i.kind.notes : "",
          i.kind.type === "api_key" ? i.kind.key_name + " " + i.kind.url + " " + i.kind.notes : "",
          i.kind.type === "note" ? i.kind.body : "",
        ]
          .join(" ")
          .toLowerCase();
        return hay.includes(q);
      })
      .sort((a, b) => {
        if (a.favorite !== b.favorite) return a.favorite ? -1 : 1;
        return b.updated_at - a.updated_at;
      });
  });

  const selected = createMemo(() => items().find((i) => i.id === selectedId()) ?? null);

  function openCreate() {
    setEditing(null);
    setEditorOpen(true);
  }
  function openEdit(it: Item) {
    setEditing(it);
    setEditorOpen(true);
  }

  async function saveDraft(draft: ItemDraft) {
    const cur = editing();
    const saved = cur ? await api.updateItem(cur.id, draft) : await api.createItem(draft);
    setEditorOpen(false);
    setEditing(null);
    await reload(saved.id);
    showToast(cur ? "条目已更新" : "条目已创建");
  }

  async function doDelete(it: Item) {
    await api.deleteItem(it.id);
    setConfirmDel(null);
    await reload(null);
    showToast("条目已删除");
  }

  return (
    <div class="vault">
      <Sidebar
        filter={filter()}
        setFilter={setFilter}
        counts={counts()}
        onLock={props.onLock}
        syncConfigured={syncConfigured()}
        lastSyncAt={lastSyncAt()}
        onOpenSync={() => setSyncOpen(true)}
        onOpenSettings={() => setSettingsOpen(true)}
      />

      <ItemList
        items={visible()}
        query={query()}
        setQuery={setQuery}
        selectedId={selectedId()}
        onSelect={setSelectedId}
        onCreate={openCreate}
        syncing={syncing()}
        syncConfigured={syncConfigured()}
        onSync={() => doSync(false)}
        onOpenSyncSettings={() => setSyncOpen(true)}
        registerSearch={(el) => (searchEl = el)}
      />

      <ItemDetail item={selected()} onEdit={openEdit} onDelete={setConfirmDel} onToast={showToast} />

      <Show when={editorOpen()}>
        <ItemEditor initial={editing()} onSave={saveDraft} onClose={() => setEditorOpen(false)} />
      </Show>

      <Show when={confirmDel()}>
        {(it) => (
          <div class="modal-backdrop" onClick={(e) => e.target === e.currentTarget && setConfirmDel(null)}>
            <div class="modal" style={{ width: "380px" }}>
              <div class="modal-head">
                <h3>删除条目</h3>
              </div>
              <div class="modal-body">
                <p style={{ "font-size": "13.5px", color: "var(--text-dim)", "line-height": "1.6" }}>
                  确定要删除「{it().title}」吗？此操作不可撤销。
                </p>
              </div>
              <div class="modal-foot">
                <button class="btn btn-ghost" onClick={() => setConfirmDel(null)}>
                  取消
                </button>
                <button
                  class="btn"
                  style={{ background: "var(--danger)", color: "#fff" }}
                  onClick={() => doDelete(it())}
                >
                  删除
                </button>
              </div>
            </div>
          </div>
        )}
      </Show>

      <Show when={syncOpen()}>
        <SyncSettings
          onClose={() => {
            setSyncOpen(false);
            refreshSyncState();
          }}
          onSynced={(msg) => {
            showToast(msg);
            reload();
          }}
          onConflict={(cs) => setConflicts(cs as MergeConflict[])}
        />
      </Show>

      <Show when={conflicts()}>
        {(cs) => (
          <ConflictResolver
            conflicts={cs()}
            onDone={(msg) => {
              setConflicts(null);
              showToast(msg);
              reload();
            }}
            onCancel={() => setConflicts(null)}
          />
        )}
      </Show>

      <Show when={settingsOpen()}>
        <SettingsPanel
          onClose={() => setSettingsOpen(false)}
          onToast={showToast}
          onSettings={onSettingsSaved}
          onChanged={() => reload()}
        />
      </Show>

      <Show when={toast()}>
        <div class="toast">
          <span class="dot-ok" />
          {toast()}
        </div>
      </Show>
    </div>
  );
}
