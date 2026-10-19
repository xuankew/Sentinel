<div align="center">

# 🛡️ Sentinel（哨兵）

**你的密钥，寸步不让。**

零知识、可离线、通过你自己的 GitHub 私有仓库同步的 **macOS 本地密码管理器**。

主密码派生密钥加密整库，云端只存密文——即使设备或仓库文件被盗，没有主密码也读不出任何一个字。

[![License: MIT](https://img.shields.io/badge/License-MIT-accent.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-2-24C8DB?logo=tauri)](https://tauri.app/)
[![SolidJS](https://img.shields.io/badge/SolidJS-1.9-4F88C6?logo=solid)](https://www.solidjs.com/)
[![Platform](https://img.shields.io/badge/platform-macOS-000?logo=apple)](#)

</div>

---

## ✨ 功能特性

- **三类条目**：登录密码、API Key、安全笔记，统一搜索（含笔记正文）、一键复制。
- **强密码生成**：内置随机强密码 / 密钥生成器，前端 `crypto.getRandomValues`。
- **TOTP 两步验证**：粘贴 Base32 或 `otpauth://` 链接，实时生成 6 位验证码。
- **剪贴板自动清除**：复制的敏感内容默认 30 秒后自动擦除。
- **零知识加密**：整库单文件加密落盘，主密码永不存储、永不上传。
- **GitHub 私有仓库同步**：多台 Mac 间同步密文，三方合并 + 冲突可视化裁决。
- **自动 / 手动同步**：可设 5/15/30/60 分钟自动同步，列表栏一键手动同步，侧栏显示上次同步时间。
- **Touch ID 解锁**：主密码存入生物识别门禁钥匙串，指纹即开（需已签名构建）。
- **空闲自动锁定**：无操作超时自动锁库。
- **加密导出 / 备份**：用独立导出密码重新封盘导出 `.sentinel`，**绝不导出明文**。
- **外观微调**：强调色 + 紧凑/舒适密度。

## 🔐 安全模型

| 环节 | 方案 |
| --- | --- |
| 密钥派生 | **Argon2id**（64 MiB 内存 / 3 迭代 / 1 并行度，V0x13） |
| 认证加密 | **XChaCha20-Poly1305**（256-bit 密钥，24 字节随机 nonce） |
| 完整性 | 文件头（含参数、盐）作为 **AAD** 参与认证，参数/盐/密文被篡改即解密失败 |
| 内存卫生 | 主密码、明文密钥使用 `Zeroizing` 用后清零 |
| 同步凭据 | GitHub Token 存 **macOS 钥匙串**（Keychain），绝不写入仓库或库文件 |
| Touch ID | 主密码存 `kSecAccessControlBiometryCurrentSet` 的**数据保护钥匙串**项 |

> **零知识**：Sentinel 不知道也无法知道你的主密码。加密/解密全部在本机完成，GitHub 上只存放密文。丢失主密码 = 数据无法恢复，请务必牢记并自行备份。

## 🚀 快速开始

### 前置要求

- macOS（Apple Silicon 或 Intel）
- [Rust](https://www.rust-lang.org/tools/install)（rustup）
- Node.js 20+ 与 [pnpm](https://pnpm.io/)
- Xcode Command Line Tools：`xcode-select --install`

### 从源码运行

```bash
# 1. 安装依赖
pnpm install

# 2. 开发模式（热重载）
pnpm tauri dev

# 3. 打包 .app / .dmg
pnpm tauri build
```

其他脚本：

```bash
pnpm dev          # 仅启动前端 Vite 开发服务器
pnpm build        # 仅构建前端到 dist/
cd src-tauri && cargo test   # 运行 Rust 加密/合并单测
```

> **关于签名**：`pnpm tauri dev` 产出的是临时 ad-hoc 签名，**Touch ID 无法启用**（数据保护钥匙串要求稳定签名），且每次构建都会触发一次钥匙串授权弹窗。正式使用请 `pnpm tauri build` 并用你自己的开发者证书签名。

## 🔄 配置 GitHub 同步

同步走你自己的私有仓库，Sentinel 不会替你创建任何远端资源。

1. **建一个私有仓库**（例如 `my-vault`），无需初始化任何文件。
2. **签发 Fine-grained Personal Access Token**：
   - GitHub → Settings → Developer settings → Fine-grained tokens → Generate new token。
   - **Repository access**：选 *Only select repositories*，勾选上面那个仓库。
   - **Permissions → Repository permissions → Contents**：设为 **Read and write**（Metadata 会自动带上，只读即可）。
   - 其余权限一律不授予。生成后复制令牌。
3. 在 Sentinel 侧栏点 **配置同步**，填入 Owner、仓库名、分支（默认 `main`）、文件路径（默认 `vault.sentinel`）与 Token，点 **测试连接** 通过后 **立即同步**。

Token 会被写入 macOS 钥匙串（service `com.sentinel.app`），不入库、不上云。

## 🗂️ 项目结构

```
src/                     前端（SolidJS + TypeScript）
  components/            三栏界面、编辑器、同步/设置面板、冲突裁决
  lib/api.ts             前端调用后端命令的封装层
  styles/vault.css       设计系统
src-tauri/src/           后端（Rust）
  crypto.rs              Argon2id + XChaCha20-Poly1305 加密内核
  vault.rs               .sentinel 文件格式与保险库持久化
  merge.rs               三方合并引擎
  sync.rs                GitHub Contents API 传输
  touchid.rs             生物识别门禁钥匙串
  commands.rs            Tauri IPC 命令
docs/                    产品文档（PRD.md）与 UI 交互稿
```

- 产品文档：[`docs/PRD.md`](docs/PRD.md)
- UI 交互稿：[`docs/ui-mockup.html`](docs/ui-mockup.html)（浏览器直接打开）

## 🧭 路线图

- [x] **M0** 脚手架：Tauri 2 + SolidJS + Vite，品牌外壳与前后端 IPC 通路
- [x] **M1** 加密内核 + 保险库创建/解锁/锁定（Argon2id + XChaCha20-Poly1305，密文落盘）
- [x] **M2** 主界面三栏 + 条目 CRUD + 搜索 + 一键复制（30s 自动清除）+ TOTP 实时验证码
- [x] **M3** GitHub 同步 + 三方合并 + 冲突裁决（私有仓库密文同步，Keychain 存 Token）
- [x] **M4** Touch ID 解锁 / 空闲自动锁定 / 强调色与密度 / 加密导出
- [x] **M5** 自动 + 手动同步 / 切换类型防误清 / 侧栏上次同步时间
- [ ] 动效打磨 · 自签打包发布

## ⚠️ 免责声明

Sentinel 是个人开源项目，**尚未经过第三方安全审计**。请自行评估风险后使用，并务必牢记主密码、定期备份加密文件。因使用本软件造成的任何数据丢失或安全问题，作者不承担责任。

## 📄 License

[MIT](LICENSE) © 2026 Sentinel Contributors
