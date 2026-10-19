import { createSignal, onMount, Show } from "solid-js";
import { api } from "./lib/api";
import Vault from "./components/Vault";
import { FingerprintIcon } from "./components/icons";
import "./App.css";
import "./styles/vault.css";

type Mode = "loading" | "create" | "unlock" | "unlocked";

function App() {
  const [mode, setMode] = createSignal<Mode>("loading");
  const [pw, setPw] = createSignal("");
  const [pw2, setPw2] = createSignal("");
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  const [touchReady, setTouchReady] = createSignal(false);

  onMount(async () => {
    try {
      const exists = await api.vaultExists();
      setMode(exists ? "unlock" : "create");
      if (exists) {
        // 仅当设备支持生物识别且已存入受保护主密码时，才显示 Touch ID 解锁入口。
        const [avail, enabled] = await Promise.all([
          api.touchIdAvailable().catch(() => false),
          api.touchIdEnabled().catch(() => false),
        ]);
        setTouchReady(avail && enabled);
      }
    } catch {
      setError("无法访问应用数据目录");
      setMode("unlock");
    }
  });

  // 主密码强度粗估（0-4）
  function strength(p: string): number {
    let s = 0;
    if (p.length >= 8) s++;
    if (p.length >= 12) s++;
    if (/[a-z]/.test(p) && /[A-Z]/.test(p)) s++;
    if (/\d/.test(p) && /[^A-Za-z0-9]/.test(p)) s++;
    return s;
  }
  const strengthLabel = () => ["太弱", "偏弱", "一般", "良好", "很强"][strength(pw())];

  async function submit() {
    setError("");
    if (busy()) return;
    if (mode() === "create") {
      if (pw().length < 8) return setError("主密码至少 8 位");
      if (pw() !== pw2()) return setError("两次输入不一致");
    } else if (!pw()) {
      return setError("请输入主密码");
    }
    setBusy(true);
    try {
      if (mode() === "create") await api.createVault(pw());
      else await api.unlock(pw());
      setPw("");
      setPw2("");
      setMode("unlocked");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  async function lock() {
    await api.lock();
    setMode("unlock");
    setError("");
  }

  async function unlockTouch() {
    if (busy()) return;
    setError("");
    setBusy(true);
    try {
      await api.unlockWithTouchId();
      setMode("unlocked");
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Show
      when={mode() === "unlocked"}
      fallback={
        <main class="splash">
          <div class="ambient" />
          <div class="grid-overlay" />

          <div class="splash-card">
            <div class="shield">
              <svg viewBox="0 0 24 24" fill="none" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                <path d="M9 12l2 2 4-4" />
              </svg>
            </div>
            <h1 class="brand">
              Senti<span>nel</span>
            </h1>
            <p class="tagline">你的密钥，寸步不让</p>

            <Show when={mode() === "loading"}>
              <p class="milestone">正在检测保险库…</p>
            </Show>

            <Show when={mode() === "create" || mode() === "unlock"}>
              <div class="form">
                <p class="form-hint">
                  {mode() === "create"
                    ? "首次使用 · 设置主密码（唯一钥匙，无法找回）"
                    : "输入主密码解锁保险库"}
                </p>
                <input
                  class="field"
                  type="password"
                  placeholder="主密码"
                  value={pw()}
                  onInput={(e) => setPw(e.currentTarget.value)}
                  onKeyUp={(e) => e.key === "Enter" && submit()}
                />
                <Show when={mode() === "create"}>
                  <div class="strength">
                    <i style={{ width: `${(strength(pw()) / 4) * 100}%` }} />
                  </div>
                  <div class="strength-label">
                    强度：{strengthLabel()} · 建议 12 位以上，混合大小写、数字、符号
                  </div>
                  <input
                    class="field"
                    type="password"
                    placeholder="再次输入确认"
                    value={pw2()}
                    onInput={(e) => setPw2(e.currentTarget.value)}
                    onKeyUp={(e) => e.key === "Enter" && submit()}
                  />
                </Show>
                <Show when={error()}>
                  <div class="error">{error()}</div>
                </Show>
                <button class="btn-primary" onClick={submit} disabled={busy()}>
                  {busy() ? "处理中…" : mode() === "create" ? "创建保险库" : "解锁"}
                </button>
                <Show when={mode() === "unlock" && touchReady()}>
                  <button class="btn-touch" onClick={unlockTouch} disabled={busy()}>
                    <FingerprintIcon />
                    <span>用 Touch ID 解锁</span>
                  </button>
                </Show>
              </div>
            </Show>

            <p class="foot-note">Sentinel · Tauri 2 + SolidJS</p>
          </div>
        </main>
      }
    >
      <Vault onLock={lock} />
    </Show>
  );
}

export default App;
