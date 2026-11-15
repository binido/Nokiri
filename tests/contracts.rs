use nokiri::{
    config,
    engine::{self, Mihomo, ProxyEngine},
    model::{Proxy, State},
    platform::{SecretStore, Vault},
    subscription,
};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};

fn proxy() -> Proxy {
    subscription::parse_vless("vless://00000000-0000-4000-8000-000000000001@127.0.0.1:12345?security=none&type=tcp#Example").unwrap()
}
fn state() -> State {
    let mut state = State::default();
    state.replace_source("fixture", vec![proxy()]);
    state.selected = Some(state.servers[0].id.clone());
    state.settings.port = free_port();
    state
}
fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
struct TestSecrets;
impl SecretStore for TestSecrets {
    fn key(&self, _: bool) -> anyhow::Result<Vec<u8>> {
        Ok(vec![42; 32])
    }
}

#[test]
fn accepts_documented_vless_transports() {
    for parameters in [
        "security=tls&type=tcp&sni=example.org&flow=xtls-rprx-vision",
        "security=tls&type=ws&host=example.org&path=%2Fproxy",
        "security=tls&type=grpc&serviceName=tunnel",
        "security=reality&type=tcp&pbk=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&sid=abcd&fp=chrome",
    ] {
        let url = format!(
            "vless://00000000-0000-4000-8000-000000000001@example.org:443?{parameters}#My%20server"
        );
        let parsed = subscription::parse_vless(&url).unwrap();
        assert!(parsed.tls);
        assert_eq!(parsed.name, "My server");
    }
    let parsed = subscription::parse_vless(
        "vless://00000000-0000-4000-8000-000000000001@[::1]:443?security=tls#IPv6",
    )
    .unwrap();
    assert_eq!(parsed.server, "::1");
}
#[test]
fn rejects_invalid_or_unsupported_imports_without_echoing_secrets() {
    for input in [
        "",
        "not a subscription",
        "dm1lc3M6Ly9zZWNyZXQ=",
        "proxies: []",
        "vless://secret@example.org:443",
        "vless://00000000-0000-4000-8000-000000000001@example.org:443?type=xhttp",
        "vless://00000000-0000-4000-8000-000000000001@example.org:443?security=tls&allowInsecure=1",
        "vless://00000000-0000-4000-8000-000000000001@example.org:443?type=ws&flow=xtls-rprx-vision",
        "vless://00000000-0000-4000-8000-000000000001@example.org:443?type=tcp&type=ws",
    ] {
        let error = subscription::parse(input).err().expect(input).to_string();
        assert!(!error.contains("secret@"));
        assert!(!error.contains("00000000-0000"));
    }
}
#[test]
fn parses_yaml_and_base64_but_rejects_proxy_routing_overrides() {
    use base64::Engine;
    let yaml = format!(
        "proxies:\n{}\nrules: [MATCH,DIRECT]\nproxy-groups: []\nexternal-controller: 0.0.0.0:9999",
        serde_yaml::to_string(&vec![proxy()]).unwrap()
    );
    assert_eq!(subscription::parse(&yaml).unwrap().len(), 1);
    let hostile = yaml.replace("  type: vless", "  dialer-proxy: DIRECT\n  type: vless");
    assert!(subscription::parse(&hostile).is_err());
    let urls = "vless://00000000-0000-4000-8000-000000000001@127.0.0.1:12345#Local";
    assert_eq!(
        subscription::parse(&base64::engine::general_purpose::STANDARD.encode(urls))
            .unwrap()
            .len(),
        1
    );
}
#[test]
fn identity_survives_rename_but_disambiguates_duplicate_names_and_sources() {
    let mut state = state();
    let selected = state.selected.clone();
    let mut renamed = proxy();
    renamed.name = "Renamed".into();
    let mut other = proxy();
    other.port = 12346;
    state.replace_source("fixture", vec![renamed, other.clone(), other.clone()]);
    assert_eq!(state.selected, selected);
    assert_eq!(state.selected_server().unwrap().proxy.name, "Renamed");
    assert_eq!(state.servers.len(), 2);
    state.replace_source("second", vec![other]);
    assert_eq!(state.servers.len(), 3);
    let ids: std::collections::HashSet<_> = state.servers.iter().map(|s| &s.id).collect();
    assert_eq!(ids.len(), 3);
    state.replace_source("fixture", vec![]);
    assert_eq!(state.selected, selected);
    assert!(state.selected_server().is_none());
    assert!(config::generate(&state, free_port(), "test").is_err());
}
#[test]
fn encrypted_catalog_restores_fixed_selection_and_settings_and_detects_tampering() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("catalog.enc");
    let vault = Vault::new(path.clone(), Box::new(TestSecrets));
    let mut original = state();
    original.settings.port = 1234;
    vault.save(&original).unwrap();
    let restored = vault.load().unwrap();
    assert_eq!(restored.selected, original.selected);
    assert_eq!(restored.settings.port, 1234);
    let bytes = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains(&original.servers[0].proxy.uuid));
    let mut corrupted = bytes;
    corrupted[15] ^= 1;
    std::fs::write(path, corrupted).unwrap();
    assert!(vault.load().is_err());
}
#[test]
fn generated_config_has_one_fixed_route_and_no_direct_or_server_fallback() {
    let mut state = state();
    let selected = state.selected.clone().unwrap();
    let mut other = proxy();
    other.port += 1;
    state.replace_source("other", vec![other]);
    let yaml = config::generate(&state, free_port(), "secret").unwrap();
    let config: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(config["rules"].as_sequence().unwrap().len(), 1);
    assert_eq!(config["rules"][0].as_str(), Some("MATCH,NOKIRI"));
    let group = &config["proxy-groups"][0];
    assert_eq!(group["type"].as_str(), Some("select"));
    assert_eq!(group["proxies"].as_sequence().unwrap().len(), 1);
    assert_eq!(group["proxies"][0].as_str(), Some(selected.as_str()));
    assert!(!yaml.contains("DIRECT"));
    assert!(!yaml.contains("fallback"));
    assert!(!yaml.contains("url-test"));
    assert_eq!(config["bind-address"].as_str(), Some("127.0.0.1"));
    assert_eq!(config["allow-lan"].as_bool(), Some(false));
    assert!(config.get("proxy-providers").is_none());
    assert!(config.get("rule-providers").is_none());
}
fn fake_core(script: &str) -> (tempfile::TempDir, PathBuf, String) {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("mihomo");
    std::fs::write(&path, script).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let hash = hex::encode(Sha256::digest(script.as_bytes()));
    (dir, path, hash)
}
#[tokio::test]
async fn core_rejects_invalid_configuration_missing_binary_and_checksum_mismatch() {
    let (dir, path, hash) = fake_core("#!/bin/sh\nexit 1\n");
    let core = Mihomo::new(path.clone(), hash);
    let config = dir.path().join("config.yaml");
    std::fs::write(&config, "broken").unwrap();
    assert!(
        core.validate_config(&config, dir.path())
            .await
            .unwrap_err()
            .to_string()
            .contains("rejected")
    );
    assert!(engine::verify_binary(&path, "incorrect").await.is_err());
    assert!(
        engine::verify_binary(&dir.path().join("missing"), "incorrect")
            .await
            .is_err()
    );
}
#[tokio::test]
async fn failed_startup_cleans_up_and_port_conflict_is_explicit() {
    let (_dir, path, hash) = fake_core(
        "#!/bin/sh\nif [ \"$1\" = \"-t\" ]; then exit 0; fi\nif [ \"$1\" = \"-v\" ]; then echo fixture; exit 0; fi\nexit 7\n",
    );
    let mut core = Mihomo::new(path, hash);
    let state = state();
    assert!(core.start(&state).await.is_err());
    assert!(!core.running());
    assert!(core.controller.is_none());
    let listener = std::net::TcpListener::bind(("127.0.0.1", state.settings.port)).unwrap();
    assert!(
        core.start(&state)
            .await
            .unwrap_err()
            .to_string()
            .contains("port is in use")
    );
    drop(listener);
    core.stop().await.unwrap();
}
#[tokio::test]
async fn explicit_proxy_never_uses_direct_when_proxy_is_unreachable() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let response = engine::proxy_client(port, Duration::from_millis(200))
        .unwrap()
        .get("https://example.org")
        .send()
        .await;
    assert!(response.is_err());
}
#[tokio::test]
async fn proxied_connectivity_has_a_real_timeout() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let stall = tokio::spawn(async move {
        let (_stream, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(10)).await;
    });
    let start = std::time::Instant::now();
    let result = engine::proxy_client(port, Duration::from_millis(100))
        .unwrap()
        .get("https://example.org")
        .send()
        .await;
    assert!(result.unwrap_err().is_timeout());
    assert!(start.elapsed() < Duration::from_secs(2));
    stall.abort();
}

#[test]
fn testing_without_selection_opens_no_proxy_listener_and_rejects_routing() {
    let mut state = state();
    state.selected = None;
    let value: serde_yaml::Value =
        serde_yaml::from_str(&config::generate_probe(&state, free_port(), "probe").unwrap())
            .unwrap();
    assert_eq!(value["mixed-port"].as_u64(), Some(0));
    assert!(value["proxy-groups"].as_sequence().unwrap().is_empty());
    assert_eq!(value["rules"][0].as_str(), Some("MATCH,REJECT"));
    assert_eq!(value["proxies"].as_sequence().unwrap().len(), 1);
}
