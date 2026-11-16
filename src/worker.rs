use crate::{
    engine::{self, Mihomo, ProxyEngine},
    model::{State, Subscription, now},
    platform::Vault,
    subscription,
};
use anyhow::{Result, bail};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Clone, Default)]
pub struct Check {
    pub testing: bool,
    pub latency: Option<u64>,
    pub last_success: Option<u64>,
    pub error: Option<String>,
}
#[derive(Clone, Default)]
pub struct Snapshot {
    pub state: State,
    pub running: bool,
    pub busy: bool,
    pub message: String,
    pub error: bool,
    pub exit_ip: Option<String>,
    pub checks: BTreeMap<String, Check>,
    pub core_version: String,
    pub diagnostics: String,
    pub pid: Option<u32>,
    pub locked: bool,
}
pub enum Action {
    Start,
    Stop,
    Restart,
    Select(String),
    Import(String, bool),
    AddSubscription(String, String, bool),
    Update(String),
    Rename(String, String),
    RemoveSubscription(String),
    RemoveServer(String),
    Settings(crate::model::Settings),
    Test(Vec<String>),
    Connectivity(bool),
    Shutdown,
}
pub struct Handle {
    pub tx: UnboundedSender<Action>,
    pub rx: mpsc::Receiver<Snapshot>,
    pub shutdown: Shutdown,
}
#[derive(Clone)]
pub struct Shutdown {
    signal: tokio::sync::watch::Sender<bool>,
    thread: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}
impl Shutdown {
    pub fn requested(&self) -> bool {
        *self.signal.borrow()
    }
    pub fn stop(&self) {
        let _ = self.signal.send(true);
        if let Some(thread) = self.thread.lock().unwrap().take() {
            let _ = thread.join();
        }
    }
}
impl Handle {
    pub fn spawn(vault: Vault, ctx: eframe::egui::Context) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (updates, snapshots) = mpsc::channel();
        let (shutdown, stopped) = tokio::sync::watch::channel(false);
        let signal_shutdown = shutdown.clone();
        let thread = std::thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let signal_ctx = ctx.clone();
                tokio::spawn(async move {
                    let mut terminate =
                        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                            .expect("signal handler");
                    tokio::select! { _ = tokio::signal::ctrl_c() => {}, _ = terminate.recv() => {} }
                    let _ = signal_shutdown.send(true);
                    signal_ctx.send_viewport_cmd(eframe::egui::ViewportCommand::Close);
                });
                let mut snapshot = Snapshot::default();
                match vault.load() {
                    Ok(state) => snapshot.state = state,
                    Err(e) => {
                        snapshot.message = e.to_string();
                        snapshot.error = true;
                        snapshot.locked = true;
                    }
                }
                let binary = engine::core_path().unwrap_or_default();
                let mut worker = Worker {
                    vault,
                    snapshot,
                    engine: Mihomo::new(binary, engine::manifest().binary_sha256),
                    updates,
                    ctx,
                };
                worker.publish();
                if worker.snapshot.state.settings.start_on_launch && !worker.snapshot.locked {
                    let result = worker.engine.start(&worker.snapshot.state).await;
                    worker.finish(result);
                }
                worker.run(rx, stopped).await;
            });
        });
        Self {
            tx,
            rx: snapshots,
            shutdown: Shutdown {
                signal: shutdown,
                thread: Arc::new(Mutex::new(Some(thread))),
            },
        }
    }
}
impl Drop for Handle {
    fn drop(&mut self) {
        self.shutdown.stop();
    }
}
struct Worker {
    vault: Vault,
    snapshot: Snapshot,
    engine: Mihomo,
    updates: mpsc::Sender<Snapshot>,
    ctx: eframe::egui::Context,
}
impl Worker {
    fn publish(&mut self) {
        self.snapshot.running = self.engine.running();
        self.snapshot.core_version = self.engine.version.clone();
        self.snapshot.diagnostics = self.engine.diagnostics();
        self.snapshot.pid = self.engine.pid();
        let _ = self.updates.send(self.snapshot.clone());
        self.ctx.request_repaint();
    }
    fn finish(&mut self, result: Result<()>) {
        self.snapshot.busy = false;
        self.snapshot.error = result.is_err();
        if let Err(e) = result {
            self.snapshot.message = format!("{e:#}");
        }
        self.publish();
    }
    fn persist(&mut self, state: State) -> Result<()> {
        self.vault.save(&state)?;
        self.snapshot.state = state;
        Ok(())
    }
    async fn run(
        &mut self,
        mut actions: UnboundedReceiver<Action>,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        'actions: loop {
            if *shutdown.borrow() {
                break;
            }
            tokio::select! {
                _ = shutdown.changed() => break,
                action = actions.recv() => {
                    let Some(action) = action else { break; };
                    if matches!(action, Action::Shutdown) { break; }
                    if self.snapshot.locked { continue; }
                    self.snapshot.busy = true; self.snapshot.error = false;
                    self.snapshot.message = "Working...".into(); self.publish();
                    let result = tokio::select! {
                        result = self.execute(action) => result,
                        _ = shutdown.changed() => break 'actions,
                    };
                    self.finish(result);
                }
                _ = tick.tick(), if self.engine.running() => {
                    if let Err(e) = self.engine.poll() {
                        self.snapshot.exit_ip = None;
                        self.finish(Err(e));
                    }
                }
            }
        }
        let _ = self.engine.stop().await;
    }
    async fn refresh(&mut self, candidate: State) -> Result<()> {
        let was_running = self.engine.running();
        self.persist(candidate)?;
        self.snapshot.exit_ip = None;
        if was_running {
            self.engine.stop().await?;
            if self.snapshot.state.selected_server().is_none() {
                bail!(
                    "The selected server was removed. The proxy is stopped. Select a valid server explicitly."
                );
            }
            self.engine.start(&self.snapshot.state).await?;
        } else if self.snapshot.state.selected.is_some()
            && self.snapshot.state.selected_server().is_none()
        {
            bail!(
                "The selected server was removed. No other server was selected. Select a valid server explicitly."
            );
        }
        Ok(())
    }
    async fn execute(&mut self, action: Action) -> Result<()> {
        let mut candidate = self.snapshot.state.clone();
        match action {
            Action::Start => {
                self.engine.start(&candidate).await?;
                self.snapshot.message = "Local proxy is running. Selection is fixed.".into();
            }
            Action::Stop => {
                self.engine.stop().await?;
                self.snapshot.exit_ip = None;
                self.snapshot.message = "Local proxy stopped.".into();
            }
            Action::Restart => {
                self.engine.stop().await?;
                self.snapshot.exit_ip = None;
                self.engine.start(&candidate).await?;
                self.snapshot.message = "Proxy restarted with the same selection.".into();
            }
            Action::Select(id) => {
                if !candidate.servers.iter().any(|s| s.id == id) {
                    bail!("This server no longer exists.");
                }
                candidate.selected = Some(id);
                self.refresh(candidate).await?;
                self.snapshot.message =
                    "Server selected. This selection stays fixed until you change it.".into();
            }
            Action::Import(input, allow) => {
                let proxies = subscription::parse(&input)?;
                subscription::require_verified_tls(&proxies, allow)?;
                let mut all: Vec<_> = candidate
                    .servers
                    .iter()
                    .filter(|s| s.source == "manual")
                    .map(|s| s.proxy.clone())
                    .collect();
                all.extend(proxies);
                candidate.replace_source("manual", all);
                self.refresh(candidate).await?;
                self.snapshot.message = "Servers imported. Select a server to use it.".into();
            }
            Action::AddSubscription(name, url, allow) => {
                validate_name(&name)?;
                let proxies = subscription::fetch(&url).await?;
                subscription::require_verified_tls(&proxies, allow)?;
                let id = uuid::Uuid::new_v4().to_string();
                candidate.replace_source(&id, proxies);
                candidate.subscriptions.push(Subscription {
                    id,
                    name,
                    url,
                    updated: Some(now()),
                    status: "Updated successfully".into(),
                    allow_unverified_tls: allow,
                });
                self.refresh(candidate).await?;
                self.snapshot.message = "Subscription added.".into();
            }
            Action::Update(id) => {
                let index = candidate
                    .subscriptions
                    .iter()
                    .position(|s| s.id == id)
                    .ok_or_else(|| anyhow::anyhow!("Subscription no longer exists"))?;
                match subscription::fetch(&candidate.subscriptions[index].url).await {
                    Ok(proxies) => {
                        subscription::require_verified_tls(
                            &proxies,
                            candidate.subscriptions[index].allow_unverified_tls,
                        )?;
                        candidate.replace_source(&id, proxies);
                        candidate.subscriptions[index].updated = Some(now());
                        candidate.subscriptions[index].status = "Updated successfully".into();
                        self.refresh(candidate).await?;
                        self.snapshot.message =
                            "Subscription updated. Fixed selection preserved.".into();
                    }
                    Err(e) => {
                        candidate.subscriptions[index].status = e.to_string();
                        self.persist(candidate)?;
                        return Err(e.into());
                    }
                }
            }
            Action::Rename(id, name) => {
                validate_name(&name)?;
                if let Some(sub) = candidate.subscriptions.iter_mut().find(|s| s.id == id) {
                    sub.name = name;
                }
                self.persist(candidate)?;
                self.snapshot.message = "Subscription renamed.".into();
            }
            Action::RemoveSubscription(id) => {
                candidate.subscriptions.retain(|s| s.id != id);
                candidate.servers.retain(|s| s.source != id);
                self.refresh(candidate).await?;
                self.snapshot.message = "Subscription removed.".into();
            }
            Action::RemoveServer(id) => {
                candidate
                    .servers
                    .retain(|s| s.id != id || s.source != "manual");
                self.refresh(candidate).await?;
                self.snapshot.message = "Manual server removed.".into();
            }
            Action::Settings(settings) => {
                if settings.port == 0 {
                    bail!("Proxy port must be 1 to 65535.");
                }
                let changed = settings.port != candidate.settings.port;
                candidate.settings = settings;
                if changed {
                    self.refresh(candidate).await?;
                } else {
                    self.persist(candidate)?;
                }
                self.snapshot.message = "Settings saved.".into();
            }
            Action::Test(ids) => {
                let mut temporary = None;
                if !self.engine.running() {
                    if candidate.servers.is_empty() {
                        bail!("Import a server before testing.");
                    }
                    let mut core =
                        Mihomo::new(engine::core_path()?, engine::manifest().binary_sha256);
                    core.start_probe(&candidate).await?;
                    temporary = Some(core);
                }
                let controller = temporary
                    .as_ref()
                    .and_then(|c| c.controller.clone())
                    .or_else(|| self.engine.controller.clone())
                    .ok_or_else(|| anyhow::anyhow!("Core is unavailable"))?;
                for id in ids {
                    if !candidate.servers.iter().any(|s| s.id == id) {
                        continue;
                    }
                    let check = self.snapshot.checks.entry(id.clone()).or_default();
                    check.testing = true;
                    self.publish();
                    let result = controller.delay(&id).await;
                    let check = self.snapshot.checks.entry(id).or_default();
                    check.testing = false;
                    match result {
                        Ok(ms) => {
                            check.latency = Some(ms);
                            check.last_success = Some(now());
                            check.error = None;
                        }
                        Err(e) => {
                            check.latency = None;
                            check.error = Some(e.to_string());
                        }
                    }
                    self.publish();
                }
                if let Some(core) = &mut temporary {
                    core.stop().await?;
                }
                self.snapshot.message =
                    "Server tests finished. Routing selection was not changed.".into();
            }
            Action::Connectivity(ip) => {
                if !self.engine.poll()? {
                    bail!("Start the local proxy first.");
                }
                let result = engine::connectivity(candidate.settings.port, ip).await?;
                if ip {
                    self.snapshot.exit_ip = Some(result);
                    self.snapshot.message = "Exit IP verified through the selected proxy.".into();
                } else {
                    self.snapshot.message = result;
                }
            }
            Action::Shutdown => {}
        }
        Ok(())
    }
}
fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        bail!("Subscription names must contain 1 to 128 printable bytes.");
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::SecretStore;
    struct Secrets;
    impl SecretStore for Secrets {
        fn key(&self, _: bool) -> Result<Vec<u8>> {
            Ok(vec![7; 32])
        }
    }
    fn worker(path: &std::path::Path) -> Worker {
        let mut state = State::default();
        let proxy = subscription::parse_vless(
            "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:12345#Fixture",
        )
        .unwrap();
        state.replace_source("fixture", vec![proxy]);
        state.selected = Some(state.servers[0].id.clone());
        state.subscriptions.push(Subscription {
            id: "fixture".into(),
            name: "Fixture".into(),
            url: "https://127.0.0.1:1/private-token".into(),
            updated: Some(1),
            status: "Previous success".into(),
            allow_unverified_tls: false,
        });
        let (updates, _) = mpsc::channel();
        Worker {
            vault: Vault::new(path.join("vault.enc"), Box::new(Secrets)),
            snapshot: Snapshot {
                state,
                ..Default::default()
            },
            engine: Mihomo::new(path.join("missing-core"), String::new()),
            updates,
            ctx: eframe::egui::Context::default(),
        }
    }
    #[test]
    fn native_shutdown_joins_worker_and_remains_safe_when_handle_drops() {
        let dir = tempfile::tempdir().unwrap();
        let handle = Handle::spawn(
            Vault::new(dir.path().join("catalog.enc"), Box::new(Secrets)),
            eframe::egui::Context::default(),
        );
        handle.rx.recv_timeout(Duration::from_secs(2)).unwrap();
        let shutdown = handle.shutdown.clone();
        assert!(!shutdown.requested());
        shutdown.stop();
        assert!(shutdown.requested());
        assert!(handle.tx.send(Action::Start).is_err());
        drop(handle);
        shutdown.stop();
    }
    #[tokio::test]
    async fn failed_subscription_update_keeps_previous_servers_selection_and_success_time() {
        let dir = tempfile::tempdir().unwrap();
        let mut worker = worker(dir.path());
        let before = serde_json::to_value(&worker.snapshot.state.servers).unwrap();
        let selected = worker.snapshot.state.selected.clone();
        let error = worker
            .execute(Action::Update("fixture".into()))
            .await
            .unwrap_err();
        assert!(!error.to_string().contains("private-token"));
        assert_eq!(
            serde_json::to_value(&worker.snapshot.state.servers).unwrap(),
            before
        );
        assert_eq!(worker.snapshot.state.selected, selected);
        assert_eq!(worker.snapshot.state.subscriptions[0].updated, Some(1));
        assert_eq!(worker.vault.load().unwrap().selected, selected);
    }
    #[tokio::test]
    async fn removing_selected_subscription_persists_an_explicit_missing_selection() {
        let dir = tempfile::tempdir().unwrap();
        let mut worker = worker(dir.path());
        let selected = worker.snapshot.state.selected.clone();
        let error = worker
            .execute(Action::RemoveSubscription("fixture".into()))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("selected server was removed"));
        assert_eq!(worker.snapshot.state.selected, selected);
        assert!(worker.snapshot.state.selected_server().is_none());
        assert!(worker.vault.load().unwrap().selected_server().is_none());
        assert!(!worker.engine.running());
    }
}
