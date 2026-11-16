#!/usr/bin/env python3
"""Collects notices from locked crates and their exact published source commits."""
from concurrent.futures import ThreadPoolExecutor
import io
import json
from pathlib import Path
import shutil
import subprocess
import tarfile
import urllib.parse
import urllib.request

ROOT = Path(__file__).resolve().parent.parent

def is_notice(name):
    return name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE", "UNLICENSE", "OFL", "UFL"))

def local_notices(package):
    directory = Path(package["manifest_path"]).parent
    files = [file for file in directory.rglob("*") if file.is_file() and is_notice(file.name)]
    if package.get("license_file"):
        file = directory / package["license_file"]
        if file.is_file() and file not in files:
            files.append(file)
    return directory, files

def source_archive(package):
    directory = Path(package["manifest_path"]).parent
    vcs = directory / ".cargo_vcs_info.json"
    repo = urllib.parse.urlparse(package.get("repository") or "")
    if repo.netloc != "github.com" or not vcs.exists():
        raise RuntimeError(f"Missing license for {package['name']}; inspect its upstream source before redistribution")
    commit = json.loads(vcs.read_text())["git"]["sha1"]
    repo_path = "/".join(repo.path.strip("/").removesuffix(".git").split("/")[:2])
    return repo_path, commit

def download_source(source):
    repo, commit = source
    cache = ROOT / "target/license-sources" / f"{repo.replace('/', '-')}-{commit}.tar.gz"
    if not cache.exists():
        cache.parent.mkdir(parents=True, exist_ok=True)
        url = f"https://codeload.github.com/{repo}/tar.gz/{commit}"
        with urllib.request.urlopen(url, timeout=60) as response:
            data = response.read(80 * 1024 * 1024 + 1)
        if len(data) > 80 * 1024 * 1024:
            raise RuntimeError(f"Source archive exceeds limit: {repo}")
        cache.write_bytes(data)
    return source, cache

def collect(destination):
    host = next(line.split(": ", 1)[1] for line in subprocess.check_output(["rustc", "-vV"], text=True).splitlines() if line.startswith("host:"))
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--locked", "--format-version=1", "--filter-platform", host], cwd=ROOT))
    tree = subprocess.check_output(["cargo", "tree", "--locked", "--target", host, "--edges", "normal,build", "--prefix", "none", "--format", "{p}"], cwd=ROOT, text=True)
    active = {(line.split()[0], line.split()[1].removeprefix("v")) for line in tree.splitlines() if len(line.split()) >= 2}
    packages = [p for p in metadata["packages"] if p["source"] is not None and (p["name"], p["version"]) in active]
    sources = {source_archive(p) for p in packages if not local_notices(p)[1]}
    with ThreadPoolExecutor(max_workers=4) as pool:
        archives = dict(pool.map(download_source, sorted(sources)))
    destination.mkdir(parents=True, exist_ok=True)
    index = []
    for package in packages:
        directory, files = local_notices(package)
        target = destination / f"{package['name']}-{package['version']}"
        target.mkdir(exist_ok=True)
        included = []
        for file in files:
            relative = file.relative_to(directory)
            output = target / relative
            output.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(file, output)
            included.append(str(relative))
        if not files:
            with tarfile.open(archives[source_archive(package)], "r:gz") as archive:
                for member in archive:
                    relative = Path(*Path(member.name).parts[1:])
                    if member.isfile() and is_notice(relative.name) and ".." not in relative.parts:
                        output = target / "upstream" / relative
                        output.parent.mkdir(parents=True, exist_ok=True)
                        output.write_bytes(archive.extractfile(member).read())
                        included.append(str(Path("upstream") / relative))
        if not included and package["name"] == "dispatch" and package["version"] == "0.2.0":
            shutil.copy2(ROOT / "assets/licenses/dispatch-MIT.txt", target / "LICENSE-MIT.txt")
            shutil.copy2(directory / "Cargo.toml", target / "UPSTREAM-Cargo.toml")
            included = ["LICENSE-MIT.txt", "UPSTREAM-Cargo.toml"]
        if not included:
            raise RuntimeError(f"No license text found for {package['name']}")
        index.append(dict(name=package["name"], version=package["version"], license=package.get("license"), repository=package.get("repository"), included_files=sorted(included)))
    (destination / "index.json").write_text(json.dumps(index, indent=2) + "\n")
    assert len(index) == len(packages) and all(p["included_files"] for p in index)
    print(f"Collected notices for {len(index)} locked Rust crates")

if __name__ == "__main__":
    collect(ROOT / "target/third-party-licenses")
