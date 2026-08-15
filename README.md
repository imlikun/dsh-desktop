# DSH Desktop

**DeepSeek Harness 桌面封装**（Tauri 2 + Rust），核心卖点是**可独立安装的换肤系统** —— 官方 UI 原样承载，皮肤即装即换。

## 核心特点：换肤系统 🎨

皮肤 = 一个独立目录（`skin.json` + `theme.css` + `assets/`），打包成 `.dshskin` 文件后可独立分发、随时切换：

- **双击即装**：macOS 上 `.dshskin` 已注册文件关联，双击自动安装并激活，无需打开应用
- **三种安装途径**：双击 `.dshskin` / 设置面板选择文件 / 手动拖入 skins 目录
- **即切即生效**：切换皮肤实时注入 CSS，无需重启、无需刷新页面
- **零门槛制作**：皮肤本质是 CSS + 一个 JSON 元信息文件，会 CSS 就能做皮肤

```
skins/
└─ aurora-purple/
   ├─ skin.json     # 元信息：id / name / version / preview
   ├─ theme.css     # 注入官方 UI 的样式（核心）
   └─ assets/       # 预览图、背景等资源
```

皮肤协议完整规范见 [docs/SKINS.md](docs/SKINS.md)。内置两个示例皮肤：`aurora-purple`（电光紫）、`neon-green`（荧光绿终端风）。

## 其他特性

- **官方 UI 原样承载**：启动 `@deepseek-ai/dsh` 本地服务（随机端口），Tauri 窗口内嵌官方 Web UI
- **安全边界**：dsh 只监听 `127.0.0.1` 随机端口；无内置 API Key（在官方 UI 设置里配）
- **零配置启动**：自动探测 dsh（PATH / 环境变量 `DSH_BIN` / 常见安装位置），找不到给出引导

## 运行

前置：Node.js ≥ 22.19，`npm install -g @deepseek-ai/dsh`

```bash
# 开发（需要 rustup + cargo）
cd src-tauri && cargo run

# 打包 .app
npx tauri build
```

## 数据位置

| 内容 | 路径 |
|---|---|
| 皮肤 | `~/Library/Application Support/com.likun.dsh-desktop/skins/` |
| 激活状态 | `~/Library/Application Support/com.likun.dsh-desktop/config.json` |
| dsh 数据 | `~/.dsh/`（官方 Harness 配置、会话） |

## 技术栈

- [Tauri 2](https://tauri.app) + Rust（壳、子进程管理、皮肤系统）
- 官方 [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)（`@deepseek-ai/dsh`）
- 设置面板：纯 HTML/CSS/JS（无构建链）

## 说明

非 DeepSeek 官方产品，不提供模型额度、不绕过 API 鉴权。DeepSeek Harness 仍处于 Developer Preview。
