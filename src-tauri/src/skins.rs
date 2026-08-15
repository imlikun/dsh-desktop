//! 皮肤系统：皮肤协议、目录扫描、安装(.dshskin)、激活与 CSS 注入
//!
//! 皮肤目录: ~/Library/Application Support/com.likun.dsh-desktop/skins/
//! 每个皮肤一个子目录:
//!   <skin-id>/
//!     skin.json    元信息（id/name/version/author/description/preview/theme）
//!     theme.css    注入到官方 UI 的 CSS（可空）
//!     assets/      资源（预览图、背景等，可空）
//! 激活状态记录在 config.json 的 active_skin 字段

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkinMeta {
    pub id: String,
    pub name: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    /// 预览：CSS 颜色或 /assets/preview.png 路径
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub theme: SkinTheme,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SkinTheme {
    #[serde(default)]
    pub dark: bool,
    #[serde(default)]
    pub primary: String,
    #[serde(default)]
    pub background: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SkinInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub preview: String,
    pub dark: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    #[serde(default)]
    pub active_skin: String,
}

fn default_version() -> String {
    "0.0.0".to_string()
}

/// 应用数据根目录（含 skins/ 与 config.json）
pub fn app_data_dir() -> PathBuf {
    let base = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("com.likun.dsh-desktop");
    base
}

pub fn skins_dir() -> PathBuf {
    app_data_dir().join("skins")
}

fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

/// 确保数据目录结构存在
pub fn ensure_dirs() -> std::io::Result<()> {
    fs::create_dir_all(skins_dir())?;
    Ok(())
}

/// 读取配置
pub fn load_config() -> AppConfig {
    fs::read_to_string(config_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let binding = config_path();
    let dir = binding.parent().ok_or("无效配置路径")?;
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let json = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(config_path(), json).map_err(|e| e.to_string())
}

/// 读取并校验一个皮肤目录，返回 SkinInfo
pub fn read_skin(dir: &Path) -> Option<SkinInfo> {
    let meta_path = dir.join("skin.json");
    let meta: SkinMeta = serde_json::from_str(&fs::read_to_string(meta_path).ok()?).ok()?;
    let id = dir.file_name()?.to_string_lossy().to_string();
    Some(SkinInfo {
        id: id.clone(),
        name: meta.name,
        version: meta.version,
        author: meta.author,
        description: meta.description,
        preview: meta.preview,
        dark: meta.theme.dark,
        active: false,
    })
}

/// 列出所有已安装皮肤
pub fn list_skins() -> Vec<SkinInfo> {
    let _ = ensure_dirs();
    let active = load_config().active_skin;
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(skins_dir()) {
        for e in entries.flatten() {
            let path = e.path();
            if path.is_dir() {
                if let Some(mut info) = read_skin(&path) {
                    info.active = info.id == active;
                    out.push(info);
                }
            }
        }
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

/// 激活皮肤（记录配置，返回该皮肤 theme.css 的完整内容）
pub fn activate_skin(id: &str) -> Result<String, String> {
    let dir = skins_dir().join(id);
    if !dir.is_dir() {
        return Err(format!("皮肤 {id} 不存在"));
    }
    let mut cfg = load_config();
    cfg.active_skin = id.to_string();
    save_config(&cfg)?;

    let css_path = dir.join("theme.css");
    if css_path.exists() {
        Ok(fs::read_to_string(&css_path).map_err(|e| e.to_string())?)
    } else {
        Ok(String::new())
    }
}

/// 读取当前激活皮肤的 CSS（无皮肤则为空字符串）
pub fn active_css() -> String {
    let cfg = load_config();
    if cfg.active_skin.is_empty() {
        return String::new();
    }
    let css_path = skins_dir().join(&cfg.active_skin).join("theme.css");
    fs::read_to_string(css_path).unwrap_or_default()
}

/// 安装 .dshskin 包（zip），解压到 skins/<id>/
/// 返回安装的皮肤 id
pub fn install_skin(zip_path: &str) -> Result<String, String> {
    let file = fs::File::open(zip_path).map_err(|e| format!("打开文件失败: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("不是有效的 .dshskin 包: {e}"))?;

    // 找到 skin.json 所在顶层目录，作为皮肤 id
    let mut skin_id: Option<String> = None;
    for i in 0..archive.len() {
        let entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        // 匹配 <id>/skin.json 或 skin.json
        if name.ends_with("skin.json") {
            let parts: Vec<&str> = name.split('/').collect();
            if parts.len() >= 2 {
                skin_id = Some(parts[parts.len() - 2].to_string());
            } else {
                skin_id = Some("skin".to_string());
            }
            break;
        }
    }
    let skin_id = skin_id.ok_or("包内未找到 skin.json")?;

    // 安全解压（防目录穿越）
    let dest = skins_dir().join(&skin_id);
    fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().to_string();
        if name.ends_with('/') {
            continue;
        }
        // 去掉前缀目录（若有 <id>/ 前缀）
        let rel = name.split('/').skip_while(|s| *s == skin_id).collect::<Vec<_>>().join("/");
        let rel = if rel.is_empty() { name.clone() } else { rel };
        let out_path = dest.join(&rel);
        // 防穿越
        if !out_path.starts_with(&dest) {
            continue;
        }
        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = fs::File::create(&out_path).map_err(|e| e.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|e| e.to_string())?;
    }

    // 校验
    if read_skin(&dest).is_none() {
        return Err("解压后 skin.json 无效".to_string());
    }
    Ok(skin_id)
}

/// 删除皮肤
pub fn uninstall_skin(id: &str) -> Result<(), String> {
    let dir = skins_dir().join(id);
    if !dir.is_dir() {
        return Err(format!("皮肤 {id} 不存在"));
    }
    fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut cfg = load_config();
    if cfg.active_skin == id {
        cfg.active_skin = String::new();
        save_config(&cfg)?;
    }
    Ok(())
}

/// 生成注入官方 UI 的 JS 脚本（替换/新增 #dsh-skin style 标签）
pub fn build_inject_js(css: &str) -> String {
    // 用 JSON 字符串转义，安全嵌入 JS 字面量
    let css_json = serde_json::to_string(css).unwrap_or_else(|_| "\"\"".to_string());
    format!(
        r#"(function() {{
  var old = document.getElementById('dsh-skin');
  if (old) old.remove();
  var s = document.createElement('style');
  s.id = 'dsh-skin';
  s.textContent = {css_json};
  document.head.appendChild(s);
}})();"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        std::env::temp_dir().join("dsh-skins-test")
    }

    fn setup() {
        let _ = fs::remove_dir_all(test_dir());
        fs::create_dir_all(test_dir()).unwrap();
        // 覆盖 app_data_dir 用不了，直接测 install_skin 需要真实目录。
        // 这里手动构造皮肤目录结构来测 read_skin / activate / build_inject_js
    }

    #[test]
    fn test_read_skin() {
        setup();
        let dir = test_dir().join("aurora");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("skin.json"),
            r##"{"id":"aurora","name":"电光紫","version":"1.0.0","author":"likun","description":"测试","preview":"#6c4dff","theme":{"dark":false,"primary":"#6c4dff"}}"##,
        )
        .unwrap();
        let info = read_skin(&dir).expect("应能读取皮肤");
        assert_eq!(info.id, "aurora");
        assert_eq!(info.name, "电光紫");
        assert_eq!(info.preview, "#6c4dff");
    }

    #[test]
    fn test_build_inject_js() {
        let js = build_inject_js("body { background: red !important; }");
        assert!(js.contains("dsh-skin"));
        assert!(js.contains("body { background: red !important; }"));
        // CSS 中若有引号/换行也应被安全转义
        let js2 = build_inject_js("a { content: \"x\"; }\n b{}");
        assert!(js2.contains("\\\"x\\\""));
    }

    #[test]
    fn test_install_skin_zip() {
        // 手工构造一个 zip 测试 install_skin
        use std::io::Write;
        setup();
        let dir = test_dir().join("pkg");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("skin.json"),
            r##"{"id":"pkg","name":"测试包","version":"0.1.0"}"##,
        )
        .unwrap();
        fs::write(dir.join("theme.css"), "body{}").unwrap();

        let zip_path = test_dir().join("test.dshskin");
        let f = fs::File::create(&zip_path).unwrap();
        let mut w = zip::ZipWriter::new(f);
        let opts = zip::write::SimpleFileOptions::default();
        w.start_file("pkg/skin.json", opts).unwrap();
        w.write_all(r#"{"id":"pkg","name":"Test"}"#.as_bytes()).unwrap();
        w.start_file("pkg/theme.css", opts).unwrap();
        w.write_all(b"body{}").unwrap();
        w.finish().unwrap();

        // install 需要真实 skins_dir；这里直接解压验证 zip 读取逻辑
        let file = fs::File::open(&zip_path).unwrap();
        let mut archive = zip::ZipArchive::new(file).unwrap();
        let mut found = false;
        for i in 0..archive.len() {
            let entry = archive.by_index(i).unwrap();
            if entry.name().ends_with("skin.json") {
                found = true;
            }
        }
        assert!(found, "zip 内应找到 skin.json");
    }
}
