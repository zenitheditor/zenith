#!/usr/bin/env python3
"""Check release manifests, LICENSE copies, and bundled assets without extracting archives."""

import argparse
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tomllib


def run(command: list[str], root: Path) -> bytes:
    # Captured machine output must bypass ogt's size gate.
    env = dict(os.environ, OGT="0")
    result = subprocess.run(command, cwd=root, env=env, capture_output=True, check=False)
    if result.returncode:
        detail = result.stderr.decode("utf-8", errors="replace").strip()
        raise ValueError(f"Command error: {' '.join(command)}: {detail}. Check the workspace and rerun.")
    return result.stdout


def dependency_tables(manifest: dict):
    sections = ("dependencies", "dev-dependencies", "build-dependencies")
    for section in sections:
        yield manifest.get(section, {})
    for target in manifest.get("target", {}).values():
        for section in sections:
            yield target.get(section, {})


def archive_bytes(archive: tarfile.TarFile, members: dict, path: str) -> bytes:
    member = members.get(path)
    if member is None or not member.isfile():
        raise ValueError(f"Archive file missing: {path}. Include this file and run cargo package again.")
    source = archive.extractfile(member)
    if source is None:
        raise ValueError(f"Archive file unreadable: {path}. Run cargo package again.")
    with source:
        return source.read()


def check_package(package: dict, packages: dict, directory: Path, tracked: list[str], root: Path) -> int:
    name = package["name"]
    version = package["version"]
    prefix = f"{name}-{version}"
    archive_path = directory / f"{prefix}.crate"
    package_root = Path(package["manifest_path"]).parent
    asset_root = package_root.relative_to(root) / "assets"
    assets = [path for path in tracked if Path(path).is_relative_to(asset_root)]
    with tarfile.open(archive_path, "r:gz") as archive:
        members = {}
        for member in archive.getmembers():
            if member.name in members:
                raise ValueError(f"Duplicate archive entry: {member.name}. Run cargo package again.")
            members[member.name] = member
        manifest_path = f"{prefix}/Cargo.toml"
        manifest = tomllib.loads(archive_bytes(archive, members, manifest_path).decode("utf-8"))
        for field, expected in (
            ("name", name),
            ("version", version),
            ("rust-version", package["rust_version"]),
        ):
            actual = manifest.get("package", {}).get(field)
            if actual != expected:
                raise ValueError(
                    f"Manifest mismatch: {manifest_path} {field}={actual!r}, expected {expected!r}. "
                    "Correct the package manifest and run cargo package again."
                )
        license_file = f"{prefix}/LICENSE"
        if archive_bytes(archive, members, license_file) != (root / "LICENSE").read_bytes():
            raise ValueError(
                f"License mismatch: {license_file} differs from the root LICENSE. "
                "Copy the root LICENSE into the crate directory and run cargo package again."
            )
        for table in dependency_tables(manifest):
            for alias, dependency in table.items():
                dependency = {"version": dependency} if isinstance(dependency, str) else dependency
                dependency_name = dependency.get("package", alias)
                if dependency_name not in packages:
                    continue
                expected = f"={packages[dependency_name]['version']}"
                if dependency.get("version") != expected:
                    raise ValueError(
                        f"Dependency mismatch: {manifest_path} {alias}={dependency.get('version')!r}, "
                        f"expected {expected!r}. Pin the workspace dependency and run cargo package again."
                    )
        for path in assets:
            asset_path = root / path
            relative = asset_path.relative_to(package_root).as_posix()
            archive_file = f"{prefix}/{relative}"
            if archive_bytes(archive, members, archive_file) != asset_path.read_bytes():
                raise ValueError(
                    f"Asset mismatch: {archive_file} differs from {path}. Run cargo package again."
                )
    return len(assets)


def main() -> int:
    parser = argparse.ArgumentParser(description="Check release archives and bundled assets.")
    parser.add_argument(
        "directory", nargs="?", default="target/package", help="Read archives here (default: target/package)."
    )
    args = parser.parse_args()
    root = Path(__file__).resolve().parent.parent
    directory = Path(args.directory).resolve()
    try:
        metadata = json.loads(run(["cargo", "metadata", "--no-deps", "--format-version=1", "--offline"], root))
        workspace = set(metadata["workspace_members"])
        packages = {package["name"]: package for package in metadata["packages"] if package["id"] in workspace}
        tracked = [os.fsdecode(path) for path in run(["git", "ls-files", "-z"], root).split(b"\0") if path]
    except (OSError, ValueError) as error:
        print(f"Package input error: {error}. Check cargo metadata and git ls-files.", file=sys.stderr)
        return 1
    errors = 0
    checked = 0
    assets = 0
    for package in sorted(packages.values(), key=lambda package: package["name"]):
        if package["publish"] == []:
            continue
        archive_path = directory / f"{package['name']}-{package['version']}.crate"
        try:
            assets += check_package(package, packages, directory, tracked, root)
            checked += 1
        except (OSError, ValueError, tarfile.TarError) as error:
            print(f"Package check error: {archive_path}: {error}. Correct the archive and rerun.", file=sys.stderr)
            errors += 1
    if errors:
        return 1
    print(f"Checked {checked} release archives and {assets} bundled assets.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
