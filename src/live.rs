//! Private, bounded local IPC. Only the GUI thread accesses the authoritative editor.
use eframe::egui;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{BufRead, Read, Write},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant, SystemTime},
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    token: String,
}

pub(crate) struct Request {
    pub line: String,
    pub reply: mpsc::SyncSender<Option<Value>>,
    cancelled: Arc<AtomicBool>,
    marker: PathBuf,
}

impl Request {
    pub fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire) || !self.marker.exists()
    }
}

pub struct Host {
    _directory: tempfile::TempDir,
    path: PathBuf,
    requests: mpsc::Receiver<Request>,
    stop: Arc<AtomicBool>,
}

pub fn directories() -> Result<Vec<PathBuf>, String> {
    if let Some(path) = std::env::var_os("PAPERCRAFT_LIVE_DIR") {
        return Ok(vec![std::path::absolute(path).map_err(|e| e.to_string())?]);
    }
    let mut dirs = Vec::new();
    if let Some(primary) =
        crate::recents::Recents::default_store().and_then(|p| p.parent().map(|p| p.join("live")))
    {
        dirs.push(primary);
    }
    let temp = std::env::temp_dir().join("papercraft-live");
    dirs.push(temp.clone());
    #[cfg(unix)]
    {
        let fallback = PathBuf::from("/tmp/papercraft-live");
        if fallback != temp {
            dirs.push(fallback);
        }
    }
    Ok(dirs)
}

pub fn directory() -> Result<PathBuf, String> {
    directories()?
        .into_iter()
        .next()
        .ok_or("Cannot locate the local live-session directory.".into())
}

fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| e.to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(limit as u64 + 1).read_to_end(&mut bytes))
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("IPC message too large.".into());
    }
    Ok(bytes)
}

impl Host {
    pub fn start_default(ctx: egui::Context) -> Result<Self, String> {
        let mut last_error = String::from("Cannot locate the local live-session directory.");
        for dir in directories()? {
            match Self::start(ctx.clone(), &dir) {
                Ok(host) => return Ok(host),
                Err(error) => last_error = error,
            }
        }
        Err(last_error)
    }

    pub fn start(ctx: egui::Context, directory: &Path) -> Result<Self, String> {
        std::fs::create_dir_all(directory).map_err(|e| e.to_string())?;
        let private = tempfile::Builder::new()
            .prefix("session-")
            .tempdir_in(directory)
            .map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(private.path(), std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        let endpoint = Endpoint { token: nonce()? };
        let path = private.path().join("session.json");
        crate::document::atomic_write(
            &path,
            &serde_json::to_vec(&endpoint).map_err(|e| e.to_string())?,
        )?;
        let heartbeat = private.path().join("alive");
        std::fs::write(&heartbeat, b"1").map_err(|e| e.to_string())?;
        let (send, requests) = mpsc::sync_channel(16);
        let stop = Arc::new(AtomicBool::new(false));
        let shutdown = stop.clone();
        std::thread::spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                if std::fs::write(&heartbeat, b"1").is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(500));
            }
        });
        let shutdown = stop.clone();
        let folder = private.path().to_owned();
        std::thread::spawn(move || {
            while !shutdown.load(Ordering::Acquire) {
                let Ok(entries) = std::fs::read_dir(&folder) else {
                    break;
                };
                for entry in entries.flatten().take(64) {
                    let path = entry.path();
                    if path.extension().is_none_or(|ext| ext != "req") {
                        continue;
                    }
                    let marker = path.with_extension("processing");
                    if std::fs::rename(&path, &marker).is_err() {
                        continue;
                    }
                    let response = receive(&marker, &endpoint.token, &send, &ctx);
                    let envelope = match response {
                        Ok(response) => serde_json::json!({"response":response}),
                        Err(error) => serde_json::json!({"error":error}),
                    };
                    // A disconnected client removes its marker; never leave a response for it.
                    if marker.exists() {
                        let _ = crate::document::atomic_write(
                            &path.with_extension("response"),
                            envelope.to_string().as_bytes(),
                        );
                    }
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        Ok(Self {
            _directory: private,
            path,
            requests,
            stop,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn next(&self) -> Option<Request> {
        self.requests.try_recv().ok()
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn receive(
    marker: &Path,
    token: &str,
    send: &mpsc::SyncSender<Request>,
    ctx: &egui::Context,
) -> Result<Option<Value>, String> {
    let bytes = read_bounded(marker, crate::mcp::MAX_REQUEST_BYTES + 1024)?;
    let envelope: Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if envelope["token"].as_str() != Some(token) {
        return Err("Live-session authentication failed.".into());
    }
    let line = envelope
        .get("request")
        .ok_or("Missing request.")?
        .to_string();
    if line.len() > crate::mcp::MAX_REQUEST_BYTES {
        return Err("Request too large.".into());
    }
    let (reply, result) = mpsc::sync_channel(1);
    let cancelled = Arc::new(AtomicBool::new(false));
    send.try_send(Request {
        line,
        reply,
        cancelled: cancelled.clone(),
        marker: marker.to_owned(),
    })
    .map_err(|_| "Editor queue unavailable.".to_owned())?;
    ctx.request_repaint();
    let response = result.recv_timeout(Duration::from_secs(30));
    cancelled.store(true, Ordering::Release);
    response.map_err(|_| "Editor did not respond; inspect its state before retrying.".into())
}

fn endpoint(path: &Path) -> Result<Endpoint, String> {
    let value = serde_json::from_slice(&read_bounded(path, 4096)?)
        .map_err(|e| format!("Invalid live session: {e}"))?;
    if !active(path) {
        return Err("Live session is not active. Open PaperCraft again.".into());
    }
    Ok(value)
}

fn active(path: &Path) -> bool {
    path.parent()
        .and_then(|p| std::fs::metadata(p.join("alive")).ok())
        .and_then(|m| m.modified().ok())
        .and_then(|t| SystemTime::now().duration_since(t).ok())
        .is_some_and(|age| age < Duration::from_secs(5))
}

fn active_sessions() -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    let mut seen = Vec::new();
    for dir in directories()? {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let folder = entry.path();
            let path = folder.join("session.json");
            if folder
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("session-"))
                && endpoint(&path).is_ok()
            {
                let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                if !seen.contains(&canonical) {
                    seen.push(canonical);
                    paths.push(path);
                }
            }
        }
    }
    Ok(paths)
}

pub fn discover_optional() -> Result<Option<PathBuf>, String> {
    let mut paths = active_sessions()?;
    match paths.len() {
        0 => Ok(None),
        1 => Ok(Some(paths.remove(0))),
        _ => Err(
            "Several editors are open. Use mcp --live SESSION_FILE from the editor's file menu, or mcp --headless."
                .into(),
        ),
    }
}

pub fn discover() -> Result<PathBuf, String> {
    let mut paths = active_sessions()?;
    match paths.len() {
        0 => Err("No live editor. Open PaperCraft first.".into()),
        1 => Ok(paths.remove(0)),
        _ => Err(
            "Several editors are open. Use mcp --live SESSION_FILE from the editor's file menu."
                .into(),
        ),
    }
}

struct Pending(PathBuf);
impl Drop for Pending {
    fn drop(&mut self) {
        for ext in ["req", "processing", "response"] {
            let _ = std::fs::remove_file(self.0.with_extension(ext));
        }
    }
}

fn exchange(path: &Path, endpoint: &Endpoint, request: Value) -> Result<Value, String> {
    if !active(path) {
        return Err("Live session is not active.".into());
    }
    let folder = path.parent().ok_or("Invalid session path.")?;
    let pending = Pending(folder.join(format!("{}.req", nonce()?)));
    let bytes = serde_json::to_vec(&serde_json::json!({"token":endpoint.token,"request":request}))
        .map_err(|e| e.to_string())?;
    crate::document::atomic_write(&pending.0, &bytes)?;
    let response = pending.0.with_extension("response");
    let deadline = Instant::now() + Duration::from_secs(35);
    loop {
        if response.exists() {
            let result: Value =
                serde_json::from_slice(&read_bounded(&response, 128 * 1024 * 1024)?)
                    .map_err(|e| e.to_string())?;
            if let Some(error) = result.get("error") {
                return Err(error.to_string());
            }
            return Ok(result["response"].clone());
        }
        if !active(path) || Instant::now() >= deadline {
            return Err("Live editor disconnected or timed out; inspect before retrying. No headless fallback was used.".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub fn serve(path: &Path) -> Result<(), String> {
    let endpoint = endpoint(path)?;
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    loop {
        let mut bytes = Vec::new();
        let read = (&mut input)
            .take(crate::mcp::MAX_REQUEST_BYTES as u64 + 1)
            .read_until(b'\n', &mut bytes)
            .map_err(|e| e.to_string())?;
        if read == 0 {
            return Ok(());
        }
        if bytes.len() > crate::mcp::MAX_REQUEST_BYTES {
            return Err("Request too large.".into());
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let request: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(_) => {
                writeln!(output,"{}",serde_json::json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Invalid JSON."}})).map_err(|e| e.to_string())?;
                output.flush().map_err(|e| e.to_string())?;
                continue;
            }
        };
        let response = exchange(path, &endpoint, request)?;
        if !response.is_null() {
            writeln!(output, "{response}")
                .and_then(|()| output.flush())
                .map_err(|e| e.to_string())?;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authentication_rejects_requests_before_the_gui_queue() {
        let dir = tempfile::tempdir().unwrap();
        let host = Host::start(egui::Context::default(), dir.path()).unwrap();
        let invalid = Endpoint {
            token: "wrong".into(),
        };
        let error = exchange(
            host.path(),
            &invalid,
            serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize"}),
        )
        .unwrap_err();
        assert!(error.contains("authentication failed"), "{error}");
        assert!(host.next().is_none());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(host.path().parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o077,
                0
            );
        }
    }
}
