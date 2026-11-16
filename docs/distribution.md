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
as a conventional macOS download requires a maintainer's signing and notarization
workflow. The initial v0.1.0 release is an explicitly ad-hoc signed early build;
it does not offer that installation experience. Nokiri does not modify Gatekeeper
settings. Keychain can request authorization
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
- For a notarized release, sign both executables and bundle with a Developer ID,
  notarize and staple using the maintainer's own Apple account. For the initial
  ad-hoc release, disclose the missing notarization and possible Gatekeeper block.
- Offer the exact corresponding source companion alongside the binary download.

## GitHub Actions builds

[CI](../.github/workflows/ci.yml) runs on pushes, pull requests and manual
`workflow_dispatch` runs. The macOS job uses the explicit `macos-26` Apple Silicon
runner, as in [Kadro](https://github.com/binido/Kadro/blob/main/.github/workflows/release.yml).
The Linux job continues to check shared Rust code on `ubuntu-latest`.

The macOS job runs the ordinary checks and real-core protocol tests, builds the
ad-hoc signed app, verifies both executable architectures, prepares the exact
Mihomo source companion and creates a verified DMG with an Applications shortcut.
Download `Nokiri-macos-26-arm64` from the workflow run's **Artifacts** section.
It contains `Nokiri-arm64.dmg` and `Mihomo-v1.19.34-source.tar.gz`, retained for
14 days. Keep the source companion alongside the DMG when sharing it.

The workflow uploads only these build outputs and creates no GitHub release.
It needs no signing secrets and retains read-only repository permissions.
Developer ID signing and notarization remain outside this workflow; Gatekeeper
can block a downloaded ad-hoc signed build.
