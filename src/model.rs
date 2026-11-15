use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Proxy {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub server: String,
    pub port: u16,
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
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Grpc {
    pub grpc_service_name: String,
}

impl Proxy {
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
}
#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct State {
    pub servers: Vec<Server>,
    pub subscriptions: Vec<Subscription>,
    pub selected: Option<String>,
    pub settings: Settings,
}
#[derive(Clone, Serialize, Deserialize)]
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
