# Third-party notices

Nokiri's original Rust application and artwork are MIT licensed (see LICENSE).
The separately executed Mihomo core is not relicensed by Nokiri's MIT license.

## Mihomo v1.19.34

- Upstream: https://github.com/MetaCubeX/mihomo/tree/v1.19.34
- License: GPL-3.0, reproduced in assets/MIHOMO-LICENSE and the app bundle.
- Official release: https://github.com/MetaCubeX/mihomo/releases/tag/v1.19.34
- Reproducible pin and SHA-256 digests: assets/core.json.

The core runs as a separate, unmodified executable and communicates through its
published localhost HTTP API. Nokiri does not link Mihomo code into the Rust binary.
The MIT license applies to Nokiri's source, not to every file in the distribution.
A bundle redistributing Mihomo must meet its GPL obligations independently.

Before distributing a bundle to others, supply the exact corresponding Mihomo
source (including upstream build scripts and dependency source required to rebuild
that release) alongside the binaries under GPL-3.0 section 6, and retain notices.
A link to upstream alone is not a substitute for the distributor's source obligation.
`scripts/prepare_core_source.py` creates a companion source archive with the
pinned upstream source, build scripts and vendored Go dependencies.
The packaging command creates a local application, not a release publication or a
claim that a source distribution has already been prepared. There is no automatic
release workflow. See docs/distribution.md for the release checklist.

## Roboto

Copyright 2011 The Roboto Project Authors (https://github.com/googlefonts/roboto-classic).
SIL Open Font License 1.1, reproduced in assets/fonts/OFL.txt. The bundled font is
a static regular instance of Google's official variable font, obtained from
https://github.com/google/fonts/tree/main/ofl/roboto.

## Material Symbols Rounded

Copyright Google. Apache License 2.0, reproduced in assets/fonts/MaterialSymbols-LICENSE.
https://github.com/google/material-design-icons. Only the navigation glyphs used
by Nokiri are bundled, as a static 24 px / weight 400 subset. Nokiri is not endorsed
by Google. Icons8's MIT Liquid Glass set was evaluated; its multicolor React assets
were less suitable for egui's compact monochrome controls. No Icons8 assets are bundled.

## Rust dependencies

Cargo.lock records exact dependency versions. The packaging script includes
notices from the active target graph and exact upstream commits when published
crates omit root license files. dispatch 0.2.0 declares MIT without supplying
standalone terms; its notice reproduces standard MIT terms and its author
attribution from the published manifest. A distributable release must
include the complete notices for the dependency graph used for that target; run
`scripts/collect_licenses.py` during packaging to collect these in the app bundle.
