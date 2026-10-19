#!/usr/bin/env bash
# Sentinel 开发重启脚本。
# 设计原则：只清理 Sentinel 自身的残留进程，绝不误伤其他项目（尤其是同样用 Tauri 的 Vela）。
#   - Vela 用端口 1420/1421；Sentinel 用 1430/1431，互不冲突。
#   - 进程匹配一律带上本项目绝对路径，不用 "tauri dev" 这类通用关键字（否则会杀掉 Vela）。
set -u

PORT=1430
HMR_PORT=1431
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SENTINEL_BIN="$ROOT/src-tauri/target/debug/sentinel"

echo "==> Sentinel 项目根: $ROOT"

# 1) 释放 Sentinel 专用端口（1430/1431）
for p in "$PORT" "$HMR_PORT"; do
  pids="$(lsof -ti tcp:"$p" -sTCP:LISTEN 2>/dev/null || true)"
  if [ -n "$pids" ]; then
    echo "==> 端口 $p 被占用，清理 PID: $(echo "$pids" | tr '\n' ' ')"
    # shellcheck disable=SC2086
    kill $pids 2>/dev/null || true
  fi
done

# 2) 精确杀掉 Sentinel 自身的 vite / 二进制（按项目绝对路径匹配，Vela 路径不同不会命中）
pkill -f "$ROOT/node_modules.*vite" 2>/dev/null && echo "==> 已停止 Sentinel 的 vite" || true
pkill -f "$SENTINEL_BIN"           2>/dev/null && echo "==> 已停止 Sentinel 应用进程" || true

# 3) 等端口真正释放（最多 ~4s）
for _ in $(seq 1 10); do
  lsof -ti tcp:"$PORT" -sTCP:LISTEN >/dev/null 2>&1 || break
  sleep 0.4
done

echo "==> 启动 pnpm tauri dev（端口 $PORT）..."
cd "$ROOT" || exit 1
exec pnpm tauri dev
