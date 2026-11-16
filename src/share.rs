use crate::{
    model::Proxy,
    subscription::{ImportError, parse_vless, validate},
};
use base64::{
    Engine,
    engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD},
};
use percent_encoding::percent_decode_str;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use url::Url;
fn bad(message: &str) -> ImportError {
    ImportError(message.into())
}
pub fn decode(input: &str) -> Result<String, ImportError> {
    for encoding in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(data) = encoding.decode(input)
            && let Ok(text) = String::from_utf8(data)
        {
            return Ok(text);
        }
    }
    Err(bad("Invalid Base64 proxy data."))
}
pub fn unescape(input: &str) -> Result<String, ImportError> {
    percent_decode_str(input)
        .decode_utf8()
        .map(String::from)
        .map_err(|_| bad("Invalid UTF-8 URL component."))
}
pub fn parse_url(input: &str) -> Result<Proxy, ImportError> {
    let scheme = input.split("://").next().unwrap_or("").to_ascii_lowercase();
    if scheme == "vless" {
        return parse_vless(input);
    }
    if scheme == "vmess" {
        return parse_vmess(input);
    }
    let mut normalized = input.to_string();
    if scheme == "ss" {
        let body = input
            .trim_start_matches("ss://")
            .split('#')
            .next()
            .unwrap_or("");
        if !body.contains('@') {
            let fragment = input.split_once('#').map_or("", |(_, s)| s);
            normalized = format!("ss://{}#{fragment}", decode(body)?);
        }
    }
    let (normalized, ports) = normalize_ports(&normalized)?;
    let u = Url::parse(&normalized).map_err(|_| bad("Invalid proxy URL or port."))?;
    let kind = match scheme.as_str() {
        "hy2" | "hysteria2" => "hysteria2",
        "hysteria" => "hysteria",
        "trojan" => "trojan",
        "ss" => "ss",
        "http" | "https" => "http",
        "socks" | "socks5" | "socks5h" => "socks5",
        _ => return Err(bad("Unsupported proxy URL scheme.")),
    };
    let host = u
        .host_str()
        .ok_or_else(|| bad("Missing server host."))?
        .trim_matches(['[', ']'])
        .to_string();
    let port = u
        .port_or_known_default()
        .unwrap_or(if kind == "socks5" { 1080 } else { 443 });
    let name = unescape(u.fragment().unwrap_or("Imported server"))?;
    let mut p = Proxy::new(kind, host, port, name);
    let q = query(&u)?;
    let get = |k: &str| q.get(k).map(String::as_str).unwrap_or("");
    let allowed: &[&str] = match kind {
        "hysteria2" => &[
            "sni",
            "insecure",
            "allowInsecure",
            "obfs",
            "obfs-password",
            "pinSHA256",
            "mport",
            "up",
            "down",
            "alpn",
        ],
        "hysteria" => &[
            "auth",
            "peer",
            "sni",
            "insecure",
            "allowInsecure",
            "obfs",
            "upmbps",
            "downmbps",
            "up",
            "down",
            "protocol",
            "alpn",
            "mport",
            "pinSHA256",
        ],
        "trojan" => &[
            "sni",
            "peer",
            "security",
            "type",
            "host",
            "path",
            "serviceName",
            "fp",
            "alpn",
            "allowInsecure",
            "insecure",
        ],
        "ss" => &[],
        _ => &["sni", "insecure", "allowInsecure"],
    };
    if q.keys().any(|k| !allowed.contains(&k.as_str())) {
        return Err(bad(
            "Unsupported URL parameter or plugin. Import supported inline YAML settings instead.",
        ));
    }
    let username = unescape(u.username())?;
    let password = unescape(u.password().unwrap_or(""))?;
    match kind {
        "hysteria2" => {
            let auth = if u.password().is_some() {
                format!("{username}:{password}")
            } else {
                username
            };
            p.options.insert("password".into(), auth.into());
            for key in ["obfs", "obfs-password", "up", "down"] {
                if !get(key).is_empty() {
                    p.options.insert(key.into(), get(key).into());
                }
            }
            p.tls = true;
        }
        "hysteria" => {
            p.options.insert(
                "auth-str".into(),
                if get("auth").is_empty() {
                    username
                } else {
                    get("auth").into()
                }
                .into(),
            );
            for (target, a, b) in [("up", "upmbps", "up"), ("down", "downmbps", "down")] {
                let value = if !get(a).is_empty() { get(a) } else { get(b) };
                p.options.insert(
                    target.into(),
                    if value.is_empty() { "100" } else { value }.into(),
                );
            }
            for key in ["obfs", "protocol"] {
                if !get(key).is_empty() {
                    p.options.insert(key.into(), get(key).into());
                }
            }
            p.tls = true;
        }
        "trojan" => {
            if u.password().is_some() {
                return Err(bad("Percent-encode colons inside a Trojan password."));
            }
            p.options.insert("password".into(), username.into());
            p.tls = true;
            if !["", "tls"].contains(&get("security")) {
                return Err(bad("Trojan currently requires TLS security."));
            }
            let mut url =
                Url::parse("vless://00000000-0000-4000-8000-000000000001@127.0.0.1:443").unwrap();
            {
                let mut pairs = url.query_pairs_mut();
                for (key, value) in &q {
                    if key != "peer" {
                        pairs.append_pair(key, value);
                    }
                }
            }
            let transport = parse_vless(url.as_str())?;
            p.network = transport.network;
            p.ws_opts = transport.ws_opts;
            p.grpc_opts = transport.grpc_opts;
            p.options.extend(
                transport
                    .options
                    .into_iter()
                    .filter(|(k, _)| k != "encryption"),
            );
        }
        "ss" => {
            let credentials = if u.password().is_some() {
                format!("{username}:{password}")
            } else {
                decode(&username)?
            };
            let (cipher, password) = credentials
                .split_once(':')
                .ok_or_else(|| bad("Shadowsocks requires cipher:password credentials."))?;
            p.options.insert("cipher".into(), cipher.into());
            p.options.insert("password".into(), password.into());
        }
        _ => {
            if !username.is_empty() {
                p.options.insert("username".into(), username.into());
            }
            if u.password().is_some() {
                p.options.insert("password".into(), password.into());
            }
            p.tls = scheme == "https";
        }
    }
    p.servername = if !get("sni").is_empty() {
        get("sni")
    } else {
        get("peer")
    }
    .into();
    p.client_fingerprint = get("fp").into();
    p.alpn = get("alpn")
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    if flag(get("insecure"))? || flag(get("allowInsecure"))? {
        p.options.insert("skip-cert-verify".into(), true.into());
    }
    if !get("pinSHA256").is_empty() {
        p.options
            .insert("fingerprint".into(), get("pinSHA256").into());
    }
    if let Some(ports) =
        ports.or_else(|| (!get("mport").is_empty()).then(|| get("mport").replace(':', "-")))
    {
        if !["hysteria", "hysteria2"].contains(&kind) {
            return Err(bad("Port hopping is only supported for Hysteria profiles."));
        }
        p.options.insert("ports".into(), ports.into());
    }
    validate(&p)?;
    Ok(p)
}
pub fn query(u: &Url) -> Result<BTreeMap<String, String>, ImportError> {
    let mut result = BTreeMap::new();
    for (k, v) in u.query_pairs() {
        if result.insert(k.into_owned(), v.into_owned()).is_some() {
            return Err(bad("Duplicate URL parameters are not supported."));
        }
    }
    Ok(result)
}
pub fn flag(input: &str) -> Result<bool, ImportError> {
    match input {
        "" | "0" | "false" => Ok(false),
        "1" | "true" => Ok(true),
        _ => Err(bad("Expected a boolean URL flag (0/1 or false/true).")),
    }
}
fn normalize_ports(input: &str) -> Result<(String, Option<String>), ImportError> {
    let Some((scheme, body)) = input.split_once("://") else {
        return Err(bad("Expected a proxy URL."));
    };
    let end = body.find(['/', '?', '#']).unwrap_or(body.len());
    let authority = &body[..end];
    let address = authority.rsplit('@').next().unwrap_or(authority);
    let Some((_, port)) = address.rsplit_once(':') else {
        return Ok((input.into(), None));
    };
    if !port.contains([',', '-']) {
        return Ok((input.into(), None));
    }
    crate::protocol::validate_ports(port).map_err(bad)?;
    let first = port.split([',', '-']).next().unwrap();
    Ok((
        format!(
            "{scheme}://{}{}{}",
            &authority[..authority.len() - port.len()],
            first,
            &body[end..]
        ),
        Some(port.into()),
    ))
}
fn parse_vmess(input: &str) -> Result<Proxy, ImportError> {
    let raw = input
        .split_once("://")
        .map(|(_, s)| s)
        .ok_or_else(|| bad("Invalid VMess URL."))?;
    let value: Value = serde_json::from_str(&decode(raw)?)
        .map_err(|_| bad("Expected a Base64 VMess JSON profile."))?;
    let object = value
        .as_object()
        .ok_or_else(|| bad("VMess data must be an object."))?;
    let text = |k: &str| {
        object
            .get(k)
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_default()
    };
    if object.keys().any(|k| {
        ![
            "v",
            "ps",
            "add",
            "port",
            "id",
            "aid",
            "scy",
            "net",
            "type",
            "host",
            "path",
            "tls",
            "sni",
            "alpn",
            "fp",
            "allowInsecure",
        ]
        .contains(&k.as_str())
    }) {
        return Err(bad("Unsupported VMess JSON field."));
    }
    let mut u =
        Url::parse("vless://00000000-0000-4000-8000-000000000001@placeholder.invalid:443").unwrap();
    u.set_host(Some(&text("add")))
        .map_err(|_| bad("Invalid VMess host."))?;
    u.set_port(Some(
        text("port")
            .parse::<u16>()
            .map_err(|_| bad("Invalid VMess port."))?,
    ))
    .map_err(|_| bad("Invalid VMess port."))?;
    u.set_username(&text("id"))
        .map_err(|_| bad("Invalid VMess UUID."))?;
    let name = text("ps");
    u.set_fragment(Some(if name.is_empty() {
        "Imported server"
    } else {
        &name
    }));
    {
        let mut q = u.query_pairs_mut();
        for (target, source) in [
            ("type", "net"),
            ("host", "host"),
            ("path", "path"),
            ("sni", "sni"),
            ("alpn", "alpn"),
            ("fp", "fp"),
            ("allowInsecure", "allowInsecure"),
        ] {
            let v = text(source);
            if !v.is_empty() {
                q.append_pair(target, &v);
            }
        }
        if text("tls") == "tls" {
            q.append_pair("security", "tls");
        } else if !text("tls").is_empty() && text("tls") != "none" {
            return Err(bad("Unsupported VMess TLS mode."));
        }
        if text("type") == "http" {
            q.append_pair("headerType", "http");
        } else if !["", "none"].contains(&text("type").as_str()) {
            return Err(bad("Unsupported VMess header type."));
        }
    }
    let mut p = parse_vless(u.as_str())?;
    p.kind = "vmess".into();
    p.options.insert(
        "alterId".into(),
        json!(if text("aid").is_empty() {
            0
        } else {
            text("aid")
                .parse::<u32>()
                .map_err(|_| bad("Invalid VMess alterId."))?
        }),
    );
    p.options.insert(
        "cipher".into(),
        if text("scy").is_empty() {
            "auto".into()
        } else {
            text("scy")
        }
        .into(),
    );
    validate(&p)?;
    Ok(p)
}
