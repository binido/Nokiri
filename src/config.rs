use crate::{model::State, subscription};
use anyhow::{Result, bail};
use serde_json::json;

pub const SELECTOR: &str = "NOKIRI";
pub fn generate(state: &State, controller_port: u16, secret: &str) -> Result<String> {
    generate_inner(state, controller_port, secret, false)
}

pub fn generate_probe(state: &State, controller_port: u16, secret: &str) -> Result<String> {
    generate_inner(state, controller_port, secret, true)
}

fn generate_inner(
    state: &State,
    controller_port: u16,
    secret: &str,
    probe: bool,
) -> Result<String> {
    let selected = if probe {
        None
    } else {
        Some(state.selected_server().ok_or_else(|| {
            anyhow::anyhow!(
                "The selected server is missing. Select a valid server before starting the proxy."
            )
        })?)
    };
    if state.settings.port == 0 || state.settings.port == controller_port {
        bail!("Proxy port must be 1 to 65535 and different from the controller port.");
    }
    let mut proxies = Vec::new();
    for server in &state.servers {
        subscription::validate(&server.proxy)?;
        let mut proxy = server.proxy.clone();
        proxy.name = server.id.clone();
        proxies.push(proxy);
    }
    let groups = selected.map_or_else(Vec::new, |s| {
        vec![json!({ "name": SELECTOR, "type": "select", "proxies": [s.id] })]
    });
    let config = json!({
        "mixed-port": if probe { 0 } else { state.settings.port },
        "bind-address": "127.0.0.1", "allow-lan": false,
        "mode": "rule", "log-level": "warning", "ipv6": false,
        "external-controller": format!("127.0.0.1:{controller_port}"),
        "secret": secret,
        "profile": { "store-selected": false, "store-fake-ip": false },
        "tun": { "enable": false }, "dns": { "enable": false },
        "geodata-mode": false, "geo-auto-update": false,
        "proxies": proxies,
        "proxy-groups": groups,
        "rules": [if probe { "MATCH,REJECT".into() } else { format!("MATCH,{SELECTOR}") }]
    });
    Ok(serde_yaml::to_string(&config)?)
}
