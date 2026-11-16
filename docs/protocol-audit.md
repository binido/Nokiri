# Protocol and runtime audit

Reviewed on 2026-11-16 on macOS ARM64 with official pinned Mihomo v1.19.34.

## Changes

- Added Hysteria 1/2, VMess, Trojan, Shadowsocks, HTTP and SOCKS5 upstream imports.
  URL lists, Base64 subscriptions and inline Clash YAML share the same validation.
- Extended VLESS to HTTPUpgrade, XHTTP, H2, HTTP camouflage and WebSocket early
  data. Unsupported extensions still fail explicitly. No external plugin or
  subscription routing settings enter the generated core configuration.
- Protocol fields are allowlisted and typed. Passwords and nested transport
  credentials are included in diagnostic redaction. Existing VLESS serialized
  fields and IDs remain compatible, and legacy subscriptions default to verified
  TLS. No user catalog reset or credential migration is needed.
- Disabled TLS verification requires explicit import consent unless a SHA-256
  certificate pin is configured. Consent persists per subscription; affected
  profiles have a visible warning. Certificate verification is the default.
- Fixed duplicate probe startup stopping an already running core. A real-core
  regression test failed before the guard and passes after it.

## Provider investigation

Both saved endpoints accepted a TCP connection. The Russian WebSocket/TLS profile
passed the production HTTPS availability check; the rebuilt GUI measured 651 ms.
The German TCP/REALITY/Vision profile failed. Isolated pinned Mihomo probes
reported `REALITY Authentication: false`; edge, chrome and chrome120 fingerprints
did not resolve it. An independent official Xray v26.11.9 probe with the same
parameters also failed HTTPS. The clock differed from an external HTTP Date by
one second. Tests did not select another server, enable DIRECT fallback, or
weaken certificate checks.

This does not establish which provider-side parameter is wrong. A current
provider export or a confirmed working client on this Mac is needed to compare
SNI, public key, short ID and remote REALITY configuration. The German connection
is unresolved; adding protocol support does not repair external authentication.
No endpoint address, UUID, subscription URL or credential is included here.

## Validation evidence

The ordinary suite passed 25 tests; the separate official configuration check
passed, and both real-core integration tests passed in 71.54 seconds. Actual
HTTPS over both local proxy types passed for VLESS TCP/WS/gRPC, Hysteria 2 with
Salamander, VMess TCP, Trojan TLS and Shadowsocks AES-128-GCM. Hysteria 2 rejected
both an incorrect certificate pin and an incorrect password. All fixtures checked
restart, fixed selection, authenticated controller and failure without fallback.
Formatting, all-target Clippy with warnings denied and bundle signature checks
passed. HTTPS/SOCKS5 upstreams were configuration-tested only.

## Security and remaining scope

`cargo audit` v0.23.0 found zero known vulnerabilities in the locked dependency
set. It reported maintenance warnings for ttf-parser, unsafe-libyaml, zbus_names,
zvariant and zvariant_derive (including duplicate transitive versions). These
remain tracked here; no dependency replacement was mixed into protocol support.
The Linux-only Secret Service dependency chain has not been runtime-tested.

The audit covered imports, generated routing, encrypted storage compatibility,
core lifecycle, diagnostics, TLS policy, dependencies and native import UI.
It is not a penetration test or an exhaustive protocol compatibility claim.
Legacy Hysteria 1 has parser and official configuration validation, without a
live legacy server fixture. XHTTP, HTTPUpgrade and H2 have official configuration
validation, without a live external provider handshake. Linux packaging and a
full VoiceOver audit remain unverified. macOS builds remain ad-hoc signed.

Protocol schemas were checked against the pinned official source and
[MetaCubeX documentation](https://wiki.metacubex.one/en/config/proxies/), including
[Hysteria 2 URI rules](https://v2.hysteria.network/docs/developers/URI-Scheme/).
