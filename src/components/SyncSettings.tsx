import { createSignal, onMount, Show, For } from "solid-js";
import { api } from "../lib/api";
import { XIcon, SyncIcon, CheckIcon, WarnIcon } from "./icons";

interface Props {
  onClose: () => void;
  onSynced: (msg: string) => void;
  onConflict: (conflicts: any[]) => void;
}

type TestState = { kind: "idle" } | { kind: "busy" } | { kind: "ok"; msg: string } | { kind: "err"; msg: string };

const SYNC_INTERVALS = [0, 5, 15, 30, 60];

export default function SyncSettings(props: Props) {
  const [owner, setOwner] = createSignal("");
  const [repo, setRepo] = createSignal("");
  const [branch, setBranch] = createSignal("main");
  const [path, setPath] = createSignal("vault.sentinel");
  const [token, setToken] = createSignal("");
  const [hasToken, setHasToken] = createSignal(false);
  const [autoSync, setAutoSync] = createSignal(0);
  const [saving, setSaving] = createSignal(false);
  const [syncing, setSyncing] = createSignal(false);
  const [test, setTest] = createSignal<TestState>({ kind: "idle" });
  const [error, setError] = createSignal("");

  onMount(async () => {
    try {
      const c = await api.getSyncConfig();
      setOwner(c.owner);
      setRepo(c.repo);
      setBranch(c.branch || "main");
      setPath(c.path || "vault.sentinel");
      setHasToken(c.has_token);
      setAutoSync(c.auto_sync_minutes || 0);
    } catch (e) {
      setError(String(e));
    }
  });

  async function save(): Promise<boolean> {
    setError("");
    setSaving(true);
    try {
      await api.saveSyncConfig({
        owner: owner(),
        repo: repo(),
        branch: branch(),
        path: path(),
        // 空字符串 = 不改动已存 token（用户没填新值时）
        token: token().trim() ? token().trim() : null,
        auto_sync_minutes: autoSync(),
      });
      if (token().trim()) {
        setHasToken(true);
        setToken("");
      }
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      setSaving(false);
    }
  }

  async function testConnection() {
    setTest({ kind: "busy" });
    if (!(await save())) {
      setTest({ kind: "err", msg: "保存失败" });
      return;
    }
    try {
      const full = await api.testSyncConnection();
      setTest({ kind: "ok", msg: `已连接 ${full}` });
    } catch (e) {
      setTest({ kind: "err", msg: String(e) });
    }
  }

  async function syncNow() {
    setError("");
    if (!(await save())) return;
    setSyncing(true);
    try {
      const res = await api.syncNow();
      if (res.status === "conflict") {
        props.onConflict(res.conflicts);
        props.onClose();
      } else if (res.status === "up_to_date") {
        props.onSynced("已是最新，无需同步");
        props.onClose();
      } else {
        props.onSynced(`同步完成 · 共 ${res.total} 条`);
        props.onClose();
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setSyncing(false);
    }
  }

  return (
    <div class="modal-backdrop" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="modal">
        <div class="modal-head">
          <h3>
            <span style={{ display: "inline-flex", "vertical-align": "-3px", "margin-right": "8px", color: "var(--accent)" }}>
              <SyncIcon size={18} />
            </span>
            GitHub 同步
          </h3>
          <button class="icon-btn" onClick={props.onClose} title="关闭">
            <XIcon size={17} />
          </button>
        </div>

        <div class="modal-body">
          <p style={{ "font-size": "12.5px", color: "var(--text-dim)", "line-height": "1.6", "margin-bottom": "16px" }}>
            通过你自己的 GitHub 私有仓库在多台 Mac 间同步。云端只存<strong style={{ color: "var(--text)" }}>密文</strong>，
            Token 存入 macOS 钥匙串，绝不上传。
          </p>

          <div class="form-group">
            <label>仓库 Owner（用户名或组织）</label>
            <input class="input mono" value={owner()} placeholder="your-name" onInput={(e) => setOwner(e.currentTarget.value)} />
          </div>
          <div class="form-group">
            <label>仓库名</label>
            <input class="input mono" value={repo()} placeholder="my-vault" onInput={(e) => setRepo(e.currentTarget.value)} />
          </div>
          <div style={{ display: "flex", gap: "12px" }}>
            <div class="form-group" style={{ flex: "1" }}>
              <label>分支</label>
              <input class="input mono" value={branch()} placeholder="main" onInput={(e) => setBranch(e.currentTarget.value)} />
            </div>
            <div class="form-group" style={{ flex: "2" }}>
              <label>文件路径</label>
              <input class="input mono" value={path()} placeholder="vault.sentinel" onInput={(e) => setPath(e.currentTarget.value)} />
            </div>
          </div>

          <div class="form-group">
            <label>
              细粒度 Personal Access Token
              <Show when={hasToken()}>
                <span style={{ color: "var(--accent)", "margin-left": "8px" }}>· 已保存</span>
              </Show>
            </label>
            <input
              class="input mono"
              type="password"
              value={token()}
              placeholder={hasToken() ? "留空则保持不变" : "github_pat_…（需 contents 读写权限）"}
              onInput={(e) => setToken(e.currentTarget.value)}
            />
          </div>

          <div class="form-group">
            <label>自动同步</label>
            <div class="seg-row">
              <For each={SYNC_INTERVALS}>
                {(m) => (
                  <button class="seg-btn" classList={{ active: autoSync() === m }} onClick={() => setAutoSync(m)}>
                    {m === 0 ? "关闭" : `每 ${m} 分`}
                  </button>
                )}
              </For>
            </div>
            <p style={{ "font-size": "12px", color: "var(--text-dim)", "margin-top": "8px", "line-height": "1.5" }}>
              开启后按间隔自动拉取并推送；遇到冲突仍会弹出裁决界面。
            </p>
          </div>

          <Show when={test().kind === "ok"}>
            <div style={{ display: "flex", "align-items": "center", gap: "8px", "font-size": "12.5px", color: "var(--accent)", "margin-bottom": "14px" }}>
              <CheckIcon size={15} /> {(test() as any).msg}
            </div>
          </Show>
          <Show when={test().kind === "err"}>
            <div style={{ display: "flex", "align-items": "center", gap: "8px", "font-size": "12.5px", color: "var(--danger)", "margin-bottom": "14px" }}>
              <WarnIcon size={15} /> {(test() as any).msg}
            </div>
          </Show>
          <Show when={error()}>
            <div class="form-error">{error()}</div>
          </Show>
        </div>

        <div class="modal-foot" style={{ "justify-content": "space-between" }}>
          <button class="btn btn-ghost" onClick={testConnection} disabled={test().kind === "busy" || saving() || syncing()}>
            {test().kind === "busy" ? "测试中…" : "测试连接"}
          </button>
          <div style={{ display: "flex", gap: "10px" }}>
            <button class="btn btn-ghost" onClick={props.onClose} disabled={saving() || syncing()}>
              关闭
            </button>
            <button class="btn btn-primary" onClick={syncNow} disabled={syncing() || saving()}>
              {syncing() ? "同步中…" : "立即同步"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
