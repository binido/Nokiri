# Contributing to Nokiri

Keep Nokiri focused on explicit local proxying. Do not add TUN, automatic system
proxy changes, routing databases, automatic server selection or direct fallback.
Use English for code, comments, docs and UI. Use Conventional Commits and a topic
branch. Never commit real subscription URLs, credentials or private endpoints.

## Development

Install stable Rust and Apple's command-line tools on an Apple Silicon Mac.
Python 3 is required by the packaging scripts. Go is needed only to prepare the
Mihomo corresponding source companion for redistribution.

```sh
python3 scripts/fetch_core.py
cargo run --locked
```

Development and packaged builds use the same system credential-store key and
application data location. Ad-hoc rebuilds can cause macOS to ask for Keychain
access again. Do not delete a user's encrypted catalog to resolve access errors.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --test real_core --test protocols -- --ignored
python3 scripts/package_macos.py
```

Ordinary tests are offline and never read the system Keychain. Real-core tests
require the verified ARM64 core, OpenSSL and Python 3. They generate ephemeral
certificates and local protocol/HTTPS fixtures. They do not require a paid proxy or
user credentials. Tests exercise public behavior rather than copying the parser.

## Architecture boundaries

- `model` and `subscription`: owned proxy data, stable identities, validated imports.
- `config` and `engine`: strict configuration, process ownership, controller, tests.
- `platform`: credential-store interface, authenticated encryption, paths and lock.
- `worker`: asynchronous command processing and transactional updates.
- `ui`: egui views, Material-inspired visual tokens and user actions.

Add an engine by implementing the small `ProxyEngine` lifecycle contract and
isolating its config/controller adapter; do not make UI or subscriptions import
an engine's unrestricted routing model. A future sing-box adapter is not implemented.

## Reporting issues

Describe the action, expected behavior, OS, Nokiri and core versions. Include
redacted diagnostics only. Never paste an entire subscription or generated runtime
configuration into an issue. See docs/validation.md for the tested acceptance scope.
