import { createSignal, createEffect, onCleanup, Show, For } from "solid-js";
import { Item, ItemKind, api, avatarColor, initials, kindLabel, maskSecret, TotpResult } from "../lib/api";
import {
  CopyIcon,
  CheckIcon,
  EyeIcon,
  EyeOffIcon,
  EditIcon,
  TrashIcon,
  LinkIcon,
  TagIcon,
  ShieldIcon,
} from "./icons";

type Login = Extract<ItemKind, { type: "login" }>;
type ApiKey = Extract<ItemKind, { type: "api_key" }>;
type Note = Extract<ItemKind, { type: "note" }>;

interface Props {
  item: Item | null;
  onEdit: (it: Item) => void;
  onDelete: (it: Item) => void;
  onToast: (msg: string) => void;
}

// 单个可复制/可揭密的字段
function SecretField(props: { label: string; value: string; onToast: (m: string) => void }) {
  const [reveal, setReveal] = createSignal(false);
  const [copied, setCopied] = createSignal(false);

  async function copy() {
    const secs = await api.copySecret(props.value);
    setCopied(true);
    props.onToast(
      secs > 0 ? `已复制${props.label}，${secs} 秒后自动清除` : `已复制${props.label}（按设置不自动清除）`
    );
    setTimeout(() => setCopied(false), 1500);
  }

  return (
    <div class="field-row">
      <div class="field-label">{props.label}</div>
      <div class="secret-box">
        <span class="secret-val" classList={{ masked: !reveal() }}>
          {reveal() ? props.value : maskSecret(props.value)}
        </span>
        <button class="mini-btn" title={reveal() ? "隐藏" : "显示"} onClick={() => setReveal(!reveal())}>
          <Show when={reveal()} fallback={<EyeIcon size={16} />}>
            <EyeOffIcon size={16} />
          </Show>
        </button>
        <button class="mini-btn" classList={{ copied: copied() }} title="复制" onClick={copy}>
          <Show when={copied()} fallback={<CopyIcon size={16} />}>
            <CheckIcon size={16} />
          </Show>
        </button>
      </div>
    </div>
  );
}

// TOTP 实时倒计时环
function TotpField(props: { secret: string; onToast: (m: string) => void }) {
  const [res, setRes] = createSignal<TotpResult | null>(null);
  const [left, setLeft] = createSignal(0);
  const [copied, setCopied] = createSignal(false);
  let timer: ReturnType<typeof setInterval> | undefined;

  async function refresh() {
    try {
      const r = await api.generateTotp(props.secret);
      setRes(r);
      setLeft(r.remaining_secs);
    } catch {
      setRes(null);
    }
  }

  createEffect(() => {
    const s = props.secret;
    if (!s) return;
    refresh();
    clearInterval(timer);
    timer = setInterval(() => {
      if (left() <= 1) refresh();
      else setLeft(left() - 1);
    }, 1000);
  });
  onCleanup(() => clearInterval(timer));

  async function copy() {
    const code = res()?.code;
    if (!code) return;
    const secs = await api.copySecret(code);
    setCopied(true);
    props.onToast(secs > 0 ? `已复制验证码，${secs} 秒后自动清除` : "已复制验证码（按设置不自动清除）");
    setTimeout(() => setCopied(false), 1500);
  }

  const period = () => res()?.period || 30;
  const frac = () => left() / period();
  const R = 13;
  const CIRC = 2 * Math.PI * R;

  return (
    <div class="field-row">
      <div class="field-label">动态验证码 (TOTP)</div>
      <div class="totp-box">
        <div class="totp-ring">
          <svg width="30" height="30" viewBox="0 0 30 30">
            <circle cx="15" cy="15" r={R} fill="none" stroke="var(--stroke)" stroke-width="3" />
            <circle
              cx="15"
              cy="15"
              r={R}
              fill="none"
              stroke="var(--accent)"
              stroke-width="3"
              stroke-linecap="round"
              stroke-dasharray={String(CIRC)}
              stroke-dashoffset={String(CIRC * (1 - frac()))}
            />
          </svg>
          <span class="ring-num">{left()}</span>
        </div>
        <span class="totp-code">{res()?.code ?? "------"}</span>
        <button
          class="mini-btn"
          classList={{ copied: copied() }}
          title="复制验证码"
          onClick={copy}
          style={{ "margin-left": "auto" }}
        >
          <Show when={copied()} fallback={<CopyIcon size={16} />}>
            <CheckIcon size={16} />
          </Show>
        </button>
      </div>
    </div>
  );
}

function PlainField(props: { label: string; children: any }) {
  return (
    <div class="field-row">
      <div class="field-label">{props.label}</div>
      <div class="plain-val">{props.children}</div>
    </div>
  );
}

function LoginBody(props: { k: Login; onToast: (m: string) => void }) {
  return (
    <>
      <Show when={props.k.username}>
        <PlainField label="用户名">
          <span style={{ "font-family": "var(--mono)" }}>{props.k.username}</span>
        </PlainField>
      </Show>
      <SecretField label="密码" value={props.k.password} onToast={props.onToast} />
      <Show when={props.k.url}>
        <PlainField label="网址">
          <a onClick={() => props.onToast("网址将在后续版本支持一键打开")}>
            <LinkIcon size={14} /> {props.k.url}
          </a>
        </PlainField>
      </Show>
      <Show when={props.k.totp} keyed>
        {(secret) => <TotpField secret={secret} onToast={props.onToast} />}
      </Show>
    </>
  );
}

function ApiKeyBody(props: { k: ApiKey; onToast: (m: string) => void }) {
  return (
    <>
      <Show when={props.k.key_name}>
        <PlainField label="Key 名称">{props.k.key_name}</PlainField>
      </Show>
      <SecretField label="Secret" value={props.k.secret} onToast={props.onToast} />
      <Show when={props.k.url}>
        <PlainField label="网址">
          <a onClick={() => props.onToast("网址将在后续版本支持一键打开")}>
            <LinkIcon size={14} /> {props.k.url}
          </a>
        </PlainField>
      </Show>
    </>
  );
}

function DetailView(props: Props & { item: Item }) {
  const item = () => props.item;
  const login = () => (item().kind.type === "login" ? (item().kind as Login) : undefined);
  const apiKey = () => (item().kind.type === "api_key" ? (item().kind as ApiKey) : undefined);
  const note = () => (item().kind.type === "note" ? (item().kind as Note) : undefined);

  return (
    <>
      <div class="detail-head">
        <div class="avatar" style={{ background: avatarColor(item().id) }}>
          {initials(item().title)}
        </div>
        <div style={{ "min-width": "0" }}>
          <div class="detail-title">{item().title}</div>
          <div class="detail-sub">{kindLabel(item().kind)}</div>
        </div>
        <div class="detail-actions">
          <button class="icon-btn" title="编辑" onClick={() => props.onEdit(item())}>
            <EditIcon size={17} />
          </button>
          <button class="icon-btn danger" title="删除" onClick={() => props.onDelete(item())}>
            <TrashIcon size={17} />
          </button>
        </div>
      </div>

      <div class="detail-body">
        <Show when={login()} keyed>
          {(k) => <LoginBody k={k} onToast={props.onToast} />}
        </Show>
        <Show when={apiKey()} keyed>
          {(k) => <ApiKeyBody k={k} onToast={props.onToast} />}
        </Show>
        <Show when={note()} keyed>
          {(k) => (
            <div class="field-row">
              <div class="field-label">笔记内容</div>
              <div class="note-text">{k.body || "（空）"}</div>
            </div>
          )}
        </Show>

        <Show when={item().tags.length > 0}>
          <PlainField label="标签">
            <div class="tag-chips">
              <For each={item().tags}>
                {(t) => (
                  <span class="chip">
                    <TagIcon size={11} /> {t}
                  </span>
                )}
              </For>
            </div>
          </PlainField>
        </Show>
      </div>
    </>
  );
}

export default function ItemDetail(props: Props) {
  return (
    <div class="detail">
      <Show
        when={props.item}
        keyed
        fallback={
          <div class="detail-empty">
            <div class="shield-sm">
              <ShieldIcon size={26} />
            </div>
            <div>选择一个条目查看详情</div>
          </div>
        }
      >
        {(item) => <DetailView {...props} item={item} />}
      </Show>
    </div>
  );
}
