#!/usr/bin/env bash
# 一次性初始化本地自签代码签名身份：生成自签证书 → 导入独立钥匙串 → 写入 .env.signing。
# 仅用于本地开发/自签打包，使钥匙串 ACL 与 Touch ID 拥有稳定签名身份；不用于分发。
set -euo pipefail
cd "$(dirname "$0")/.."

CERT_CN="Sentinel Self-Signed Code Signing"
KC_NAME="sentinel-signing.keychain-db"
KC="$HOME/Library/Keychains/$KC_NAME"
ENV_FILE=".env.signing"

if [[ -f "$ENV_FILE" ]]; then
  echo "已存在 $ENV_FILE，如需重建请先删除该文件与钥匙串 $KC_NAME"
  exit 1
fi

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

P12_PW="$(openssl rand -hex 16)"
KC_PW="$(openssl rand -hex 16)"

echo "==> 生成自签代码签名证书（CN=$CERT_CN）"
openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
  -keyout "$WORK/signing.key" -out "$WORK/signing.crt" \
  -subj "/CN=$CERT_CN" \
  -addext "keyUsage=digitalSignature" \
  -addext "extendedKeyUsage=codeSigning" \
  -addext "basicConstraints=critical,CA:false"
openssl pkcs12 -export -out "$WORK/signing.p12" \
  -inkey "$WORK/signing.key" -in "$WORK/signing.crt" \
  -passout "pass:$P12_PW" -name "$CERT_CN"

echo "==> 创建独立钥匙串 $KC_NAME 并导入"
if [[ ! -f "$KC" ]]; then
  security create-keychain -p "$KC_PW" "$KC"
fi
security unlock-keychain -p "$KC_PW" "$KC"
security import "$WORK/signing.p12" -k "$KC" -P "$P12_PW" \
  -T /usr/bin/codesign -T /usr/bin/security
# 允许 codesign 免提示使用该私钥
security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KC_PW" "$KC"

echo "==> 加入用户钥匙串搜索列表"
read -r -a EXISTING <<<"$(security list-keychains -d user | tr -d '"' | tr '\n' ' ')"
FOUND=0
for k in "${EXISTING[@]}"; do
  [[ "$k" == *"$KC_NAME" ]] && FOUND=1
done
if [[ "$FOUND" -eq 0 ]]; then
  security list-keychains -d user -s "${EXISTING[@]}" "$KC"
fi

cat >"$ENV_FILE" <<EOF
# 本地自签身份配置（已 gitignore，勿提交）
APPLE_SIGNING_IDENTITY="$CERT_CN"
SIGNING_KEYCHAIN="$KC"
SIGNING_KEYCHAIN_PASSWORD="$KC_PW"
EOF
chmod 600 "$ENV_FILE"

echo
echo "完成。身份：$CERT_CN"
echo "签名构建请运行：scripts/build-signed.sh"
echo "验证身份可见：security find-identity -v -p codesigning"
