use nokiri::{config, model::State, subscription};
#[test]
fn imports_common_protocol_urls_and_generates_strict_configs() {
    let urls = [
        "hy2://user%3Asecret@example.org:443?sni=example.org&obfs=salamander&obfs-password=opaque#Hysteria2",
        "hysteria://example.org:443?auth=secret&peer=example.org&upmbps=20&downmbps=100#Hysteria",
        "trojan://secret@example.org:443?sni=example.org#Trojan",
        "ss://YWVzLTEyOC1nY206c2VjcmV0@example.org:8388#Shadowsocks",
        "socks5://user:secret@example.org:1080#SOCKS",
        "https://user:secret@example.org:443#HTTPS",
    ];
    for url in urls {
        let proxies = subscription::parse(url)
            .unwrap_or_else(|e| panic!("scheme {}: {e}", url.split("://").next().unwrap()));
        let mut state = State::default();
        state.replace_source("fixture", proxies);
        state.selected = Some(state.servers[0].id.clone());
        let value: serde_yaml::Value =
            serde_yaml::from_str(&config::generate(&state, 19090, "fixture").unwrap()).unwrap();
        assert_eq!(value["rules"][0].as_str(), Some("MATCH,NOKIRI"));
        assert_eq!(
            value["proxy-groups"][0]["proxies"]
                .as_sequence()
                .unwrap()
                .len(),
            1
        );
    }
}
#[test]
fn accepts_vless_httpupgrade_xhttp_and_websocket_early_data() {
    for query in [
        "type=httpupgrade&security=tls&host=example.org&path=%2Fupgrade",
        "type=xhttp&security=tls&host=example.org&path=%2Fx&mode=packet-up",
        "type=ws&security=tls&path=%2Fws%3Fed%3D2048",
    ] {
        let url =
            format!("vless://00000000-0000-4000-8000-000000000001@example.org:443?{query}#Fixture");
        assert!(subscription::parse(&url).is_ok(), "{query}");
    }
}

#[test]
fn preserves_vmess_data_and_hysteria_passwords_port_hopping_and_tls_policy() {
    use base64::Engine;
    let raw = r#"{"v":"2","ps":"VMess fixture","add":"example.org","port":"443","id":"00000000-0000-4000-8000-000000000001","aid":"0","scy":"auto","net":"ws","host":"example.org","path":"/tunnel","tls":"tls","sni":"example.org"}"#;
    let url = format!(
        "vmess://{}",
        base64::engine::general_purpose::STANDARD.encode(raw)
    );
    let p = subscription::parse(&url).unwrap().remove(0);
    assert_eq!(p.kind, "vmess");
    assert_eq!(p.options["alterId"], 0);
    assert_eq!(p.ws_opts.unwrap().path, "/tunnel");
    let p = subscription::parse(
        "hy2://user%3Asecret@example.org:443,8443-8450?sni=example.org&insecure=1",
    )
    .unwrap()
    .remove(0);
    assert_eq!(p.options["password"], "user:secret");
    assert_eq!(p.options["ports"], "443,8443-8450");
    assert!(subscription::require_verified_tls(std::slice::from_ref(&p), false).is_err());
    assert!(subscription::require_verified_tls(std::slice::from_ref(&p), true).is_ok());
    let mut pinned = p;
    pinned
        .options
        .insert("fingerprint".into(), "11".repeat(32).into());
    assert!(subscription::require_verified_tls(&[pinned], false).is_ok());
}
#[test]
fn rejects_untrusted_settings_and_bad_protocol_inputs_without_leaking_credentials() {
    for input in [
        "hy2://hidden-password@example.org:443?obfs=salamander",
        "hy2://hidden-password@example.org:443-0",
        "ss://hidden-password@example.org:8388?plugin=exec",
        "vless://00000000-0000-4000-8000-000000000001@example.org:443?type=xhttp&extra=%7B%22downloadSettings%22%3A%7B%22server%22%3A%22hidden-password%22%7D%7D",
        "proxies:\n- {name: test, type: hysteria2, server: example.org, port: 443, password: hidden-password, dialer-proxy: DIRECT}",
    ] {
        let error = subscription::parse(input)
            .err()
            .expect("unsafe profile must fail")
            .to_string();
        assert!(!error.contains("hidden-password"));
        assert!(!error.contains("00000000-0000"));
    }
}

#[tokio::test]
#[ignore = "Requires the verified official Mihomo executable"]
async fn official_core_accepts_every_supported_protocol_and_extended_transport() {
    use nokiri::engine::{self, Mihomo};
    let mut urls = vec![
        "hy2://secret@example.org:443?obfs=salamander&obfs-password=opaque".to_string(),
        "hysteria://example.org:443?auth=secret&upmbps=20&downmbps=100".to_string(),
        "trojan://secret@example.org:443?peer=example.org".to_string(),
        "ss://YWVzLTEyOC1nY206c2VjcmV0@example.org:8388".to_string(),
        "socks5://user:secret@example.org:1080".to_string(),
        "https://user:secret@example.org:443".to_string(),
    ];
    use base64::Engine;
    urls.push(format!("vmess://{}",base64::engine::general_purpose::STANDARD.encode(r#"{"v":"2","add":"example.org","port":"443","id":"00000000-0000-4000-8000-000000000001","net":"ws","path":"/tunnel","tls":"tls"}"#)));
    for query in [
        "type=httpupgrade&security=tls&path=%2Fupgrade",
        "type=xhttp&security=tls&path=%2Fx&mode=packet-up",
        "type=ws&security=tls&ed=2048",
        "type=h2&security=tls&host=example.org&path=%2Fh2",
        "type=tcp&headerType=http&host=example.org&path=%2F",
    ] {
        urls.push(format!(
            "vless://00000000-0000-4000-8000-000000000001@example.org:443?{query}"
        ));
    }
    let core = Mihomo::new(
        engine::core_path().unwrap(),
        engine::manifest().binary_sha256,
    );
    let dir = tempfile::tempdir().unwrap();
    for url in urls {
        let mut state = State::default();
        state.replace_source("fixture", subscription::parse(&url).unwrap());
        state.selected = Some(state.servers[0].id.clone());
        let path = dir.path().join("config.yaml");
        std::fs::write(&path, config::generate(&state, 19090, "fixture").unwrap()).unwrap();
        core.validate_config(&path, dir.path())
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", state.servers[0].proxy.description()));
        if state.servers[0].proxy.kind == "http" {
            assert_eq!(state.servers[0].proxy.engine_value()["tls"], true);
        }
    }
}

#[test]
fn existing_vless_catalog_identity_and_mixed_subscriptions_remain_compatible() {
    use base64::Engine;
    use sha2::{Digest, Sha256};
    let input = "vless://00000000-0000-4000-8000-000000000001@example.org:443?type=ws&security=tls&path=%2Ftunnel#Old";
    let p = subscription::parse(input).unwrap().remove(0);
    let mut original = serde_json::to_value(&p).unwrap();
    original["name"] = "".into();
    assert!(original["ws-opts"].get("v2ray-http-upgrade").is_none());
    let old: nokiri::model::Proxy = serde_json::from_value(original.clone()).unwrap();
    assert_eq!(p.identity("manual"), old.identity("manual"));
    // The original identity hashes source and the field-ordered serialized proxy.
    let expected = hex::encode(Sha256::digest(format!(
        "manual:{}",
        serde_json::to_string(&old).unwrap()
    )));
    assert_eq!(p.identity("manual"), expected);
    let mixed = format!("{input}\nhy2://secret@example.org:443");
    assert_eq!(
        subscription::parse(&base64::engine::general_purpose::STANDARD.encode(mixed))
            .unwrap()
            .len(),
        2
    );
}
