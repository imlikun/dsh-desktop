# DSH Desktop 皮肤协议 (v1)

皮肤是**独立安装**的主题包，不内置于主程序。每个皮肤一个目录，可打包为 `.dshskin`（zip）。

## 目录结构

```
<skin-id>/
├── skin.json      # 必填：元信息
├── theme.css      # 选填：注入官方 UI 的 CSS（可空文件）
└── assets/        # 选填：预览图、背景、字体等资源
```

## skin.json 规范

```json
{
  "id": "aurora-purple",
  "name": "电光紫",
  "version": "1.0.0",
  "author": "likun",
  "description": "电光紫流光玻璃风，呼应主站视觉",
  "preview": "#6c4dff",
  "theme": {
    "dark": false,
    "primary": "#6c4dff",
    "background": "#f7f7fb"
  }
}
```

| 字段 | 类型 | 必填 | 说明 |
|---|---|---|---|
| id | string | ✅ | 唯一标识，与目录名一致 |
| name | string | ✅ | 显示名 |
| version | string | 否 | 默认 "0.0.0" |
| author | string | 否 | 作者 |
| description | string | 否 | 一句话说明 |
| preview | string | 否 | 预览色（`#hex`）或 `assets/` 下图片路径 |
| theme.dark | bool | 否 | 深色主题标记 |
| theme.primary | string | 否 | 主色（供设置页显示） |
| theme.background | string | 否 | 背景色（供设置页显示） |

## theme.css 注入机制

- 应用启动、切换皮肤时，主进程把 `theme.css` 全文注入官方 UI 的 `<style id="dsh-skin">` 标签（幂等：先删旧再插新）。
- 官方 UI 是 DeepSeek Harness 自带 Web 界面，类名随版本变化。皮肤应尽量使用**稳定的全局选择器**：`:root` 变量、`body`、通用语义类。
- 官方 UI 若自身使用 CSS 变量，皮肤可覆盖同名变量实现整体换肤。

## 打包 .dshskin

```bash
cd <skin-id> && zip -r ../<skin-id>.dshskin .   # 或任意 zip 工具，顶层含 skin.json 即可
```

## 安装方式

1. 设置面板 →「安装皮肤包 (.dshskin)」文件选择器；
2. 或手动把皮肤目录放入 `~/Library/Application Support/com.likun.dsh-desktop/skins/`，重启生效；
3. 删除皮肤不会删除应用数据。
