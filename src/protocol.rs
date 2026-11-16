use crate::model::Proxy;
use serde_json::Value;

pub fn validate_options(p: &Proxy) -> Result<(), &'static str> {
    let protocol: &[&str] = match p.kind.as_str() {
        "vless" => &["encryption", "xhttp-opts", "h2-opts", "http-opts"],
        "vmess" => &["alterId", "cipher", "h2-opts", "http-opts"],
        "trojan" => &["password"],
        "ss" => &["cipher", "password"],
        "hysteria2" => &[
            "password",
            "ports",
            "hop-interval",
            "up",
            "down",
            "obfs",
            "obfs-password",
            "obfs-min-packet-size",
            "obfs-max-packet-size",
        ],
        "hysteria" => &[
            "auth-str", "auth", "ports", "up", "down", "obfs", "protocol",
        ],
        "http" | "socks5" => &["username", "password"],
        _ => {
            return Err(
                "Unsupported protocol. Supported: VLESS, VMess, Trojan, Shadowsocks, Hysteria, Hysteria 2, HTTP and SOCKS5.",
            );
        }
    };
    if p.kind == "ss"
        && (p.tls
            || p.options.contains_key("fingerprint")
            || p.options.contains_key("skip-cert-verify"))
    {
        return Err("Shadowsocks TLS plugins are not supported.");
    }
    if p.kind == "socks5" && !p.servername.is_empty() {
        return Err(
            "SOCKS5 uses the server hostname for TLS; an SNI override is not supported by this core.",
        );
    }
    if !["vless", "vmess", "trojan"].contains(&p.kind.as_str()) && p.network != "tcp" {
        return Err("This protocol does not accept VLESS transport options.");
    }
    for (key, value) in &p.options {
        if !protocol.contains(&key.as_str())
            && !["skip-cert-verify", "fingerprint", "tfo"].contains(&key.as_str())
        {
            return Err(
                "Unsupported proxy field. Routing overrides, executable plugins and external file references are not accepted.",
            );
        }
        match key.as_str() {
            "skip-cert-verify" | "tfo" => {
                if !value.is_boolean() {
                    return Err("Expected a boolean proxy option.");
                }
            }
            "alterId" | "obfs-min-packet-size" | "obfs-max-packet-size" => {
                if value.as_u64().is_none_or(|n| n > 65535) {
                    return Err("Invalid numeric proxy option.");
                }
            }
            "up" | "down" | "hop-interval" => {
                if !(value.is_string() || value.as_u64().is_some()) {
                    return Err("Expected a bandwidth or interval value.");
                }
            }
            "xhttp-opts" => validate_object(
                value,
                &[
                    "path",
                    "host",
                    "mode",
                    "headers",
                    "no-grpc-header",
                    "x-padding-bytes",
                ],
            )?,
            "h2-opts" => validate_object(value, &["host", "path"])?,
            "http-opts" => validate_object(value, &["method", "path", "headers"])?,
            _ => {
                if !value.is_string() {
                    return Err("Expected a string proxy option.");
                }
            }
        }
    }
    let text = |key: &str| p.options.get(key).and_then(Value::as_str).unwrap_or("");
    if ["trojan", "ss", "hysteria2"].contains(&p.kind.as_str()) && text("password").is_empty() {
        return Err("This protocol requires a password.");
    }
    if p.kind == "ss" && text("cipher").is_empty() {
        return Err("Shadowsocks requires a cipher.");
    }
    if p.kind == "hysteria" {
        if text("auth-str").is_empty() && text("auth").is_empty() {
            return Err("Hysteria requires authentication.");
        }
        if !p.options.contains_key("up") || !p.options.contains_key("down") {
            return Err("Hysteria requires upload and download bandwidth values.");
        }
        if !["", "udp"].contains(&text("protocol")) {
            return Err(
                "Hysteria currently supports UDP transport only; faketcp needs privileges.",
            );
        }
    }
    if p.kind == "hysteria2"
        && (!text("obfs").is_empty() && !["salamander", "gecko"].contains(&text("obfs"))
            || !text("obfs").is_empty() && text("obfs-password").is_empty())
    {
        return Err("Hysteria 2 obfuscation requires salamander or gecko and a password.");
    }
    if let Some(ports) = p.options.get("ports") {
        validate_ports(ports.as_str().ok_or("Invalid port list.")?)?;
    }
    if !text("fingerprint").is_empty() {
        let compact = text("fingerprint").replace(':', "");
        if compact.len() != 64 || !compact.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err("Certificate fingerprint must contain 32 SHA-256 bytes.");
        }
    }
    Ok(())
}
fn validate_object(value: &Value, allowed: &[&str]) -> Result<(), &'static str> {
    let object = value
        .as_object()
        .ok_or("Transport options must be an object.")?;
    for (key, value) in object {
        if !allowed.contains(&key.as_str()) {
            return Err("Unsupported nested transport option.");
        }
        match key.as_str() {
            "headers" => {
                let headers = value.as_object().ok_or("Headers must be an object.")?;
                for (name, values) in headers {
                    if name.is_empty()
                        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        return Err("Invalid HTTP header name.");
                    }
                    if let Some(s) = values.as_str() {
                        if s.contains(['\r', '\n']) {
                            return Err("Invalid HTTP header value.");
                        }
                    } else if !values.as_array().is_some_and(|a| {
                        a.iter()
                            .all(|v| v.as_str().is_some_and(|s| !s.contains(['\r', '\n'])))
                    }) {
                        return Err("Invalid HTTP header values.");
                    }
                }
            }
            "no-grpc-header" => {
                if !value.is_boolean() {
                    return Err("Invalid transport flag.");
                }
            }
            "host" | "path" => {
                if !value.is_string()
                    && !value
                        .as_array()
                        .is_some_and(|a| a.iter().all(Value::is_string))
                {
                    return Err("Invalid transport host/path.");
                }
            }
            _ => {
                if !value.is_string() {
                    return Err("Invalid transport parameter.");
                }
            }
        }
    }
    Ok(())
}
pub fn validate_ports(input: &str) -> Result<(), &'static str> {
    if input.is_empty() {
        return Err("Port list cannot be empty.");
    }
    for segment in input.split(',') {
        let ports: Vec<_> = segment.split('-').collect();
        if ports.is_empty() || ports.len() > 2 {
            return Err("Invalid port range.");
        }
        let start = ports[0]
            .parse::<u16>()
            .ok()
            .filter(|p| *p > 0)
            .ok_or("Invalid port.")?;
        if ports.len() == 2 && ports[1].parse::<u16>().ok().is_none_or(|end| end < start) {
            return Err("Invalid port range.");
        }
    }
    Ok(())
}
