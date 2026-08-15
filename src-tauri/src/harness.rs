//! dsh 子进程管理：探测可执行文件、spawn、解析随机端口、健康检查、停止

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

pub struct Harness {
    pub child: Child,
    pub port: u16,
}

/// 探测 dsh 可执行文件路径
/// 优先级：1) 环境变量 DSH_BIN  2) PATH 中的 dsh  3) managed node 常见安装位置
pub fn find_dsh() -> Option<String> {
    if let Ok(p) = std::env::var("DSH_BIN") {
        if !p.is_empty() && std::path::Path::new(&p).exists() {
            return Some(p);
        }
    }
    if let Ok(which) = Command::new("which").arg("dsh").output() {
        if which.status.success() {
            let p = String::from_utf8_lossy(&which.stdout).trim().to_string();
            if !p.is_empty() {
                return Some(p);
            }
        }
    }
    // managed node 常见路径：~/.workbuddy/binaries/node/versions/*/bin/dsh
    if let Some(home) = dirs::home_dir() {
        let glob = home.join(".workbuddy/binaries/node/versions");
        if let Ok(entries) = std::fs::read_dir(&glob) {
            let mut candidates: Vec<_> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("bin").join("dsh"))
                .filter(|p| p.exists())
                .collect();
            candidates.sort();
            if let Some(p) = candidates.last() {
                return Some(p.to_string_lossy().to_string());
            }
        }
    }
    None
}

fn parse_port_from_line(line: &str) -> Option<u16> {
    // 期望格式: dsh web: http://127.0.0.1:49473
    let idx = line.rfind(':')?;
    line[idx + 1..].trim().parse::<u16>().ok()
}

/// 轮询 TCP 端口直到可连接（服务就绪）
fn wait_port_ready(port: u16, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    false
}

/// 启动 dsh web（随机端口），解析端口并等待就绪
pub fn spawn_dsh(dsh_path: &str) -> Result<Harness, String> {
    let mut cmd = Command::new(dsh_path);
    cmd.args(["web", "--port", "0"])
        .stdout(Stdio::piped())
        // stderr 丢弃，避免管道缓冲区填满导致 dsh 阻塞
        .stderr(Stdio::null());
    // dsh 是 #!/usr/bin/env node 脚本；.app 从 launchd 启动时 PATH 无 node，
    // 把 dsh 所在目录（node bin）注入 PATH，保证 node 可被找到
    if let Some(dir) = std::path::Path::new(dsh_path).parent() {
        let dir_s = dir.to_string_lossy().to_string();
        let cur = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{dir_s}:{cur}"));
    }
    let mut child = cmd.spawn().map_err(|e| format!("启动 dsh 失败: {e}"))?;

    let stdout = child.stdout.take().ok_or("无法读取 dsh 输出")?;
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            if let Ok(l) = line {
                let _ = tx.send(l);
            }
        }
    });

    // 从 stdout 解析端口（dsh 输出: dsh web: http://127.0.0.1:PORT）
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut port: Option<u16> = None;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(line) => {
                if let Some(p) = parse_port_from_line(&line) {
                    port = Some(p);
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    let port = port.ok_or("无法从 dsh 输出解析端口（可能启动失败）")?;

    // 等待服务就绪（TCP 可连接）
    if !wait_port_ready(port, Duration::from_secs(30)) {
        let _ = child.kill();
        return Err(format!("dsh 服务在端口 {port} 30 秒内未就绪"));
    }

    Ok(Harness { child, port })
}

impl Harness {
    /// 停止 dsh 子进程
    pub fn stop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
