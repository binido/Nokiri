use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub struct Proxy {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub server: String,
    pub port: u16,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub tls: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub servername: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub flow: String,
    #[serde(default = "tcp")]
    pub network: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub client_fingerprint: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub alpn: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reality_opts: Option<Reality>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ws_opts: Option<WebSocket>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc_opts: Option<Grpc>,
    #[serde(default)]
    pub udp: bool,
    #[serde(default = "xudp")]
    pub packet_encoding: String,
    #[serde(default, flatten)]
    pub options: BTreeMap<String, serde_json::Value>,
}
fn tcp() -> String {
    "tcp".into()
}
fn xudp() -> String {
    "xudp".into()
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Reality {
    pub public_key: String,
    #[serde(default)]
    pub short_id: String,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WebSocket {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    #[serde(default)]
    pub max_early_data: u32,
    #[serde(default)]
    pub early_data_header_name: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub v2ray_http_upgrade: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub v2ray_http_upgrade_fast_open: bool,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Grpc {
    pub grpc_service_name: String,
}

impl Proxy {
    pub fn new(kind: &str, server: String, port: u16, name: String) -> Self {
        serde_json::from_value(
            serde_json::json!({"type":kind,"server":server,"port":port,"name":name}),
        )
        .unwrap()
    }
    pub fn unverified_tls(&self) -> bool {
        self.options
            .get("skip-cert-verify")
            .and_then(|v| v.as_bool())
            == Some(true)
            && self
                .options
                .get("fingerprint")
                .and_then(|v| v.as_str())
                .is_none_or(str::is_empty)
    }
    pub fn engine_value(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap();
        let fields = value.as_object_mut().unwrap();
        if !["vless", "vmess"].contains(&self.kind.as_str()) {
            fields.remove("uuid");
            fields.remove("packet-encoding");
        }
        if self.kind != "vless" {
            fields.remove("flow");
            fields.remove("reality-opts");
        }
        if !["vless", "vmess", "trojan"].contains(&self.kind.as_str()) {
            for key in ["network", "ws-opts", "grpc-opts", "client-fingerprint"] {
                fields.remove(key);
            }
        }
        if !["vless", "vmess", "trojan", "http", "socks5"].contains(&self.kind.as_str()) {
            fields.remove("tls");
        }
        if self.kind != "vless" {
            fields.remove("packet-encoding");
        }
        if !["vless", "vmess"].contains(&self.kind.as_str()) {
            fields.remove("servername");
            if !self.servername.is_empty() {
                fields.insert("sni".into(), self.servername.clone().into());
            }
        }
        value
    }
    pub fn description(&self) -> String {
        let protocol = match self.kind.as_str() {
            "hysteria2" => "Hysteria 2",
            "hysteria" => "Hysteria",
            "ss" => "Shadowsocks",
            "vmess" => "VMess",
            "vless" => "VLESS",
            "trojan" => "Trojan",
            "http" => "HTTP",
            "socks5" => "SOCKS5",
            _ => &self.kind,
        };
        let transport = if ["vless", "vmess", "trojan"].contains(&self.kind.as_str()) {
            format!(" {}", self.network.to_uppercase())
        } else {
            String::new()
        };
        format!(
            "{protocol}{transport}{}",
            if self.reality_opts.is_some() {
                " + REALITY"
            } else if self.tls {
                " + TLS"
            } else {
                ""
            }
        )
    }
    pub fn identity(&self, source: &str) -> String {
        let mut canonical = self.clone();
        canonical.name.clear();
        hex::encode(Sha256::digest(format!(
            "{source}:{}",
            serde_json::to_string(&canonical).unwrap()
        )))
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Server {
    pub id: String,
    pub source: String,
    pub proxy: Proxy,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Subscription {
    pub id: String,
    pub name: String,
    pub url: String,
    pub updated: Option<u64>,
    pub status: String,
    #[serde(default)]
    pub allow_unverified_tls: bool,
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    pub servers: Vec<Server>,
    pub subscriptions: Vec<Subscription>,
    pub selected: Option<String>,
    pub settings: Settings,
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Settings {
    pub port: u16,
    pub dark: bool,
    pub start_on_launch: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            port: 7890,
            dark: true,
            start_on_launch: false,
        }
    }
}
impl State {
    pub fn selected_server(&self) -> Option<&Server> {
        self.selected
            .as_ref()
            .and_then(|id| self.servers.iter().find(|s| &s.id == id))
    }
    pub fn replace_source(&mut self, source: &str, proxies: Vec<Proxy>) {
        self.servers.retain(|s| s.source != source);
        for proxy in proxies {
            let id = proxy.identity(source);
            if !self.servers.iter().any(|s| s.id == id) {
                self.servers.push(Server {
                    id,
                    source: source.into(),
                    proxy,
                });
            }
        }
    }
    pub fn source_name(&self, source: &str) -> &str {
        self.subscriptions
            .iter()
            .find(|s| s.id == source)
            .map_or("Manual import", |s| s.name.as_str())
    }
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
