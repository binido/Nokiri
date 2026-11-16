#!/usr/bin/env python3
"""Prepares the pinned Mihomo corresponding source with vendored Go dependencies."""
import hashlib
import json
from pathlib import Path
import subprocess
import tarfile
import urllib.request

ROOT = Path(__file__).resolve().parent.parent

def prepare():
    manifest = json.loads((ROOT / "assets/core.json").read_text())
    manifest.update(json.loads((ROOT / "assets/core-source.json").read_text()))
    workspace = ROOT / "target/core-source"
    workspace.mkdir(parents=True, exist_ok=True)
    source = workspace / f"mihomo-{manifest['version']}.tar.gz"
    if not source.exists():
        with urllib.request.urlopen(manifest["source_archive_url"], timeout=60) as response:
            source.write_bytes(response.read(40 * 1024 * 1024 + 1))
    if hashlib.sha256(source.read_bytes()).hexdigest() != manifest["source_archive_sha256"]:
        raise SystemExit("Mihomo source checksum mismatch; source archive rejected.")
    with tarfile.open(source) as archive:
        for member in archive:
            relative = Path(member.name)
            if relative.is_absolute() or ".." in relative.parts or not (member.isfile() or member.isdir()):
                raise SystemExit("Unsafe source archive entry rejected.")
            output = workspace / relative
            if member.isdir():
                output.mkdir(parents=True, exist_ok=True)
            else:
                output.parent.mkdir(parents=True, exist_ok=True)
                output.write_bytes(archive.extractfile(member).read())
                output.chmod(member.mode & 0o777)
    directory = workspace / f"mihomo-{manifest['version'].removeprefix('v')}"
    subprocess.run(["go", "mod", "vendor"], cwd=directory, check=True)
    assert (directory / "vendor/modules.txt").is_file()
    assert (directory / "LICENSE").is_file()
    (directory / "NOKIRI-SOURCE-NOTES.txt").write_text(
        f"Unmodified Mihomo {manifest['version']} source, with dependencies vendored by go mod vendor.\n"
        f"Official binary provenance and SHA-256: {json.dumps(manifest)}\n"
        "Build recipes are in Makefile and .github/workflows. Use -mod=vendor for offline builds.\n"
        "The supplied official binary was not rebuilt by Nokiri; bit-for-bit reproduction is not claimed.\n")
    destination = ROOT / "dist" / f"Mihomo-{manifest['version']}-source.tar.gz"
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(destination, "w:gz") as archive:
        archive.add(directory, arcname=directory.name)
    print(f"Prepared {destination}")
    return destination

if __name__ == "__main__":
    prepare()
