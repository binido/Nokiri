use nokiri::{
    config,
    engine::{self, Mihomo, ProxyEngine},
    model::State,
    subscription,
};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::{Child, Command};

fn port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
async fn ready(port: u16) {
    for _ in 0..100 {
        if tokio::net::TcpStream::connect(("127.0.0.1", port))
            .await
            .is_ok()
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("Fixture did not become ready on port {port}");
}
async fn vless_fixture(binary: &Path, dir: &Path, port: u16, transport: &str) -> Child {
    let listener = serde_json::json!({"name":"fixture","type":"vless","port":port,"listen":"127.0.0.1","allow-insecure":true,"users":[{"username":"fixture","uuid":"00000000-0000-4000-8000-000000000001"}]});
    let mut configuration = serde_json::json!({"listeners":[listener],"rules":["MATCH,DIRECT"],"log-level":"silent","geo-auto-update":false});
    if transport == "ws" {
        configuration["listeners"][0]["ws-path"] = "/tunnel".into();
    }
    if transport == "grpc" {
        configuration["listeners"][0]["grpc-service-name"] = "tunnel".into();
    }
    let file = dir.join(format!("server-{port}.yaml"));
    std::fs::write(&file, serde_yaml::to_string(&configuration).unwrap()).unwrap();
    let mut child = Command::new(binary)
        .arg("-d")
        .arg(dir)
        .arg("-f")
        .arg(file)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    if let Some(status) = child.try_wait().unwrap() {
        panic!("VLESS fixture exited: {status}");
    }
    ready(port).await;
    child
}
fn catalog(server_port: u16, transport: &str) -> State {
    let url = format!(
        "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:{server_port}?security=none&type={transport}&path=%2Ftunnel&serviceName=tunnel#Local%20fixture"
    );
    let mut state = State::default();
    state.replace_source("fixture", vec![subscription::parse_vless(&url).unwrap()]);
    state.selected = Some(state.servers[0].id.clone());
    state.settings.port = port();
    state
}

#[tokio::test]
#[ignore = "Requires the verified official macOS ARM64 core, openssl and Python 3"]
async fn real_vless_https_failure_isolation_restart_and_unexpected_termination() {
    let binary = engine::core_path().unwrap();
    engine::verify_binary(&binary, &engine::manifest().binary_sha256)
        .await
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let cert = dir.path().join("cert.pem");
    let key = dir.path().join("key.pem");
    let status = Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "1",
            "-subj",
            "/CN=localhost",
            "-addext",
            "subjectAltName=IP:127.0.0.1,DNS:localhost",
            "-addext",
            "basicConstraints=critical,CA:FALSE",
            "-keyout",
        ])
        .arg(&key)
        .arg("-out")
        .arg(&cert)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .unwrap();
    assert!(status.success());
    let target_port = port();
    let script = dir.path().join("https.py");
    std::fs::write(
        &script,
        r#"import http.server,ssl,sys
class Handler(http.server.BaseHTTPRequestHandler):
 def do_GET(self):
  self.send_response(204 if self.path == '/204' else 200)
  self.send_header('Connection','close');self.end_headers()
  if self.path != '/204': self.wfile.write(b'127.0.0.1')
 def log_message(self,*args): pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',int(sys.argv[3])),Handler)
ctx=ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER);ctx.load_cert_chain(sys.argv[1],sys.argv[2])
server.socket=ctx.wrap_socket(server.socket,server_side=True);server.serve_forever()
"#,
    )
    .unwrap();
    let mut target = Command::new("python3")
        .arg(script)
        .arg(&cert)
        .arg(&key)
        .arg(target_port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    ready(target_port).await;
    let root = reqwest::Certificate::from_pem(&std::fs::read(&cert).unwrap()).unwrap();
    let target_url = format!("https://127.0.0.1:{target_port}/204");
    for transport in ["tcp", "ws", "grpc"] {
        let server_port = port();
        let backup_port = port();
        let mut upstream = vless_fixture(&binary, dir.path(), server_port, transport).await;
        let mut backup = vless_fixture(&binary, dir.path(), backup_port, transport).await;
        let mut state = catalog(server_port, transport);
        let extra = catalog(backup_port, transport).servers.remove(0).proxy;
        state.replace_source("backup", vec![extra]);
        let mut core = Mihomo::new(binary.clone(), engine::manifest().binary_sha256);
        core.start(&state)
            .await
            .unwrap_or_else(|e| panic!("{transport}: {e:?}\n{}", core.diagnostics()));
        let client = || {
            reqwest::Client::builder()
                .no_proxy()
                .proxy(
                    reqwest::Proxy::all(format!("http://127.0.0.1:{}", state.settings.port))
                        .unwrap(),
                )
                .add_root_certificate(root.clone())
                .timeout(Duration::from_secs(3))
                .build()
                .unwrap()
        };
        assert_eq!(
            client()
                .get(&target_url)
                .send()
                .await
                .unwrap_or_else(|e| panic!("{transport}: {e:?}\n{}", core.diagnostics()))
                .status(),
            204
        );
        let socks = reqwest::Client::builder()
            .no_proxy()
            .proxy(
                reqwest::Proxy::all(format!("socks5h://127.0.0.1:{}", state.settings.port))
                    .unwrap(),
            )
            .add_root_certificate(root.clone())
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap();
        assert_eq!(socks.get(&target_url).send().await.unwrap().status(), 204);
        let controller = core.controller.clone().unwrap();
        let group: serde_json::Value = controller
            .get(&format!("/proxies/{}", config::SELECTOR))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(group["now"].as_str(), state.selected.as_deref());
        assert_eq!(group["all"].as_array().unwrap().len(), 1);
        let mut unauthorized = controller.get("/version").build().unwrap();
        unauthorized
            .headers_mut()
            .remove(reqwest::header::AUTHORIZATION);
        assert_eq!(
            reqwest::Client::builder()
                .no_proxy()
                .build()
                .unwrap()
                .execute(unauthorized)
                .await
                .unwrap()
                .status(),
            401
        );
        core.stop().await.unwrap();
        core.start(&state).await.unwrap();
        assert_eq!(
            client().get(&target_url).send().await.unwrap().status(),
            204
        );
        upstream.kill().await.unwrap();
        upstream.wait().await.unwrap();
        assert!(
            client().get(&target_url).send().await.is_err(),
            "{transport}: dead selected server must fail even with a live backup and directly reachable destination"
        );
        let direct = reqwest::Client::builder()
            .no_proxy()
            .add_root_certificate(root.clone())
            .build()
            .unwrap();
        assert_eq!(direct.get(&target_url).send().await.unwrap().status(), 204);
        let selected = state.selected.clone().unwrap();
        assert_eq!(
            core.controller
                .as_ref()
                .unwrap()
                .get(&format!("/proxies/{}", config::SELECTOR))
                .send()
                .await
                .unwrap()
                .json::<serde_json::Value>()
                .await
                .unwrap()["now"]
                .as_str(),
            Some(selected.as_str())
        );
        let pid = core.pid().unwrap();
        Command::new("kill")
            .arg("-KILL")
            .arg(pid.to_string())
            .status()
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(
            core.poll()
                .unwrap_err()
                .to_string()
                .contains("unexpectedly")
        );
        assert!(!core.running());
        assert!(client().get(&target_url).send().await.is_err());
        core.stop().await.unwrap();
        backup.kill().await.unwrap();
        backup.wait().await.unwrap();
    }
    target.kill().await.unwrap();
    target.wait().await.unwrap();
}

#[tokio::test]
#[ignore = "Requires the verified official macOS ARM64 core"]
async fn official_core_rejects_invalid_config_and_validates_tls_transports() {
    let binary = engine::core_path().unwrap();
    let core = Mihomo::new(binary, engine::manifest().binary_sha256);
    let dir = tempfile::tempdir().unwrap();
    let path: PathBuf = dir.path().join("config.yaml");
    std::fs::write(&path, "proxies: [this-is-invalid]\n").unwrap();
    assert!(core.validate_config(&path, dir.path()).await.is_err());
    for params in [
        "security=tls&type=tcp&flow=xtls-rprx-vision",
        "security=tls&type=ws&path=%2Fproxy",
        "security=tls&type=grpc&serviceName=tunnel",
        "security=reality&type=tcp&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&fp=chrome&sid=abcd",
    ] {
        let mut state = catalog(12345, "tcp");
        let proxy = subscription::parse_vless(&format!(
            "vless://00000000-0000-4000-8000-000000000001@example.org:443?{params}#TLS"
        ))
        .unwrap();
        state.replace_source("fixture", vec![proxy]);
        state.selected = Some(state.servers[0].id.clone());
        std::fs::write(&path, config::generate(&state, port(), "fixture").unwrap()).unwrap();
        core.validate_config(&path, dir.path()).await.unwrap();
    }
}
