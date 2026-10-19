import { createSignal, onMount, Show, For } from "solid-js";
import { api, Settings, Accent, Density, ImportSummary } from "../lib/api";
import { applyAppearance } from "../lib/theme";
import { XIcon, GearIcon, DownloadIcon, UploadIcon, FingerprintIcon, CheckIcon, KeyIcon } from "./icons";

interface Props {
  onClose: () => void;
  onToast: (msg: string) => void;
  onSettings: (s: Settings) => void;
  onChanged?: () => void;
}

const ACCENTS: { key: Accent; label: string; swatch: string }[] = [
  { key: "teal", label: "青", swatch: "linear-gradient(135deg,#4fd1c5,#3b82f6)" },
  { key: "blue", label: "蓝", swatch: "linear-gradient(135deg,#60a5fa,#818cf8)" },
  { key: "violet", label: "紫", swatch: "linear-gradient(135deg,#a78bfa,#c084fc)" },
  { key: "amber", label: "琥珀", swatch: "linear-gradient(135deg,#fbbf24,#f59e0b)" },
  { key: "rose", label: "玫红", swatch: "linear-gradient(135deg,#fb7185,#f472b6)" },
  { key: "green", label: "绿", swatch: "linear-gradient(135deg,#4ade80,#22d3ee)" },
];

const DENSITIES: { key: Density; label: string }[] = [
  { key: "comfortable", label: "舒适" },
  { key: "compact", label: "紧凑" },
];

const LOCK_OPTIONS = [0, 1, 5, 15, 30, 60];
const CLIPBOARD_OPTIONS = [15, 30, 60, 0];

export default function SettingsPanel(props: Props) {
  const [settings, setSettings] = createSignal<Settings | null>(null);
  const [error, setError] = createSignal("");
  const [saving, setSaving] = createSignal(false);

  // 导出
  const [exportOpen, setExportOpen] = createSignal(false);
  const [exportPw, setExportPw] = createSignal("");
  const [exportBusy, setExportBusy] = createSignal(false);
  const [exportErr, setExportErr] = createSignal("");

  // 导入（CSV）
  const [importBusy, setImportBusy] = createSignal(false);
  const [importErr, setImportErr] = createSignal("");
  const [importResult, setImportResult] = createSignal<ImportSummary | null>(null);

  // Touch ID
  const [touchAvailable, setTouchAvailable] = createSignal(false);
  const [touchOn, setTouchOn] = createSignal(false);
  const [touchPw, setTouchPw] = createSignal("");
  const [touchAsk, setTouchAsk] = createSignal(false);
  const [touchBusy, setTouchBusy] = createSignal(false);
  const [touchErr, setTouchErr] = createSignal("");

  // 修改主密码
  const [pwOpen, setPwOpen] = createSignal(false);
  const [pwOld, setPwOld] = createSignal("");
  const [pwNew, setPwNew] = createSignal("");
  const [pwConfirm, setPwConfirm] = createSignal("");
  const [pwBusy, setPwBusy] = createSignal(false);
  const [pwErr, setPwErr] = createSignal("");

  // 恢复提示
  const [hintVal, setHintVal] = createSignal("");
  const [hintBusy, setHintBusy] = createSignal(false);

  onMount(async () => {
    try {
      const s = await api.getSettings();
      setSettings(s);
      // 以钥匙串实际状态为准（而非 settings.json 标记），避免与解锁页显示不一致。
      const [avail, enabled, hint] = await Promise.all([
        api.touchIdAvailable().catch(() => false),
        api.touchIdEnabled().catch(() => false),
        api.getRecoveryHint().catch(() => ""),
      ]);
      setTouchAvailable(avail);
      setTouchOn(enabled);
      setHintVal(hint);
    } catch (e) {
      setError(String(e));
    }
  });

  function patch(p: Partial<Settings>) {
    setSettings((s) => (s ? { ...s, ...p } : s));
  }

  // 外观即时预览（不等待保存）
  function pickAccent(a: Accent) {
    patch({ accent: a });
    applyAppearance(a, settings()?.density ?? "comfortable");
  }
  function pickDensity(d: Density) {
    patch({ density: d });
    applyAppearance(settings()?.accent ?? "teal", d);
  }

  async function save() {
    const s = settings();
    if (!s) return;
    setError("");
    setSaving(true);
    try {
      const saved = await api.saveSettings(s);
      setSettings(saved);
      applyAppearance(saved.accent, saved.density);
      props.onSettings(saved);
      props.onToast("设置已保存");
      props.onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setSaving(false);
    }
  }

  async function doExport() {
    if (!exportPw()) return setExportErr("请输入导出密码");
    setExportErr("");
    setExportBusy(true);
    try {
      const path = await api.exportVault(exportPw());
      setExportBusy(false);
      if (path) {
        setExportOpen(false);
        setExportPw("");
        props.onToast("已加密导出");
      }
    } catch (e) {
      setExportErr(String(e));
      setExportBusy(false);
    }
  }

  async function doImport() {
    setImportErr("");
    setImportResult(null);
    setImportBusy(true);
    try {
      const summary = await api.importCsv();
      if (summary) {
        setImportResult(summary);
        if (summary.imported > 0) props.onChanged?.();
        props.onToast(
          summary.imported > 0
            ? `已导入 ${summary.imported} 条`
            : "没有新条目被导入",
        );
      }
      // summary 为 null = 用户取消了文件选择，静默不提示
    } catch (e) {
      setImportErr(String(e));
    } finally {
      setImportBusy(false);
    }
  }

  async function toggleTouch() {
    setTouchErr("");
    if (touchOn()) {
      // 关闭
      setTouchBusy(true);
      try {
        await api.disableTouchId();
        setTouchOn(false);
        patch({ touch_id_enabled: false });
        props.onToast("已关闭 Touch ID");
      } catch (e) {
        setTouchErr(String(e));
      } finally {
        setTouchBusy(false);
      }
      return;
    }
    // 开启需要主密码写入钥匙串
    setTouchAsk(true);
  }

  async function confirmEnableTouch() {
    if (!touchPw()) return setTouchErr("请输入主密码");
    setTouchErr("");
    setTouchBusy(true);
    try {
      await api.enableTouchId(touchPw());
      setTouchOn(true);
      setTouchAsk(false);
      setTouchPw("");
      patch({ touch_id_enabled: true });
      props.onToast("已开启 Touch ID 解锁");
    } catch (e) {
      setTouchErr(String(e));
    } finally {
      setTouchBusy(false);
    }
  }

  function openPwModal() {
    setPwOld("");
    setPwNew("");
    setPwConfirm("");
    setPwErr("");
    setPwOpen(true);
  }

  async function doChangePassword() {
    const o = pwOld();
    const n = pwNew();
    if (!o) return setPwErr("请输入原主密码");
    if (n.length < 8) return setPwErr("新主密码至少 8 位");
    if (n === o) return setPwErr("新主密码不能与原密码相同");
    if (n !== pwConfirm()) return setPwErr("两次输入的新主密码不一致");
    setPwErr("");
    setPwBusy(true);
    try {
      await api.changeMasterPassword(o, n);
      setPwOpen(false);
      setPwOld("");
      setPwNew("");
      setPwConfirm("");
      props.onToast("主密码已修改");
    } catch (e) {
      setPwErr(String(e));
    } finally {
      setPwBusy(false);
    }
  }

  async function saveHint() {
    setHintBusy(true);
    try {
      await api.setRecoveryHint(hintVal());
      props.onToast("恢复提示已保存");
    } catch (e) {
      setError(String(e));
    } finally {
      setHintBusy(false);
    }
  }

  return (
    <div class="modal-backdrop" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="modal">
        <div class="modal-head">
          <h3>
            <span style={{ display: "inline-flex", "vertical-align": "-3px", "margin-right": "8px", color: "var(--accent)" }}>
              <GearIcon size={18} />
            </span>
            设置
          </h3>
          <button class="icon-btn" onClick={props.onClose} title="关闭">
            <XIcon size={17} />
          </button>
        </div>

        <div class="modal-body">
          <Show when={settings()}>
            {(s) => (
              <>
                {/* 外观：强调色 */}
                <div class="form-group">
                  <label>强调色</label>
                  <div class="accent-row">
                    <For each={ACCENTS}>
                      {(a) => (
                        <button
                          class="accent-swatch"
                          classList={{ active: s().accent === a.key }}
                          style={{ background: a.swatch }}
                          title={a.label}
                          onClick={() => pickAccent(a.key)}
                        >
                          <Show when={s().accent === a.key}>
                            <CheckIcon size={15} />
                          </Show>
                        </button>
                      )}
                    </For>
                  </div>
                </div>

                {/* 外观：密度 */}
                <div class="form-group">
                  <label>界面密度</label>
                  <div class="seg-row">
                    <For each={DENSITIES}>
                      {(d) => (
                        <button
                          class="seg-btn"
                          classList={{ active: s().density === d.key }}
                          onClick={() => pickDensity(d.key)}
                        >
                          {d.label}
                        </button>
                      )}
                    </For>
                  </div>
                </div>

                {/* 自动锁定 */}
                <div class="form-group">
                  <label>自动锁定（无操作后）</label>
                  <div class="seg-row">
                    <For each={LOCK_OPTIONS}>
                      {(m) => (
                        <button
                          class="seg-btn"
                          classList={{ active: s().auto_lock_minutes === m }}
                          onClick={() => patch({ auto_lock_minutes: m })}
                        >
                          {m === 0 ? "永不" : `${m} 分`}
                        </button>
                      )}
                    </For>
                  </div>
                </div>

                {/* 剪贴板自动清除 */}
                <div class="form-group">
                  <label>剪贴板自动清除（复制敏感字段后）</label>
                  <div class="seg-row">
                    <For each={CLIPBOARD_OPTIONS}>
                      {(sec) => (
                        <button
                          class="seg-btn"
                          classList={{ active: s().clipboard_clear_secs === sec }}
                          onClick={() => patch({ clipboard_clear_secs: sec })}
                        >
                          {sec === 0 ? "从不" : `${sec} 秒`}
                        </button>
                      )}
                    </For>
                  </div>
                  <div class="form-sub">选「从不」将不再自动清除剪贴板，敏感内容可能被其他程序读取</div>
                </div>

                {/* Touch ID */}
                <div class="form-group">
                  <label>Touch ID 解锁</label>
                  <div class="setting-line">
                    <span class="sl-icon" style={{ color: touchOn() ? "var(--accent)" : "var(--text-faint)" }}>
                      <FingerprintIcon size={20} />
                    </span>
                    <div class="sl-text">
                      <div>用 Touch ID 解锁保险库</div>
                      <div class="sl-sub">
                        {touchAvailable()
                          ? "主密码将存入受生物识别保护的钥匙串项"
                          : "此设备未检测到可用的 Touch ID"}
                      </div>
                    </div>
                    <button
                      class="switch"
                      classList={{ on: touchOn() }}
                      disabled={!touchAvailable() || touchBusy()}
                      onClick={toggleTouch}
                      title={touchAvailable() ? "" : "设备不支持"}
                    >
                      <span class="knob" />
                    </button>
                  </div>

                  <Show when={touchAsk()}>
                    <div class="touch-ask">
                      <input
                        class="input"
                        type="password"
                        placeholder="输入主密码以启用"
                        value={touchPw()}
                        onInput={(e) => setTouchPw(e.currentTarget.value)}
                        onKeyUp={(e) => e.key === "Enter" && confirmEnableTouch()}
                      />
                      <div style={{ display: "flex", gap: "8px" }}>
                        <button class="btn btn-ghost" onClick={() => { setTouchAsk(false); setTouchPw(""); setTouchErr(""); }} disabled={touchBusy()}>
                          取消
                        </button>
                        <button class="btn btn-primary" onClick={confirmEnableTouch} disabled={touchBusy()}>
                          {touchBusy() ? "启用中…" : "启用"}
                        </button>
                      </div>
                    </div>
                  </Show>
                  <Show when={touchErr()}>
                    <div class="form-error" style={{ "margin-top": "10px" }}>{touchErr()}</div>
                  </Show>
                </div>

                {/* 修改主密码 */}
                <div class="form-group">
                  <label>安全</label>
                  <div class="setting-line">
                    <span class="sl-icon" style={{ color: "var(--text-faint)" }}>
                      <KeyIcon size={20} />
                    </span>
                    <div class="sl-text">
                      <div>修改主密码</div>
                      <div class="sl-sub">用新密码重新加密整个保险库；已启用 Touch ID 会一并更新</div>
                    </div>
                    <button class="btn btn-ghost" onClick={openPwModal}>
                      修改
                    </button>
                  </div>
                </div>

                {/* 恢复提示 */}
                <div class="form-group">
                  <label>恢复提示</label>
                  <div class="hint-edit">
                    <input
                      class="input"
                      type="text"
                      placeholder="一句帮你回忆主密码的话（可选）"
                      value={hintVal()}
                      onInput={(e) => setHintVal(e.currentTarget.value)}
                      onKeyUp={(e) => e.key === "Enter" && saveHint()}
                    />
                    <button class="btn btn-ghost" onClick={saveHint} disabled={hintBusy()}>
                      {hintBusy() ? "保存中…" : "保存"}
                    </button>
                  </div>
                  <div class="form-sub">
                    明文仅存本机，用于解锁页帮主人回忆主密码；不加密、不随同步上云
                  </div>
                </div>

                {/* 导出 */}
                <div class="form-group">
                  <label>数据导出</label>
                  <div class="setting-line">
                    <span class="sl-icon" style={{ color: "var(--text-faint)" }}>
                      <DownloadIcon size={20} />
                    </span>
                    <div class="sl-text">
                      <div>加密导出备份</div>
                      <div class="sl-sub">用独立导出密码重新加密为 .sentinel 文件，绝不含明文</div>
                    </div>
                    <button class="btn btn-ghost" onClick={() => { setExportOpen(true); setExportErr(""); }}>
                      导出
                    </button>
                  </div>
                </div>

                {/* 导入 */}
                <div class="form-group">
                  <label>数据导入</label>
                  <div class="setting-line">
                    <span class="sl-icon" style={{ color: "var(--text-faint)" }}>
                      <UploadIcon size={20} />
                    </span>
                    <div class="sl-text">
                      <div>从 CSV 导入</div>
                      <div class="sl-sub">支持 Chrome / 1Password / Bitwarden 等导出的 CSV，按表头自动识别列</div>
                    </div>
                    <button class="btn btn-ghost" onClick={doImport} disabled={importBusy()}>
                      {importBusy() ? "导入中…" : "选择文件"}
                    </button>
                  </div>
                  <div class="form-sub">
                    与库中已有条目完全重复的行会自动跳过；源 CSV 为明文，导入后请安全删除。全程本地解析入库，不上传、不保留副本。
                  </div>
                  <Show when={importErr()}>
                    <div class="form-error">{importErr()}</div>
                  </Show>
                  <Show when={importResult()}>
                    {(r) => (
                      <div class="form-sub">
                        已导入 {r().imported} 条
                        <Show when={r().skipped_duplicate > 0}>，跳过重复 {r().skipped_duplicate} 条</Show>
                        <Show when={r().skipped_invalid > 0}>，跳过无效 {r().skipped_invalid} 条</Show>
                      </div>
                    )}
                  </Show>
                </div>
              </>
            )}
          </Show>

          <Show when={error()}>
            <div class="form-error">{error()}</div>
          </Show>
        </div>

        <div class="modal-foot">
          <button class="btn btn-ghost" onClick={props.onClose}>
            取消
          </button>
          <button class="btn btn-primary" onClick={save} disabled={saving() || !settings()}>
            {saving() ? "保存中…" : "保存"}
          </button>
        </div>

        {/* 导出密码子弹层 */}
        <Show when={exportOpen()}>
          <div class="modal-backdrop" style={{ "z-index": "70" }} onClick={(e) => e.target === e.currentTarget && setExportOpen(false)}>
            <div class="modal" style={{ width: "380px" }}>
              <div class="modal-head">
                <h3>加密导出</h3>
              </div>
              <div class="modal-body">
                <p style={{ "font-size": "12.5px", color: "var(--text-dim)", "line-height": "1.6", "margin-bottom": "14px" }}>
                  设置一个<strong style={{ color: "var(--text)" }}>导出密码</strong>。导出的文件用它单独加密，
                  与主密码无关，可安全地存到别处。
                </p>
                <input
                  class="input"
                  type="password"
                  placeholder="导出密码"
                  value={exportPw()}
                  onInput={(e) => setExportPw(e.currentTarget.value)}
                  onKeyUp={(e) => e.key === "Enter" && doExport()}
                />
                <Show when={exportErr()}>
                  <div class="form-error" style={{ "margin-top": "12px" }}>{exportErr()}</div>
                </Show>
              </div>
              <div class="modal-foot">
                <button class="btn btn-ghost" onClick={() => setExportOpen(false)} disabled={exportBusy()}>
                  取消
                </button>
                <button class="btn btn-primary" onClick={doExport} disabled={exportBusy()}>
                  {exportBusy() ? "导出中…" : "选择位置并导出"}
                </button>
              </div>
            </div>
          </div>
        </Show>

        {/* 修改主密码子弹层 */}
        <Show when={pwOpen()}>
          <div class="modal-backdrop" style={{ "z-index": "70" }} onClick={(e) => e.target === e.currentTarget && !pwBusy() && setPwOpen(false)}>
            <div class="modal" style={{ width: "400px" }}>
              <div class="modal-head">
                <h3>修改主密码</h3>
              </div>
              <div class="modal-body">
                <p style={{ "font-size": "12.5px", color: "var(--text-dim)", "line-height": "1.6", "margin-bottom": "14px" }}>
                  将用新密码<strong style={{ color: "var(--text)" }}>重新加密整个保险库</strong>。
                  若已配置 GitHub 同步，请先确保各设备<strong style={{ color: "var(--text)" }}>已同步一致</strong>；
                  修改后云端会一并轮换为新密码，其他设备需各自用新密码解锁并同样修改。
                </p>
                <div class="form-group">
                  <label>原主密码</label>
                  <input class="input" type="password" value={pwOld()} placeholder="当前主密码" onInput={(e) => setPwOld(e.currentTarget.value)} />
                </div>
                <div class="form-group">
                  <label>新主密码（至少 8 位）</label>
                  <input class="input" type="password" value={pwNew()} placeholder="新主密码" onInput={(e) => setPwNew(e.currentTarget.value)} />
                </div>
                <div class="form-group">
                  <label>确认新主密码</label>
                  <input
                    class="input"
                    type="password"
                    value={pwConfirm()}
                    placeholder="再次输入新主密码"
                    onInput={(e) => setPwConfirm(e.currentTarget.value)}
                    onKeyUp={(e) => e.key === "Enter" && doChangePassword()}
                  />
                </div>
                <Show when={pwErr()}>
                  <div class="form-error">{pwErr()}</div>
                </Show>
              </div>
              <div class="modal-foot">
                <button class="btn btn-ghost" onClick={() => setPwOpen(false)} disabled={pwBusy()}>
                  取消
                </button>
                <button class="btn btn-primary" onClick={doChangePassword} disabled={pwBusy()}>
                  {pwBusy() ? "重新加密中…" : "确认修改"}
                </button>
              </div>
            </div>
          </div>
        </Show>
      </div>
    </div>
  );
}
