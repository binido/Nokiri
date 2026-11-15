use crate::{config, model::State, platform::atomic_write};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    task::JoinHandle,
};

pub const TEST_URL: &str = "https://www.gstatic.com/generate_204";
pub const IP_URL: &str = "https://api.ipify.org";
const TIMEOUT: Duration = Duration::from_secs(8);
#[derive(Deserialize)]
pub struct CoreManifest {
    pub version: String,
    pub binary_sha256: String,
}
pub fn manifest() -> CoreManifest {
    serde_json::from_str(include_str!("../assets/core.json")).unwrap()
}
pub fn core_path() -> Result<PathBuf> {
    let executable = std::env::current_exe()?;
    if let Some(contents) = executable.parent().and_then(Path::parent)
        && contents.file_name().is_some_and(|s| s == "Contents")
    {
        return Ok(contents.join("Resources/mihomo"));
    }
    Ok(Path::new(env!("CARGO_MANIFEST_DIR")).join("target/core/mihomo"))
}
pub async fn verify_binary(path: &Path, expected: &str) -> Result<()> {
    let bytes = tokio::fs::read(path)
        .await
        .context("Mihomo is missing. Run scripts/fetch_core.py before building the app.")?;
    if hex::encode(Sha256::digest(bytes)) != expected {
        bail!(
            "Mihomo checksum mismatch. The executable will not be run. Reinstall the verified core."
        );
    }
    Ok(())
}

pub trait ProxyEngine {
    fn start(&mut self, state: &State) -> impl std::future::Future<Output = Result<()>> + Send;
    fn stop(&mut self) -> impl std::future::Future<Output = Result<()>> + Send;
    fn poll(&mut self) -> Result<bool>;
}

pub struct Mihomo {
    binary: PathBuf,
    expected_hash: String,
    child: Option<Child>,
    runtime: Option<tempfile::TempDir>,
    readers: Vec<JoinHandle<()>>,
    logs: Arc<Mutex<VecDeque<String>>>,
    pub controller: Option<Controller>,
    pub version: String,
}
impl Mihomo {
    pub fn new(binary: PathBuf, expected_hash: String) -> Self {
        Self {
            binary,
            expected_hash,
            child: None,
            runtime: None,
            readers: Vec::new(),
            logs: Arc::new(Mutex::new(VecDeque::new())),
            controller: None,
            version: String::new(),
        }
    }
    pub fn running(&self) -> bool {
        self.child.is_some()
    }
    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().and_then(Child::id)
    }
    pub fn diagnostics(&self) -> String {
        self.logs
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub async fn validate_config(&self, path: &Path, dir: &Path) -> Result<()> {
        verify_binary(&self.binary, &self.expected_hash).await?;
        let output = tokio::time::timeout(
            TIMEOUT,
            Command::new(&self.binary)
                .args(["-t", "-d"])
                .arg(dir)
                .arg("-f")
                .arg(path)
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Core configuration validation timed out")??;
        if !output.status.success() {
            bail!(
                "Mihomo rejected the generated configuration. Check the imported transport parameters; the proxy was not started."
            );
        }
        Ok(())
    }
    async fn launch(&mut self, state: &State) -> Result<()> {
        if self.running() {
            bail!("The proxy engine is already running.");
        }
        let proxy_probe = std::net::TcpListener::bind(("127.0.0.1", state.settings.port)).context(
            "Local proxy port is in use or unavailable. Choose another port in Settings.",
        )?;
        let controller_probe = std::net::TcpListener::bind(("127.0.0.1", 0))?;
        let port = controller_probe.local_addr()?.port();
        let secret = uuid::Uuid::new_v4().to_string();
        let yaml = config::generate(state, port, &secret)?;
        let runtime = tempfile::Builder::new().prefix("nokiri-").tempdir()?;
        let path = runtime.path().join("config.yaml");
        atomic_write(&path, yaml.as_bytes())?;
        self.validate_config(&path, runtime.path()).await?;
        let output = tokio::time::timeout(
            TIMEOUT,
            Command::new(&self.binary)
                .arg("-v")
                .kill_on_drop(true)
                .output(),
        )
        .await??;
        self.version = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .unwrap_or("Mihomo")
            .to_string();
        drop(proxy_probe);
        drop(controller_probe);
        self.logs.lock().unwrap().clear();
        let mut child = Command::new(&self.binary)
            .arg("-d")
            .arg(runtime.path())
            .arg("-f")
            .arg(&path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()
            .context("Could not start Mihomo")?;
        let mut sensitive = vec![secret.clone()];
        for server in &state.servers {
            sensitive.extend([
                server.proxy.uuid.clone(),
                server.proxy.server.clone(),
                server.proxy.name.clone(),
            ]);
        }
        macro_rules! capture {
            ($pipe:expr) => {{
                let pipe = $pipe;
                let logs = self.logs.clone();
                let sensitive = sensitive.clone();
                self.readers.push(tokio::spawn(async move {
                    let mut lines = BufReader::new(pipe).lines();
                    while let Ok(Some(mut line)) = lines.next_line().await {
                        for value in &sensitive {
                            if !value.is_empty() {
                                line = line.replace(value, "[redacted]");
                            }
                        }
                        if line.len() > 1024 {
                            line = line.chars().take(512).collect();
                        }
                        let mut buffer = logs.lock().unwrap();
                        if buffer.len() == 24 {
                            buffer.pop_front();
                        }
                        buffer.push_back(line);
                    }
                }));
            }};
        }
        capture!(child.stdout.take().unwrap());
        capture!(child.stderr.take().unwrap());
        self.child = Some(child);
        self.runtime = Some(runtime);
        self.controller = Some(Controller::new(port, secret)?);
        let start = Instant::now();
        loop {
            if !self.poll()? {
                bail!("Mihomo exited during startup. See diagnostics for redacted core output.");
            }
            if let Ok(response) = self
                .controller
                .as_ref()
                .unwrap()
                .get("/version")
                .send()
                .await
                && response.status().is_success()
            {
                break;
            }
            if start.elapsed() > TIMEOUT {
                bail!("Mihomo controller did not become ready within 8 seconds.");
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let selected = state.selected_server().unwrap();
        self.controller
            .as_ref()
            .unwrap()
            .select(&selected.id)
            .await?;
        Ok(())
    }
}
impl ProxyEngine for Mihomo {
    async fn start(&mut self, state: &State) -> Result<()> {
        if self.running() {
            bail!("The proxy engine is already running.");
        }
        let result = self.launch(state).await;
        if result.is_err() {
            self.stop().await?;
        }
        result
    }
    async fn stop(&mut self) -> Result<()> {
        self.controller = None;
        let result = if let Some(mut child) = self.child.take() {
            child.start_kill().ok();
            tokio::time::timeout(Duration::from_secs(3), child.wait())
                .await
                .context("Timed out waiting for Mihomo to stop")?
                .map(|_| ())
                .map_err(anyhow::Error::from)
        } else {
            Ok(())
        };
        for reader in self.readers.drain(..) {
            reader.abort();
        }
        self.runtime.take();
        result
    }
    fn poll(&mut self) -> Result<bool> {
        if let Some(child) = &mut self.child {
            if let Some(status) = child.try_wait()? {
                self.child.take();
                self.controller = None;
                self.runtime.take();
                bail!(
                    "Mihomo stopped unexpectedly ({status}). The local proxy is unavailable; no fallback was activated."
                );
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
impl Drop for Mihomo {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.start_kill();
        }
        for reader in self.readers.drain(..) {
            reader.abort();
        }
    }
}

#[derive(Clone)]
pub struct Controller {
    base: String,
    secret: String,
    client: reqwest::Client,
}
impl Controller {
    pub fn new(port: u16, secret: String) -> Result<Self> {
        Ok(Self {
            base: format!("http://127.0.0.1:{port}"),
            secret,
            client: reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(7))
                .build()?,
        })
    }
    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.secret)
    }
    pub async fn select(&self, id: &str) -> Result<()> {
        self.client
            .put(format!("{}/proxies/{}", self.base, config::SELECTOR))
            .bearer_auth(&self.secret)
            .json(&serde_json::json!({"name": id}))
            .send()
            .await
            .context("Could not restore the fixed server selection")?
            .error_for_status()
            .context("Core rejected the selected server")?;
        Ok(())
    }
    pub async fn delay(&self, id: &str) -> Result<u64> {
        let response = self
            .get(&format!("/proxies/{id}/delay"))
            .query(&[("url", TEST_URL), ("timeout", "5000"), ("expected", "204")])
            .send()
            .await
            .map_err(|_| anyhow::anyhow!("Server test timed out or the core is unavailable."))?;
        if !response.status().is_success() {
            bail!("HTTPS test failed or timed out through this server.");
        }
        let value: serde_json::Value = response.json().await?;
        value["delay"]
            .as_u64()
            .filter(|ms| *ms > 0)
            .context("Core did not report a successful HTTPS test")
    }
}

pub fn proxy_client(port: u16, timeout: Duration) -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .no_proxy()
        .proxy(reqwest::Proxy::all(format!("http://127.0.0.1:{port}"))?)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(timeout)
        .build()?)
}
pub async fn connectivity(port: u16, exit_ip: bool) -> Result<String> {
    let response = proxy_client(port, TIMEOUT)?
        .get(if exit_ip { IP_URL } else { TEST_URL })
        .send()
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Proxied HTTPS request failed or timed out. No direct request was attempted."
            )
        })?;
    if exit_ip {
        if response.status() != reqwest::StatusCode::OK {
            bail!(
                "Exit IP service returned HTTP {}.",
                response.status().as_u16()
            );
        }
        let mut response = response;
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if bytes.len() + chunk.len() > 64 {
                bail!("Invalid exit IP response.");
            }
            bytes.extend_from_slice(&chunk);
        }
        let text = std::str::from_utf8(&bytes)?.trim();
        let ip: std::net::IpAddr = text
            .parse()
            .context("Exit IP response is not an IP address")?;
        Ok(ip.to_string())
    } else {
        if response.status() != reqwest::StatusCode::NO_CONTENT {
            bail!(
                "Connectivity endpoint returned HTTP {}, expected 204.",
                response.status().as_u16()
            );
        }
        Ok("HTTPS verified through the selected proxy".into())
    }
}
