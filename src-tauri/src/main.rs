//! DeepSeek Harness Desktop — Tauri 2 主进程
//!
//! 职责：启动/停止 dsh 子进程、承载官方 UI、皮肤系统（扫描/安装/切换/注入）、设置窗口

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod harness;
mod skins;

use std::sync::Mutex;
use tauri::{
    AppHandle, Emitter, Manager, RunEvent, State, WebviewUrl, WebviewWindowBuilder,
};

/// 全局状态：dsh 子进程
struct AppState {
    harness: Mutex<Option<harness::Harness>>,
}

// ---------- IPC 命令 ----------

#[tauri::command]
fn list_skins() -> Vec<skins::SkinInfo> {
    skins::list_skins()
}

#[tauri::command]
fn activate_skin(app: AppHandle, id: String) -> Result<(), String> {
    let css = skins::activate_skin(&id)?;
    inject_css(&app, &css);
    Ok(())
}

#[tauri::command]
fn install_skin(app: AppHandle, zip_path: String) -> Result<skins::SkinInfo, String> {
    let id = skins::install_skin(&zip_path)?;
    // 安装后自动激活
    let css = skins::activate_skin(&id)?;
    inject_css(&app, &css);
    skins::list_skins()
        .into_iter()
        .find(|s| s.id == id)
        .ok_or("皮肤已安装但列表读取失败".to_string())
}

#[tauri::command]
fn uninstall_skin(app: AppHandle, id: String) -> Result<(), String> {
    skins::uninstall_skin(&id)?;
    let css = skins::active_css();
    inject_css(&app, &css);
    Ok(())
}

#[tauri::command]
fn get_harness_status(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let guard = state.harness.lock().map_err(|e| e.to_string())?;
    match guard.as_ref() {
        Some(h) => Ok(serde_json::json!({
            "running": true,
            "port": h.port,
            "url": format!("http://127.0.0.1:{}", h.port),
        })),
        None => Ok(serde_json::json!({"running": false})),
    }
}

#[tauri::command]
fn open_skins_dir() -> Result<(), String> {
    let dir = skins::skins_dir();
    let _ = std::fs::create_dir_all(&dir);
    std::process::Command::new("open")
        .arg(&dir)
        .spawn()
        .map_err(|e| format!("打开目录失败: {e}"))?;
    Ok(())
}

#[tauri::command]
fn get_dsh_bin() -> Option<String> {
    harness::find_dsh()
}

/// 弹出文件选择器选 .dshskin 并安装，返回新皮肤信息
#[tauri::command]
async fn pick_and_install_skin(
    app: AppHandle,
) -> Result<Option<skins::SkinInfo>, String> {
    use tauri_plugin_dialog::DialogExt;
    let file = app
        .dialog()
        .file()
        .add_filter("DSH 皮肤包", &["dshskin"])
        .blocking_pick_file();
    let Some(path) = file else {
        return Ok(None); // 用户取消
    };
    let path_str = path
        .into_path()
        .map_err(|e| e.to_string())?
        .to_string_lossy()
        .to_string();
    let id = skins::install_skin(&path_str)?;
    let css = skins::activate_skin(&id)?;
    inject_css(&app, &css);
    Ok(skins::list_skins().into_iter().find(|s| s.id == id))
}

/// 打开设置窗口
#[tauri::command]
fn open_settings(app: AppHandle) -> Result<(), String> {
    open_settings_window(&app)?;
    Ok(())
}

// ---------- 内部函数 ----------

/// 向主窗口注入 CSS（切换到当前激活皮肤）
fn inject_css(app: &AppHandle, css: &str) {
    if let Some(main) = app.get_webview_window("main") {
        let js = skins::build_inject_js(css);
        let _ = main.eval(&js);
    }
}

fn open_settings_window(app: &AppHandle) -> Result<(), String> {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }
    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App("settings.html".into()))
        .title("DSH Desktop 设置")
        .inner_size(720.0, 560.0)
        .resizable(true)
        .build()
        .map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            skins::ensure_dirs().map_err(|e| e.to_string())?;

            // 应用菜单：设置…(Cmd+,) + 打开皮肤目录
            use tauri::menu::{Menu, MenuItem};
            let settings_item = MenuItem::with_id(app, "settings", "设置…", true, Some("CmdOrCtrl+,"))
                .map_err(|e| e.to_string())?;
            let skins_item = MenuItem::with_id(app, "openskins", "打开皮肤目录", true, None::<&str>)
                .map_err(|e| e.to_string())?;
            let menu = Menu::with_items(app, &[&settings_item, &skins_item])
                .map_err(|e| e.to_string())?;
            app.set_menu(menu).map_err(|e| e.to_string())?;

            // 菜单事件
            let app_handle = app.handle().clone();
            app.on_menu_event(move |app, event| match event.id().as_ref() {
                "settings" => {
                    let _ = open_settings_window(app);
                }
                "openskins" => {
                    let _ = open_skins_dir();
                }
                _ => {}
            });
            let _ = app_handle;

            // 1. 探测并启动 dsh
            let dsh_bin = harness::find_dsh();
            let state = AppState {
                harness: Mutex::new(None),
            };
            app.manage(state);

            match dsh_bin {
                Some(bin) => match harness::spawn_dsh(&bin) {
                    Ok(h) => {
                        let port = h.port;
                        {
                            let st: State<'_, AppState> = app.state();
                            *st.harness.lock().unwrap() = Some(h);
                        }
                        // 2. 创建主窗口，加载官方 UI
                        let url = format!("http://127.0.0.1:{port}/");
                        let win = WebviewWindowBuilder::new(
                            app,
                            "main",
                            WebviewUrl::External(url.parse().unwrap()),
                        )
                        .title("DeepSeek Harness")
                        .inner_size(1280.0, 820.0)
                        .min_inner_size(900.0, 600.0)
                        .on_page_load(|window, _payload| {
                            // 页面加载完成后注入激活皮肤
                            let css = skins::active_css();
                            if !css.is_empty() {
                                let js = skins::build_inject_js(&css);
                                let _ = window.eval(&js);
                            }
                        })
                        .build()
                        .map_err(|e| format!("创建主窗口失败: {e}"))?;
                        let _ = win.show();
                    }
                    Err(e) => {
                        eprintln!("dsh 启动失败: {e}");
                        // 显示错误窗口
                        let win = WebviewWindowBuilder::new(
                            app,
                            "main",
                            WebviewUrl::App("error.html".into()),
                        )
                        .title("DSH Desktop - 启动失败")
                        .inner_size(640.0, 400.0)
                        .build()
                        .map_err(|e| e.to_string())?;
                        let _ = win.show();
                    }
                },
                None => {
                    eprintln!("未找到 dsh，请先 npm install -g @deepseek-ai/dsh");
                    let win = WebviewWindowBuilder::new(
                        app,
                        "main",
                        WebviewUrl::App("error.html".into()),
                    )
                    .title("DSH Desktop - 缺少 dsh")
                    .inner_size(640.0, 400.0)
                    .build()
                    .map_err(|e| e.to_string())?;
                    let _ = win.show();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_skins,
            activate_skin,
            install_skin,
            uninstall_skin,
            get_harness_status,
            open_skins_dir,
            get_dsh_bin,
            open_settings,
            pick_and_install_skin,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    // 主窗口关闭 → 退出应用（由 RunEvent::Exit 统一收尾）
                    let _ = window.app_handle().exit(0);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Tauri 应用构建失败")
        .run(|app, event| {
            // macOS 双击 .dshskin 打开 → 自动安装皮肤
            if let RunEvent::Opened { ref urls } = event {
                for url in urls {
                    if let Ok(path) = url.to_file_path() {
                        let path_str = path.to_string_lossy().to_string();
                        if path_str.ends_with(".dshskin") {
                            match skins::install_skin(&path_str) {
                                Ok(id) => {
                                    let css = skins::activate_skin(&id).unwrap_or_default();
                                    inject_css(app, &css);
                                    // 通知设置窗口刷新皮肤列表
                                    let _ = app.emit("skins-changed", ());
                                    // 若已安装过，重新激活并保持最新 CSS
                                    let _ = skins::active_css();
                                }
                                Err(e) => eprintln!("安装皮肤失败: {e}"),
                            }
                        }
                    }
                }
            }
            // 退出时停止 dsh 子进程
            if let RunEvent::Exit = event {
                if let Some(st) = app.try_state::<AppState>() {
                    if let Ok(mut guard) = st.harness.lock() {
                        if let Some(h) = guard.as_mut() {
                            h.stop();
                        }
                    }
                }
            }
        });
}
