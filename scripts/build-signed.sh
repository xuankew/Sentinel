#!/usr/bin/env bash
# 用本地自签身份构建签名 .app（解锁钥匙串 + 注入 APPLE_SIGNING_IDENTITY）。
# 产物：src-tauri/target/release/bundle/macos/Sentinel.app（加 --debug 则为 debug 包）。
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ ! -f .env.signing ]]; then
  echo "尚未初始化自签身份，请先运行：scripts/setup-signing.sh"
  exit 1
fi

set -a
# shellcheck disable=SC1091
source .env.signing
set +a

security unlock-keychain -p "$SIGNING_KEYCHAIN_PASSWORD" "$SIGNING_KEYCHAIN"
exec pnpm tauri build "$@"
