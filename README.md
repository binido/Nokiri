<div align="center">
  <img src="assets/icons/nokiri.png" width="112" alt="Nokiri app icon" />
  <h1>Nokiri</h1>
  <p><b>Your server. Your choice.</b></p>
  <p>A small native proxy manager for people who already have a server.<br />Built with Rust and egui. macOS first, Linux in view.</p>
  <p>
    <a href="https://github.com/binido/Nokiri/actions/workflows/ci.yml"><img src="https://img.shields.io/badge/CI-configured-orange" alt="CI configured" /></a>
    <img src="https://img.shields.io/badge/macOS-Apple_Silicon-black" alt="macOS Apple Silicon" />
    <a href="LICENSE"><img src="https://img.shields.io/badge/Nokiri-MIT-blue" alt="Nokiri source MIT license" /></a>
    <a href="assets/MIHOMO-LICENSE"><img src="https://img.shields.io/badge/Mihomo-GPL--3.0-blue" alt="Mihomo core GPL-3.0 license" /></a>
  </p>
  <p><a href="#installation">Install</a> · <a href="#usage">Use Nokiri</a> · <a href="#build-from-source">Build</a> · <a href="#security-and-privacy">Security</a></p>
  <img src="docs/screenshots/dashboard-dark.png" width="860" alt="Nokiri's actual macOS dashboard before importing a server" />
</div>

Nokiri exposes one local HTTP/SOCKS5 endpoint and keeps your chosen server fixed.
It is designed for terminal tools, development clients and applications that
already support proxies. It does not intercept the rest of your Mac's traffic.

## Features

- **Deliberate routing.** Explicit selection, stable server IDs and selection
  persistence across application and core restarts. Duplicate names are safe.
- **Subscriptions without surprises.** Multiple HTTPS subscriptions, manual
  updates, renaming and removal. A failed update preserves the previous catalog.
- **Actual connection checks.** Per-server HTTPS availability and latency,
  selected-proxy HTTPS checks, and a public exit IP obtained through the proxy.
- **Native and quiet.** Rust / egui, bundled Roboto, orange Material 3-inspired
  surfaces, light and dark themes, event-driven repainting and a managed core.
- **Private by default.** Encrypted catalog, macOS Keychain key, localhost-only
  listeners and an authenticated controller. No account or telemetry.

If your selected server fails, proxied connections fail. Nokiri does not choose a
backup or switch to DIRECT. If a subscription removes that server, the proxy
stops and asks you to select a valid server explicitly.

## Installation

Download the Apple Silicon DMG from [GitHub Releases](https://github.com/binido/Nokiri/releases).
Open it and drag Nokiri into Applications. The first release is built on macOS 26
for ARM64; there is no Intel or Linux application download. You can also build
`dist/Nokiri.app` from source using the instructions below.

The initial build is ad-hoc signed, **not Developer ID signed or notarized**.
Gatekeeper may block a downloaded copy. If your Mac refuses to open it, inspect
the source and distribution notes before deciding whether to allow this build. See
[distribution notes](docs/distribution.md) for signing, Keychain access after
rebuilding, and the corresponding-source companion needed when sharing the core.

## Usage

1. Open **Subscriptions** and add a name and an HTTPS subscription URL. For a
   single server, choose **Import proxy / YAML** and paste its URL or YAML.
2. Open **Servers**, select a server explicitly, and optionally run **Test**.
3. On **Dashboard**, click **Start proxy**. The default endpoint is
   `http://127.0.0.1:7890`; the same port accepts SOCKS5.
4. Point your client at that endpoint. Use **Test HTTPS** or **Check exit IP**
   to verify the selected proxy. **Stop proxy** closes the local listener.

For one terminal command:

```sh
HTTPS_PROXY=http://127.0.0.1:7890 HTTP_PROXY=http://127.0.0.1:7890 curl https://example.com
```

Or explicitly use SOCKS5 with remote destination-name resolution:

```sh
curl --proxy socks5h://127.0.0.1:7890 https://example.com
```

Nokiri does not change macOS network settings. Your client's own bypass settings,
such as `NO_PROXY`, remain its responsibility. Settings allow another local port,
a theme, and starting the saved selection when Nokiri opens.

### Background operation on macOS

Closing the window keeps Nokiri and its proxy running in the menu bar and removes
the Dock icon. The menu shows connection state and any error, and provides
**Open Nokiri**, **Connect / Disconnect**, and **Quit Nokiri**. Opening the window
restores its Dock icon. **Quit Nokiri** and **Cmd+Q** stop the proxy and exit.
Connection errors stay in the menu bar without opening the window or reconnecting
automatically.

In Settings, **Launch Nokiri at macOS login** independently enables a hidden launch
at your next login. The checkbox takes effect immediately; the existing
**Start the selected proxy when Nokiri opens** setting controls whether that
launch also connects. Both are off by default. Install Nokiri.app in Applications
before enabling launch at login, and enable it again if you move the app.
macOS may require allowing Nokiri in Login Items / background activity settings.

Launch registration is a per-user file at
`~/Library/LaunchAgents/io.github.binido.nokiri.plist`; turning the checkbox off
removes it. It contains only the application path and `--background`, with no
server credentials. Nokiri runs after user login, without administrator access or
a separate system service. Removing the app alone does not remove this file;
disable launch at login before uninstalling.

## Supported imports

| Input | Scope |
| --- | --- |
| `vless://` | TCP, WebSocket, HTTPUpgrade, gRPC, HTTP, H2, XHTTP; TLS, REALITY and TCP Vision |
| `hysteria2://`, `hy2://` | Password authentication, SNI, Salamander/Gecko, port hopping, certificate pin |
| `hysteria://` | Legacy Hysteria over UDP, auth, bandwidth, obfuscation and port hopping |
| `vmess://` | Common Base64 JSON profiles, TCP, WebSocket, gRPC, HTTP and H2 |
| `trojan://` | TLS over TCP, WebSocket or gRPC |
| `ss://` | Shadowsocks SIP002 and legacy Base64, without executable plugins |
| `http://`, `https://`, `socks5://` | HTTP CONNECT / SOCKS5 upstreams with optional authentication |
| Base64 subscription | Mixed supported URLs, standard / URL-safe, padded or unpadded |
| Clash / Mihomo YAML | An inline `proxies` list with allowlisted protocol fields |

VLESS supports SNI, client fingerprint, ALPN, WebSocket early data, gRPC service
name, REALITY public key / short ID, and core-supported VLESS encryption strings.
XHTTP accepts path, host, mode and a limited set of inline options; external
`downloadSettings`, multiplexing and gRPC multi mode remain unsupported. Unknown
fields fail with an explanation instead of being silently dropped.

Certificate verification stays enabled by default. A profile requesting
`insecure=1`, `allowInsecure=1` or `skip-cert-verify: true` requires explicit
consent in the import dialog unless a valid certificate SHA-256 pin is present.
Unverified profiles remain visibly marked. Subscription updates retain the
source's import consent. Legacy Hysteria URLs without bandwidth values default
to 100 Mbps upload/download; YAML must specify them.

Imports are all-or-nothing. Subscription-provided rules, groups, providers,
scripts and listener settings are never activated. Provider-only YAML without
inline proxies and external executable plugins are unsupported.

HTTPS subscriptions have a 20-second timeout and 4 MiB / 5000-server limits.
Redirects are refused to avoid disclosing subscription credentials. Availability
tests are manual, do not change selection, and are not background health checks.
See [validation](docs/validation.md) for executed tests and remaining coverage.

## Build from source

Requirements: stable Rust, Python 3, Apple's command-line tools, and macOS on
Apple Silicon. The bundle declares macOS 12 minimum; validation was on the
current development Mac, not every older macOS version.

```sh
git clone https://github.com/binido/Nokiri.git
cd Nokiri
python3 scripts/fetch_core.py
cargo run --locked
```

Build the standalone app:

```sh
python3 scripts/package_macos.py
open dist/Nokiri.app
```

The installer downloads only the pinned official Mihomo release, verifies both
archive and executable SHA-256, and does not execute an unverified binary. The
application verifies the executable again before each activation. For quality
checks and local real-core tests, see [CONTRIBUTING.md](CONTRIBUTING.md).

## Architecture

```text
egui views  ->  command worker  ->  owned catalog + encrypted persistence
                         |
                         +-> validated YAML -> managed Mihomo subprocess
                         +-> authenticated localhost controller
                         +-> explicit HTTP-proxied HTTPS checks
```

Mihomo handles the proxy protocols. Nokiri imports a limited data model and
constructs its own configuration: one selector containing exactly the chosen
server, one `MATCH,NOKIRI` rule, no routing providers and no fallback. Additional
nodes are present only so the controller can test them; they cannot be selected
through the routing group. Stopped-proxy availability tests use a temporary core
with no proxy listener, no selector, and `MATCH,REJECT`.

The engine lifecycle contract is deliberately small. Platform integration is
separate from models, parsing and GUI logic. No sing-box backend is implemented.

## Security and privacy

- The catalog, including URLs, UUIDs, endpoints and settings, is encrypted using
  ChaCha20-Poly1305. The random encryption key is held in macOS Keychain, through
  a `SecretStore` interface. There is no plaintext storage fallback.
- Data lives in the platform application-data directory returned by `directories`,
  outside the repository. On macOS this is `~/Library/Application Support/io.github.binido.Nokiri`.
  The catalog is written atomically with private file permissions.
- Mihomo receives a short-lived private configuration file inside a mode-0700
  temporary directory. It contains the credentials the core must use and is
  removed on normal shutdown. Known secrets are redacted from the in-memory
  diagnostics buffer; no engine log file is written.
- Mixed HTTP/SOCKS5 and controller bind to `127.0.0.1`. The controller uses a
  random per-launch bearer secret. Other local applications can use the mixed
  proxy; the OS user and trusted local software remain the trust boundary.
- Import and update requests go directly to the HTTPS subscription provider.
  Manual connectivity checks contact Google; exit-IP checks contact ipify through
  the selected proxy. No checks run automatically in the background.
- Destination traffic entering the mixed listener has no direct route. OS DNS
  resolution of the upstream hostname and subscription fetching are outside that
  traffic boundary. Nokiri is not a firewall or kill switch for other programs.

A forced application kill or power loss can leave a runtime file and orphaned
core. Nokiri never kills a process it did not retain as its own child. On a
conflict it reports the occupied port rather than terminating another process.
Back up the encrypted catalog **and its credential-store key** together; one
without the other cannot restore your subscriptions.

## Linux roadmap

The models, parser, strict config, controller and egui interface are shared.
Linux builds select Secret Service and XDG data directories. Linux CI is
configured; Linux GUI operation and packaging have not been executed here.

Before an Arch Linux release: pin and verify an official Linux x86_64 core,
exercise a running Secret Service, test X11/Wayland, add desktop/SVG icon assets
and native packaging, then consider an optional user systemd unit. No Linux
core installer, AppImage, Arch package or systemd service is shipped yet.

## License and credits

Nokiri's original application source and routing mark use the [MIT license](LICENSE).
The separately executed, unmodified Mihomo binary uses [GPL-3.0](assets/MIHOMO-LICENSE).
A redistributed app bundle includes both licenses and dependency notices; the
MIT license does not replace the core's source-distribution obligations.
See [third-party notices](THIRD_PARTY_NOTICES.md) and [distribution](docs/distribution.md).

Built with [egui](https://github.com/emilk/egui), [Tokio](https://tokio.rs),
[reqwest](https://github.com/seanmonstar/reqwest), and
[Mihomo](https://github.com/MetaCubeX/mihomo). Typography is Google's
[Roboto](https://github.com/google/fonts/tree/main/ofl/roboto) (OFL-1.1);
UI icons are [Material Symbols](https://github.com/google/material-design-icons)
(Apache-2.0). The repository presentation was inspired by
[Purge](https://github.com/jithin-sabu/purge-app), with original text and assets.

Screenshots show the implemented application with an empty catalog. Functional
proxy checks used local fixtures as documented in the validation record. No
private endpoints or credentials are included. Light mode: [view screenshot](docs/screenshots/dashboard-light.png).

The desktop layout and interaction follow-up is recorded in the [design review](docs/design-review.md).
