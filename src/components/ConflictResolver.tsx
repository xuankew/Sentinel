import { createSignal, For, Show } from "solid-js";
import {
  api,
  MergeConflict,
  Side,
  Item,
  avatarColor,
  initials,
  maskSecret,
  primarySecret,
  diffFields,
  conflictKind,
  relTime,
} from "../lib/api";
import { WarnIcon } from "./icons";

interface Props {
  conflicts: MergeConflict[];
  onDone: (msg: string) => void;
  onCancel: () => void;
}

// 单侧版本卡片：可点选，选中高亮
function VersionCard(props: {
  side: Side;
  item: Item | null;
  base: Item | null;
  chosen: boolean;
  onChoose: () => void;
}) {
  const it = () => props.item;

  return (
    <button
      class="conflict-card"
      classList={{ chosen: props.chosen, deleted: !it() }}
      onClick={props.onChoose}
      disabled={!it()}
    >
      <div class="cc-head">
        <Show
          when={it()}
          fallback={<span class="cc-deleted">此版本已删除该条目</span>}
        >
          <div class="cc-avatar" style={{ background: avatarColor(it()!.id) }}>
            {initials(it()!.title)}
          </div>
          <div style={{ "min-width": "0", flex: "1" }}>
            <div class="cc-title">{it()!.title}</div>
            <div class="cc-time">改于 {relTime(it()!.updated_at)}</div>
          </div>
        </Show>
        <Show when={props.chosen}>
          <span class="cc-badge">已选</span>
        </Show>
      </div>

      <Show when={it()}>
        <div class="cc-secret">{maskSecret(primarySecret(it()!))}</div>
        <Show when={props.base && diffFields(props.base, it()!).length > 0}>
          <div class="cc-diff">改动：{diffFields(props.base, it()!).join("、")}</div>
        </Show>
      </Show>
    </button>
  );
}

export default function ConflictResolver(props: Props) {
  // 每条冲突默认预选 suggested，用户可改选
  const [choices, setChoices] = createSignal<Record<string, Side>>(
    Object.fromEntries(props.conflicts.map((c) => [c.id, c.suggested])),
  );
  const [busy, setBusy] = createSignal(false);
  const [error, setError] = createSignal("");

  function choose(id: string, side: Side) {
    setChoices((c) => ({ ...c, [id]: side }));
  }

  async function submit() {
    setError("");
    setBusy(true);
    try {
      const res = await api.resolveSync(choices());
      if (res.status === "synced") props.onDone(`冲突已解决 · 共 ${res.total} 条`);
      else props.onDone("同步完成");
    } catch (e) {
      setError(String(e));
      setBusy(false);
    }
  }

  async function abandon() {
    setError("");
    try {
      await api.cancelSync();
    } catch {
      /* 忽略：本地状态未变 */
    }
    props.onCancel();
  }

  return (
    <div class="modal-backdrop">
      <div class="modal" style={{ width: "560px" }}>
        <div class="modal-head">
          <h3>
            <span style={{ display: "inline-flex", "vertical-align": "-3px", "margin-right": "8px", color: "var(--warn)" }}>
              <WarnIcon size={18} />
            </span>
            同步冲突 · {props.conflicts.length} 条
          </h3>
        </div>

        <div class="modal-body">
          <p style={{ "font-size": "12.5px", color: "var(--text-dim)", "line-height": "1.6", "margin-bottom": "16px" }}>
            以下条目在本机和远端都被改动，无法自动合并。请为每条选择保留哪个版本，未选中的版本将被丢弃。
          </p>

          <For each={props.conflicts}>
            {(c) => (
              <div class="conflict-item">
                <div class="conflict-name">
                  {(c.ours ?? c.theirs)?.title ?? "（已删除条目）"}
                  <span class="conflict-kind">{conflictKind(c)}</span>
                </div>
                <div class="conflict-cols">
                  <VersionCard
                    side="ours"
                    item={c.ours}
                    base={c.base}
                    chosen={choices()[c.id] === "ours"}
                    onChoose={() => choose(c.id, "ours")}
                  />
                  <VersionCard
                    side="theirs"
                    item={c.theirs}
                    base={c.base}
                    chosen={choices()[c.id] === "theirs"}
                    onChoose={() => choose(c.id, "theirs")}
                  />
                </div>
              </div>
            )}
          </For>

          <Show when={error()}>
            <div class="form-error" style={{ "margin-top": "14px" }}>
              {error()}
            </div>
          </Show>
        </div>

        <div class="modal-foot" style={{ "justify-content": "space-between" }}>
          <button class="btn btn-ghost" onClick={abandon} disabled={busy()}>
            放弃同步
          </button>
          <button class="btn btn-primary" onClick={submit} disabled={busy()}>
            {busy() ? "提交中…" : `解决并同步 ${props.conflicts.length} 条`}
          </button>
        </div>
      </div>
    </div>
  );
}
