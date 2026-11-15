#!/usr/bin/env python3
"""Installs the pinned official core after verifying both artifact and executable."""
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import tempfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent

def install():
    if platform.system() != "Darwin" or platform.machine() != "arm64":
        raise SystemExit("This core distribution currently supports macOS Apple Silicon only.")
    manifest = json.loads((ROOT / "assets/core.json").read_text())
    destination = ROOT / "target/core/mihomo"
    if destination.exists() and hashlib.sha256(destination.read_bytes()).hexdigest() == manifest["binary_sha256"]:
        print(f"Verified Mihomo {manifest['version']}: {destination}")
        return destination
    request = urllib.request.Request(manifest["url"], headers={"User-Agent": "Nokiri-build/0.1"})
    with urllib.request.urlopen(request, timeout=60) as response:
        compressed = response.read(40 * 1024 * 1024 + 1)
    if hashlib.sha256(compressed).hexdigest() != manifest["artifact_sha256"]:
        raise SystemExit("Official core artifact checksum mismatch; nothing was installed.")
    binary = gzip.decompress(compressed)
    if hashlib.sha256(binary).hexdigest() != manifest["binary_sha256"]:
        raise SystemExit("Core executable checksum mismatch; nothing was installed.")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.NamedTemporaryFile(dir=destination.parent, delete=False) as temp:
        temp.write(binary)
        temp.flush()
        os.fsync(temp.fileno())
        name = temp.name
    os.chmod(name, 0o755)
    os.replace(name, destination)
    print(f"Installed verified Mihomo {manifest['version']}: {destination}")
    return destination

if __name__ == "__main__":
    install()
