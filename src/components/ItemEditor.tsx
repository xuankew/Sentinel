import { createSignal, Show } from "solid-js";
import jsQR from "jsqr";
import { Item, ItemDraft, ItemKind } from "../lib/api";
import { XIcon, RefreshIcon, EyeIcon, EyeOffIcon, QrIcon } from "./icons";

interface Props {
  initial: Item | null; // null = 新建
  onSave: (draft: ItemDraft) => Promise<void> | void;
  onClose: () => void;
}

type KindType = "login" | "api_key" | "note";

const TABS: { key: KindType; label: string }[] = [
  { key: "login", label: "登录密码" },
  { key: "api_key", label: "API Key" },
  { key: "note", label: "安全笔记" },
];

// 生成一个较强随机密码（前端仅用 crypto.getRandomValues）
function genPassword(len = 20): string {
  const sets = ["ABCDEFGHJKLMNPQRSTUVWXYZ", "abcdefghijkmnopqrstuvwxyz", "23456789", "!@#$%^&*-_=+"];
  const all = sets.join("");
  const buf = new Uint32Array(len);
  crypto.getRandomValues(buf);
  const chars: string[] = [];
  // 每类至少一个
  sets.forEach((s, i) => chars.push(s[buf[i] % s.length]));
  for (let i = sets.length; i < len; i++) chars.push(all[buf[i] % all.length]);
  // 洗牌
  for (let i = chars.length - 1; i > 0; i--) {
    const j = buf[i] % (i + 1);
    [chars[i], chars[j]] = [chars[j], chars[i]];
  }
  return chars.join("");
}

function emptyKind(t: KindType): ItemKind {
  if (t === "login") return { type: "login", username: "", password: "", url: "", totp: null, notes: "" };
  if (t === "api_key") return { type: "api_key", key_name: "", secret: "", url: "", notes: "" };
  return { type: "note", body: "" };
}

// 判断某类型下是否已填入内容（切换会丢失这些字段）
function kindHasContent(k: ItemKind): boolean {
  const anyK = k as any;
  if (k.type === "login")
    return !!(anyK.username || anyK.password || anyK.url || anyK.totp || anyK.notes);
  if (k.type === "api_key")
    return !!(anyK.key_name || anyK.secret || anyK.url || anyK.notes);
  return !!anyK.body;
}

// 从图片文件本地解码二维码，返回其中的文本（通常是 otpauth:// 链接）。
// 全程在 WebView 内用 canvas + jsQR 完成，不上传、不申请屏幕录制权限。
async function decodeQrFromFile(file: File): Promise<string> {
  if (!file.type.startsWith("image/")) throw new Error("请选择图片文件");
  const bitmap = await createImageBitmap(file);
  try {
    const canvas = document.createElement("canvas");
    canvas.width = bitmap.width;
    canvas.height = bitmap.height;
    const ctx = canvas.getContext("2d", { willReadFrequently: true });
    if (!ctx) throw new Error("无法读取图片数据");
    ctx.drawImage(bitmap, 0, 0);
    const img = ctx.getImageData(0, 0, canvas.width, canvas.height);
    const code = jsQR(img.data, img.width, img.height, {
      inversionAttempts: "attemptBoth",
    });
    const text = code?.data?.trim();
    if (!text) throw new Error("未在图片中识别到二维码，请确保截图清晰完整");
    return text;
  } finally {
    bitmap.close();
  }
}

export default function ItemEditor(props: Props) {
  const init = props.initial;
  const [kindType, setKindType] = createSignal<KindType>(
    init ? (init.kind.type as KindType) : "login",
  );
  const [title, setTitle] = createSignal(init?.title ?? "");
  const [kind, setKind] = createSignal<ItemKind>(init ? structuredClone(init.kind) : emptyKind("login"));
  const [tags, setTags] = createSignal((init?.tags ?? []).join(", "));
  const [favorite, setFavorite] = createSignal(init?.favorite ?? false);
  const [error, setError] = createSignal("");
  const [busy, setBusy] = createSignal(false);
  // 敏感字段默认遮罩（type=password），点眼睛才明文显示
  const [showPw, setShowPw] = createSignal(false);
  const [showSecret, setShowSecret] = createSignal(false);
  const [showTotp, setShowTotp] = createSignal(false);
  // 切换类型时若已填内容，先挂起等确认，避免误清
  const [pendingType, setPendingType] = createSignal<KindType | null>(null);
  // 二维码识别：状态提示 + 隐藏文件选择器
  const [qrBusy, setQrBusy] = createSignal(false);
  const [qrMsg, setQrMsg] = createSignal("");
  const [qrErr, setQrErr] = createSignal(false);
  const [qrOver, setQrOver] = createSignal(false);
  let qrFileInput!: HTMLInputElement;

  async function handleQrFile(file: File | undefined) {
    if (!file) return;
    setQrBusy(true);
    setQrMsg("");
    setQrErr(false);
    try {
      const text = await decodeQrFromFile(file);
      patch({ totp: text });
      setQrErr(false);
      setQrMsg("已识别二维码并填入 TOTP 密钥");
    } catch (e) {
      setQrErr(true);
      setQrMsg(String(e));
    } finally {
      setQrBusy(false);
      // 清空以便同一文件可再次选择触发 change
      if (qrFileInput) qrFileInput.value = "";
    }
  }

  function applySwitch(t: KindType) {
    setKindType(t);
    setKind(emptyKind(t));
    setPendingType(null);
  }

  function switchType(t: KindType) {
    if (t === kindType()) return;
    if (kindHasContent(kind())) {
      setPendingType(t);
      return;
    }
    applySwitch(t);
  }

  const pendingLabel = () => TABS.find((x) => x.key === pendingType())?.label ?? "";

  // 类型安全的字段更新
  function patch(p: Partial<Record<string, any>>) {
    setKind((k) => ({ ...k, ...p }) as ItemKind);
  }

  function parseTags(): string[] {
    return tags()
      .split(/[,，]/)
      .map((t) => t.trim())
      .filter(Boolean);
  }

  async function save() {
    setError("");
    if (!title().trim()) return setError("请填写标题");
    const draft: ItemDraft = {
      title: title().trim(),
      kind: kind(),
      tags: parseTags(),
      favorite: favorite(),
    };
    setBusy(true);
    try {
      await props.onSave(draft);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  }

  const k = () => kind() as any;

  return (
    <div class="modal-backdrop" onClick={(e) => e.target === e.currentTarget && props.onClose()}>
      <div class="modal">
        <div class="modal-head">
          <h3>{init ? "编辑条目" : "新建条目"}</h3>
          <button class="icon-btn" onClick={props.onClose} title="关闭">
            <XIcon size={17} />
          </button>
        </div>

        <div class="modal-body">
          <div class="type-tabs">
            {TABS.map((t) => (
              <button
                class="type-tab"
                classList={{ active: kindType() === t.key }}
                onClick={() => switchType(t.key)}
                disabled={!!init}
                style={init ? { opacity: kindType() === t.key ? 1 : 0.4, cursor: "default" } : {}}
              >
                {t.label}
              </button>
            ))}
          </div>

          <Show when={pendingType()}>
            <div class="touch-ask type-switch-ask">
              <span class="tsa-text">
                切换到「{pendingLabel()}」将清空当前已填的字段内容（标题与标签保留）。
              </span>
              <div class="tsa-actions">
                <button class="btn btn-ghost" onClick={() => setPendingType(null)}>
                  取消
                </button>
                <button class="btn btn-danger" onClick={() => applySwitch(pendingType()!)}>
                  确认切换
                </button>
              </div>
            </div>
          </Show>

          <div class="form-group">
            <label>标题</label>
            <input class="input" value={title()} placeholder="例如：GitHub / 阿里云" onInput={(e) => setTitle(e.currentTarget.value)} />
          </div>

          <Show when={kindType() === "login"}>
            <div class="form-group">
              <label>用户名</label>
              <input class="input" value={k().username} onInput={(e) => patch({ username: e.currentTarget.value })} />
            </div>
            <div class="form-group">
              <label>密码</label>
              <div class="gen-row">
                <input
                  class="input mono"
                  type={showPw() ? "text" : "password"}
                  value={k().password}
                  onInput={(e) => patch({ password: e.currentTarget.value })}
                />
                <button
                  class="btn btn-ghost"
                  title={showPw() ? "隐藏密码" : "显示密码"}
                  onClick={() => setShowPw(!showPw())}
                >
                  {showPw() ? <EyeOffIcon size={16} /> : <EyeIcon size={16} />}
                </button>
                <button
                  class="btn btn-ghost"
                  title="生成随机强密码（覆盖当前内容）"
                  onClick={() => patch({ password: genPassword() })}
                >
                  <RefreshIcon size={16} />
                </button>
              </div>
            </div>
            <div class="form-group">
              <label>网址</label>
              <input class="input" value={k().url} placeholder="https://" onInput={(e) => patch({ url: e.currentTarget.value })} />
            </div>
            <div
              class="form-group qr-drop"
              classList={{ "qr-dragover": qrOver() }}
              onDragOver={(e) => {
                e.preventDefault();
                setQrOver(true);
              }}
              onDragLeave={() => setQrOver(false)}
              onDrop={(e) => {
                e.preventDefault();
                setQrOver(false);
                handleQrFile(e.dataTransfer?.files?.[0]);
              }}
            >
              <label>TOTP 密钥（可选，用于两步验证）</label>
              <div class="gen-row">
                <input
                  class="input mono"
                  type={showTotp() ? "text" : "password"}
                  value={k().totp ?? ""}
                  placeholder="Base32 或 otpauth:// 链接"
                  onInput={(e) => patch({ totp: e.currentTarget.value || null })}
                />
                <button
                  class="btn btn-ghost"
                  title={showTotp() ? "隐藏密钥" : "显示密钥"}
                  onClick={() => setShowTotp(!showTotp())}
                >
                  {showTotp() ? <EyeOffIcon size={16} /> : <EyeIcon size={16} />}
                </button>
                <button
                  class="btn btn-ghost"
                  title="从二维码图片识别（选择或拖入截图，本地解码，不上传）"
                  disabled={qrBusy()}
                  onClick={() => qrFileInput?.click()}
                >
                  <QrIcon size={16} />
                </button>
                <input
                  ref={qrFileInput}
                  type="file"
                  accept="image/*"
                  style={{ display: "none" }}
                  onChange={(e) => handleQrFile(e.currentTarget.files?.[0])}
                />
              </div>
              <Show when={qrBusy()}>
                <div class="form-sub">正在识别二维码…</div>
              </Show>
              <Show when={!qrBusy() && qrMsg()}>
                <div class="form-sub" classList={{ "form-sub-err": qrErr() }}>
                  {qrMsg()}
                </div>
              </Show>
            </div>
            <div class="form-group">
              <label>备注（可选）</label>
              <textarea
                class="input"
                value={k().notes ?? ""}
                rows={3}
                placeholder="例如：备用邮箱、恢复信息、说明等"
                onInput={(e) => patch({ notes: e.currentTarget.value })}
              />
            </div>
          </Show>

          <Show when={kindType() === "api_key"}>
            <div class="form-group">
              <label>Key 名称</label>
              <input class="input" value={k().key_name} placeholder="例如：production" onInput={(e) => patch({ key_name: e.currentTarget.value })} />
            </div>
            <div class="form-group">
              <label>Secret</label>
              <div class="gen-row">
                <input
                  class="input mono"
                  type={showSecret() ? "text" : "password"}
                  value={k().secret}
                  onInput={(e) => patch({ secret: e.currentTarget.value })}
                />
                <button
                  class="btn btn-ghost"
                  title={showSecret() ? "隐藏密钥" : "显示密钥"}
                  onClick={() => setShowSecret(!showSecret())}
                >
                  {showSecret() ? <EyeOffIcon size={16} /> : <EyeIcon size={16} />}
                </button>
                <button
                  class="btn btn-ghost"
                  title="生成随机密钥（覆盖当前内容）"
                  onClick={() => patch({ secret: genPassword(32) })}
                >
                  <RefreshIcon size={16} />
                </button>
              </div>
            </div>
            <div class="form-group">
              <label>网址</label>
              <input class="input" value={k().url} placeholder="https://" onInput={(e) => patch({ url: e.currentTarget.value })} />
            </div>
            <div class="form-group">
              <label>备注（可选）</label>
              <textarea
                class="input"
                value={k().notes ?? ""}
                rows={3}
                placeholder="例如：用途、轮换周期、说明等"
                onInput={(e) => patch({ notes: e.currentTarget.value })}
              />
            </div>
          </Show>

          <Show when={kindType() === "note"}>
            <div class="form-group">
              <label>笔记内容</label>
              <textarea class="input" value={k().body} rows={5} onInput={(e) => patch({ body: e.currentTarget.value })} />
            </div>
          </Show>

          <div class="form-group">
            <label>标签（逗号分隔）</label>
            <input class="input" value={tags()} placeholder="work, dev" onInput={(e) => setTags(e.currentTarget.value)} />
          </div>

          <label class="checkbox-row">
            <input type="checkbox" checked={favorite()} onChange={(e) => setFavorite(e.currentTarget.checked)} />
            加入收藏
          </label>

          <Show when={error()}>
            <div class="form-error" style={{ "margin-top": "14px" }}>
              {error()}
            </div>
          </Show>
        </div>

        <div class="modal-foot">
          <button class="btn btn-ghost" onClick={props.onClose}>
            取消
          </button>
          <button class="btn btn-primary" onClick={save} disabled={busy()}>
            {busy() ? "保存中…" : "保存"}
          </button>
        </div>
      </div>
    </div>
  );
}
