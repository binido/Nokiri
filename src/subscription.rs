use crate::model::{Grpc, Proxy, Reality, WebSocket};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use percent_encoding::percent_decode_str;
use std::collections::BTreeMap;
use thiserror::Error;
use url::Url;

pub const MAX_BYTES: usize = 4 * 1024 * 1024;
#[derive(Debug, Error)]
#[error("{0}")]
pub struct ImportError(pub String);
type Result<T> = std::result::Result<T, ImportError>;
fn bad(message: &str) -> ImportError {
    ImportError(message.into())
}

pub fn parse(input: &str) -> Result<Vec<Proxy>> {
    if input.len() > MAX_BYTES {
        return Err(bad("Subscription exceeds the 4 MiB limit."));
    }
    let input = input.trim_start_matches('\u{feff}').trim();
    if input.starts_with("vless://")
        || input.lines().any(|l| l.contains("://")) && !input.contains("proxies:")
    {
        return parse_lines(input);
    }
    if let Ok(value) = serde_yaml::from_str::<serde_yaml::Value>(input)
        && let Some(items) = value.get("proxies").and_then(|v| v.as_sequence())
    {
        if items.is_empty() || items.len() > 5000 {
            return Err(bad("Expected 1 to 5000 proxy entries."));
        }
        return items.iter().enumerate().map(|(i, v)| {
                let proxy: Proxy = serde_yaml::from_value(v.clone()).map_err(|_| bad(&format!("Proxy {} has unsupported fields or protocol. Only VLESS TCP, WebSocket and gRPC are supported; insecure TLS and routing overrides are rejected.", i + 1)))?;
                validate(&proxy)?;
                Ok(proxy)
            }).collect();
    }
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    for encoding in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(bytes) = encoding.decode(&compact)
            && let Ok(decoded) = String::from_utf8(bytes)
        {
            return parse_lines(&decoded);
        }
    }
    Err(bad(
        "Expected Clash YAML with a proxies list, Base64 proxy URLs, or a VLESS URL.",
    ))
}
fn parse_lines(input: &str) -> Result<Vec<Proxy>> {
    let lines: Vec<_> = input
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() || lines.len() > 5000 {
        return Err(bad("Expected 1 to 5000 VLESS URLs."));
    }
    lines
        .into_iter()
        .enumerate()
        .map(|(i, l)| parse_vless(l).map_err(|e| bad(&format!("Entry {}: {}", i + 1, e.0))))
        .collect()
}

pub fn parse_vless(input: &str) -> Result<Proxy> {
    let u = Url::parse(input).map_err(|_| bad("Invalid VLESS URL."))?;
    if u.scheme() != "vless" {
        return Err(bad(
            "Unsupported protocol. This release imports VLESS only.",
        ));
    }
    if u.password().is_some() {
        return Err(bad("VLESS URLs must contain a UUID without a password."));
    }
    let mut q = BTreeMap::new();
    for (k, v) in u.query_pairs() {
        if q.insert(k.to_string(), v.to_string()).is_some() {
            return Err(bad("Duplicate VLESS parameters are not supported."));
        }
    }
    let allowed = [
        "encryption",
        "security",
        "type",
        "sni",
        "serverName",
        "fp",
        "alpn",
        "pbk",
        "sid",
        "spx",
        "flow",
        "host",
        "path",
        "serviceName",
        "authority",
        "mode",
        "headerType",
        "allowInsecure",
        "insecure",
    ];
    if q.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(bad(
            "Unsupported VLESS parameter. Remove unsupported transport extensions before importing.",
        ));
    }
    let get = |k: &str| q.get(k).map(String::as_str).unwrap_or("");
    if !["", "none"].contains(&get("encryption")) {
        return Err(bad("Only VLESS encryption=none is supported."));
    }
    if !["", "none"].contains(&get("headerType"))
        || !get("authority").is_empty()
        || !["", "gun"].contains(&get("mode"))
    {
        return Err(bad(
            "HTTP camouflage, gRPC authority and multi mode are not supported.",
        ));
    }
    if ["allowInsecure", "insecure"]
        .iter()
        .any(|k| !["", "0", "false"].contains(&get(k)))
    {
        return Err(bad(
            "Disabling TLS certificate verification is not supported.",
        ));
    }
    let security = get("security");
    if !["", "none", "tls", "reality"].contains(&security) {
        return Err(bad("Unsupported VLESS security mode."));
    }
    let network = if get("type").is_empty() {
        "tcp"
    } else {
        get("type")
    };
    let host = u
        .host_str()
        .ok_or_else(|| bad("Missing server host."))?
        .trim_matches(['[', ']'])
        .to_string();
    let name = percent_decode_str(u.fragment().unwrap_or("Imported server"))
        .decode_utf8()
        .map_err(|_| bad("Invalid UTF-8 in server name."))?
        .to_string();
    let mut headers = BTreeMap::new();
    if !get("host").is_empty() {
        headers.insert("Host".into(), get("host").into());
    }
    let proxy = Proxy {
        name,
        kind: "vless".into(),
        server: host,
        port: u.port().ok_or_else(|| bad("Missing server port."))?,
        uuid: percent_decode_str(u.username())
            .decode_utf8()
            .map_err(|_| bad("Invalid UUID encoding."))?
            .to_string(),
        tls: ["tls", "reality"].contains(&security),
        servername: if get("sni").is_empty() {
            get("serverName")
        } else {
            get("sni")
        }
        .into(),
        flow: get("flow").into(),
        network: network.into(),
        client_fingerprint: if security == "reality" && get("fp").is_empty() {
            "chrome"
        } else {
            get("fp")
        }
        .into(),
        alpn: get("alpn")
            .split(',')
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect(),
        reality_opts: (security == "reality").then(|| Reality {
            public_key: get("pbk").into(),
            short_id: get("sid").into(),
        }),
        ws_opts: (network == "ws").then(|| WebSocket {
            path: if get("path").is_empty() {
                "/"
            } else {
                get("path")
            }
            .into(),
            headers,
            max_early_data: 0,
            early_data_header_name: String::new(),
        }),
        grpc_opts: (network == "grpc").then(|| Grpc {
            grpc_service_name: get("serviceName").into(),
        }),
        udp: false,
        packet_encoding: "xudp".into(),
    };
    validate(&proxy)?;
    Ok(proxy)
}

pub fn validate(p: &Proxy) -> Result<()> {
    if p.kind != "vless" {
        return Err(bad(
            "Unsupported proxy protocol. Only VLESS is supported in this release.",
        ));
    }
    if p.name.trim().is_empty() || p.name.len() > 256 || p.name.chars().any(char::is_control) {
        return Err(bad("Server names must contain 1 to 256 printable bytes."));
    }
    if p.server.is_empty()
        || p.server.len() > 253
        || p.server
            .chars()
            .any(|c| c.is_whitespace() || c.is_control() || matches!(c, '/' | '@' | '#' | '?'))
        || p.port == 0
    {
        return Err(bad("Invalid server host or port."));
    }
    uuid::Uuid::parse_str(&p.uuid).map_err(|_| bad("VLESS requires a valid UUID."))?;
    if !["tcp", "ws", "grpc"].contains(&p.network.as_str()) {
        return Err(bad(
            "Unsupported transport. Supported transports are TCP, WebSocket and gRPC.",
        ));
    }
    if !["", "xtls-rprx-vision"].contains(&p.flow.as_str())
        || !p.flow.is_empty() && (!p.tls || p.network != "tcp")
    {
        return Err(bad("Vision requires TCP with TLS or REALITY."));
    }
    if !["", "xudp", "packetaddr"].contains(&p.packet_encoding.as_str()) {
        return Err(bad("Unsupported UDP packet encoding."));
    }
    if p.ws_opts.is_some() != (p.network == "ws") || p.grpc_opts.is_some() != (p.network == "grpc")
    {
        return Err(bad(
            "Transport options do not match the selected transport.",
        ));
    }
    if let Some(ws) = &p.ws_opts {
        if ws.max_early_data != 0
            || !ws.early_data_header_name.is_empty()
            || ws.path.contains('?') && ws.path.contains("ed=")
        {
            return Err(bad(
                "WebSocket early data is not supported in this release.",
            ));
        }
        if ws.headers.keys().any(|h| h != "Host") {
            return Err(bad("Only the WebSocket Host header is supported."));
        }
    }
    if let Some(r) = &p.reality_opts
        && (!p.tls
            || p.client_fingerprint.is_empty()
            || URL_SAFE_NO_PAD
                .decode(&r.public_key)
                .map_or(true, |b| b.len() != 32)
            || r.short_id.len() > 16
            || r.short_id.len() % 2 != 0
            || !r.short_id.chars().all(|c| c.is_ascii_hexdigit()))
    {
        return Err(bad(
            "REALITY requires TLS, a fingerprint, a 32-byte public key and an even hexadecimal short ID.",
        ));
    }
    Ok(())
}

pub async fn fetch(raw_url: &str) -> Result<Vec<Proxy>> {
    let url = Url::parse(raw_url).map_err(|_| bad("Invalid subscription URL."))?;
    if url.scheme() != "https" || url.host_str().is_none() {
        return Err(bad("Subscriptions require HTTPS."));
    }
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(20))
        .user_agent("Nokiri/0.1")
        .build()
        .map_err(|_| bad("Could not initialize HTTPS client."))?;
    let mut response = client.get(url).send().await.map_err(|_| {
        bad("Subscription request failed or timed out. Check the URL and TLS certificate.")
    })?;
    if !response.status().is_success() {
        return Err(bad(&format!(
            "Subscription returned HTTP {}. Redirects are disabled to protect credentials.",
            response.status().as_u16()
        )));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| bad("Could not read subscription response."))?
    {
        if bytes.len() + chunk.len() > MAX_BYTES {
            return Err(bad("Subscription exceeds the 4 MiB limit."));
        }
        bytes.extend_from_slice(&chunk);
    }
    let input = String::from_utf8(bytes).map_err(|_| bad("Subscription is not UTF-8."))?;
    parse(&input)
}
