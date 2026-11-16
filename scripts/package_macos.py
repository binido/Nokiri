#!/usr/bin/env python3
"""Builds an unsigned Apple Silicon application with a verified standalone core."""
from pathlib import Path
import platform
import json
import plistlib
import shutil
import subprocess

from fetch_core import ROOT, install
from collect_licenses import collect

def package():
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise SystemExit("Packaging currently requires an Apple Silicon Mac.")
    core = install()
    subprocess.run(["cargo", "build", "--release", "--locked"], cwd=ROOT, check=True)
    bundle = ROOT / "dist/Nokiri.app/Contents"
    if bundle.parent.exists():
        shutil.rmtree(bundle.parent)
    for folder in ("MacOS", "Resources", "Resources/Licenses"):
        (bundle / folder).mkdir(parents=True, exist_ok=True)
    shutil.copy2(Path(json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version=1"], cwd=ROOT))["target_directory"]) / "release/nokiri", bundle / "MacOS/Nokiri")
    shutil.copy2(core, bundle / "Resources/mihomo")
    shutil.copy2(ROOT / "assets/core.json", bundle / "Resources/core.json")
    iconset = ROOT / "target/Nokiri.iconset"
    iconset.mkdir(parents=True, exist_ok=True)
    for size in (16, 32, 128, 256, 512):
        for scale in (1, 2):
            filename = f"icon_{size}x{size}{'@2x' if scale == 2 else ''}.png"
            subprocess.run(["sips", "-z", str(size * scale), str(size * scale), str(ROOT / "assets/icons/nokiri.png"), "--out", str(iconset / filename)], check=True, stdout=subprocess.DEVNULL)
    subprocess.run(["iconutil", "-c", "icns", str(iconset), "-o", str(bundle / "Resources/Nokiri.icns")], check=True)
    info = dict(CFBundleName="Nokiri", CFBundleDisplayName="Nokiri", CFBundleExecutable="Nokiri", CFBundleIdentifier="io.github.binido.nokiri", CFBundleIconFile="Nokiri", CFBundlePackageType="APPL", CFBundleShortVersionString="0.1.0", CFBundleVersion="1", LSMinimumSystemVersion="12.0", NSHighResolutionCapable=True, NSPrincipalClass="NSApplication", NSSupportsAutomaticGraphicsSwitching=True, NSHumanReadableCopyright="Copyright 2026 Binido. Nokiri MIT; bundled Mihomo GPL-3.0.")
    (bundle / "Info.plist").write_bytes(plistlib.dumps(info))
    for source, name in [(ROOT / "LICENSE", "Nokiri-MIT.txt"), (ROOT / "assets/MIHOMO-LICENSE", "Mihomo-GPL-3.0.txt"), (ROOT / "assets/fonts/OFL.txt", "Roboto-OFL.txt"), (ROOT / "assets/fonts/MaterialSymbols-LICENSE", "MaterialSymbols-Apache-2.0.txt"), (ROOT / "THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md")]:
        shutil.copy2(source, bundle / "Resources/Licenses" / name)
    collect(bundle / "Resources/Licenses/Rust")
    # Apple Silicon needs an ad-hoc app signature for local execution; this is not Developer ID signing.
    subprocess.run(["codesign", "--force", "--sign", "-", str(bundle.parent)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(bundle.parent)], check=True)
    print(f"Built {bundle.parent}\nAd-hoc signed locally. Not Developer ID signed or notarized.")
    return bundle.parent

if __name__ == "__main__":
    package()
