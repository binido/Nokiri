# macOS distribution

## Local bundle

On an Apple Silicon Mac, with stable Rust, Python 3, and Apple's command-line tools:

```sh
python3 scripts/package_macos.py
open dist/Nokiri.app
```

The script verifies the official pinned gzip artifact, the decompressed core, and
the app's ad-hoc signature. Runtime core verification uses a digest compiled into
Nokiri; modifying the bundle's core.json cannot authorize another executable.
The script packages all notices for the current target's locked Rust graph,
including license text from exact upstream source commits when crates omit it.
It requires network access on its first run. The application itself does not
download or update an executable at runtime.

Nokiri is **ad-hoc signed only**, with no Developer ID or notarization. Finder
launch works locally. A downloaded copy can be blocked by Gatekeeper. Distribution
to other people still requires a maintainer's signing and notarization workflow;
Nokiri does not modify Gatekeeper settings. Keychain can request authorization
again after an ad-hoc rebuild changes the application identity.

## Corresponding Mihomo source

With Go installed, prepare the source companion before sharing the bundle:

```sh
python3 scripts/prepare_core_source.py
```

This verifies the exact pinned upstream archive and produces
`dist/Mihomo-v1.19.34-source.tar.gz`, including the original build recipes,
license, go.mod / go.sum, and dependencies vendored by Go. Distribute this
companion next to the app wherever you distribute its bundled core. Retain
upstream notices. The official binary is supplied unmodified; reproducible
bit-for-bit rebuilding is not asserted.

For a new core version, inspect its official release, pin both compressed and
executable SHA-256 digests plus the corresponding source archive digest in
`assets/core.json` and `assets/core-source.json`, run all real-core tests, and regenerate the source companion.
Never derive runtime trust from an editable bundle manifest.

## Release checklist

- Run the quality checks and real-core integration tests in CONTRIBUTING.md.
- Generate the local .app and source companion; inspect included license notices.
- Test the app from a folder outside the source checkout.
- For public distribution, sign both executables and bundle with a Developer ID,
  notarize and staple using the maintainer's own Apple account.
- Offer the exact corresponding source companion alongside the binary download.

CI builds and checks the app on macOS and checks shared Rust code on Linux. It
publishes no release and uploads no private catalogs or credentials.
