# Validation record

Executed on 2026-11-15 on an Apple Silicon development Mac (Darwin 27.2),
with Rust 1.99.1 and pinned official Mihomo v1.19.34.

## Automated checks

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --locked --all-targets -- -D warnings`: passed.
- `cargo test --locked`: 13 passed; the 2 real-core tests are intentionally ignored
  in the ordinary run and were executed separately.
- `cargo test --locked --test real_core -- --ignored`: 2 passed.
- Python scripts compiled, core installer and notice collector executed, source
  companion generated, .app built, and ad-hoc signature verification passed.
- Bundle metadata, pinned executable hash, .icns, and 236 active Rust dependency
  notice entries checked. Mihomo corresponding source includes vendored modules.

The regular suite covers VLESS transports and invalid inputs, YAML and Base64,
rejection of routing overrides, duplicate names and stable identities, selection
and settings persistence, authenticated encryption and corruption detection,
failed subscription updates, removal of a selected subscription, one-server-only
routing, stopped-proxy test isolation, invalid core configuration, missing or
modified executables, startup failures, port conflicts and proxy timeouts.

## Acceptance scenarios

| Scenario | Executed evidence | Limits |
| --- | --- | --- |
| A: normal proxy operation | Local VLESS fixtures over TCP, WebSocket and gRPC; HTTPS 204 through HTTP and SOCKS5 | Fixture VLESS transports are unencrypted loopback; HTTPS destination certificate is verified |
| B: fixed selection | Encrypted vault reload and stable identity tests; GUI quit and reopen restored the same selection | No system login / launch-agent test |
| C: selected server failure | Killed selected VLESS fixture with a live backup and a directly reachable HTTPS destination; new proxied requests failed | Not an internet outage or packet-loss benchmark |
| D: subscription update | Stable identity across rename/update; failed actual fetch preserves old catalog and update time; removed-selection worker test persists unavailable state | No production 3x-ui subscription URL was supplied |
| E: core failure | Killed owned core; integration test detects exit and requests fail; GUI displayed the unexpected-exit message and Start control | Forced application death itself is not supervised |

The real-core suite also checks controller authorization, selector membership,
core restart retaining selection, and real Mihomo rejection of invalid config.
It validates generated TLS, Vision, WebSocket TLS, gRPC TLS and REALITY configs
with Mihomo's configuration checker. It does **not** prove an end-to-end TLS or
REALITY handshake with a production VLESS provider. Do not describe that as tested.

## Desktop checks

The final real GUI was launched from a copy of its macOS application bundle
in `/tmp/nokiri-standalone-check`, outside the repository. Its core executed
from that copy's `Contents/Resources` directory. The standalone bundle signature
and decoded .icns were verified. Both themes were saved and photographed.
The real GUI was launched from its macOS application bundle. It imported a local
VLESS test fixture, stored its encrypted catalog using Keychain, selected it,
started the bundled core, performed an HTTPS request to Google's 204 endpoint,
measured server latency, and obtained a public IP through the local proxy from
ipify. The fixture itself forwards to the internet; it is not a production VPN
or a claim of a different geographic exit location. No exit IP is saved in the
repository screenshots.

Keychain requested authorization after ad-hoc rebuilding. Access was restored by
the user, not by disabling Keychain checks. Code-signing updates can require
another authorization. Screenshots are captures of the actual implemented UI.

## Not verified in this environment

Linux GUI / packaging, remote GitHub Actions execution, older macOS versions,
production provider subscriptions, real external TLS/REALITY sessions, notarized
distribution, hardware resource benchmarks, and Layered Icon Composer assets.
No public release was published. UI accessibility support is enabled through
AccessKit, but a full VoiceOver audit was not performed.

The temporary GUI fixture catalog was removed after verifying that it contained
only the test server and no subscriptions. Nokiri and its test upstream were
stopped cleanly; the application is ready for the user's own imports.

## Desktop design follow-up

The subsequent design review added six UI regression tests. The current ordinary
suite passes 19 tests; the two real-core tests remain separately runnable and
were not rerun for this UI-only change. Navigation clipping and icon-click tests
failed before the fix and pass afterward. All pages were rendered at 720x560 and
980x780 in both themes, including long names. See [design review](design-review.md)
for findings and direct desktop checks. Current screenshots show the corrected
first-launch UI with an empty catalog. The macOS app was rebuilt and launched.

## Multiple-protocol follow-up (2026-11-16)

The ordinary suite now passes 25 tests. Formatting, all-target Clippy with
warnings denied, and diff whitespace checks pass. The separate protocol test
passes all six checks, including official Mihomo configuration validation for
Hysteria 1/2, VMess, Trojan, Shadowsocks, HTTPS, SOCKS5, and VLESS HTTPUpgrade,
XHTTP, HTTP camouflage, H2 and WebSocket early data.

The expanded real-core suite uses loopback fixtures, ephemeral credentials and a
verified HTTPS destination. Hysteria 2 additionally uses Salamander and a pinned
server certificate, rejects a wrong pin/password, and retains selection across
restart. The same HTTP/SOCKS5, restart, dead-server/no-fallback and controller
checks cover VLESS TCP/WS/gRPC, VMess, Trojan TLS and Shadowsocks.

The macOS bundle was rebuilt and signature-verified. Native checks confirmed
both saved profiles and the existing selection, the protocol import help,
TLS consent disabled by default, and successful Russian profile testing. The
German profile remains unavailable. See [protocol audit](protocol-audit.md)
for the diagnosis, dependency findings and precise validation limits.
