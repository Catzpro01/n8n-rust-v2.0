#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Generate deterministic dependency/license/SBOM material for the tracer bundle."""

from __future__ import annotations

from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tomllib
from urllib.parse import quote


REPO = Path(__file__).resolve().parents[1]


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: release_metadata.py BUNDLE")
    bundle = Path(sys.argv[1]).resolve()
    release_dir = bundle / "release"
    documentation_dir = bundle / "usr/share/doc/workflowd"
    release_dir.mkdir(parents=True, exist_ok=True)
    documentation_dir.mkdir(parents=True, exist_ok=True)

    cargo_components, cargo_locks = cargo_dependencies()
    npm_components, npm_locks = npm_dependencies()
    components = sorted(
        cargo_components + npm_components,
        key=lambda component: component["bom-ref"],
    )
    locks = sorted(cargo_locks + npm_locks, key=lambda lock: lock["purl"])
    missing_licenses = [
        component["bom-ref"] for component in components if not component.get("licenses")
    ]
    if missing_licenses:
        raise SystemExit(f"dependencies without declared licenses: {missing_licenses}")

    epoch = int(os.environ.get("SOURCE_DATE_EPOCH", "0"))
    timestamp = datetime.fromtimestamp(epoch, timezone.utc).isoformat().replace("+00:00", "Z")
    sbom = {
        "bomFormat": "CycloneDX",
        "specVersion": "1.5",
        "version": 1,
        "metadata": {
            "timestamp": timestamp,
            "component": {
                "type": "application",
                "bom-ref": "pkg:cargo/workflowd@0.1.0",
                "name": "workflowd",
                "version": "0.1.0",
                "licenses": [{"expression": "AGPL-3.0-or-later"}],
            },
            "tools": {
                "components": [
                    {
                        "type": "application",
                        "name": "release_metadata.py",
                        "version": "1",
                    }
                ]
            },
        },
        "components": components,
    }
    write_json(documentation_dir / "sbom.cdx.json", sbom)
    write_json(documentation_dir / "dependency-lock-checksums.json", locks)
    write_json(
        documentation_dir / "third-party-licenses.json",
        [
            {
                "name": component["name"],
                "version": component["version"],
                "purl": component["purl"],
                "license": component["licenses"][0]["expression"],
                "scope": component.get("scope", "required"),
            }
            for component in components
        ],
    )

    binary = bundle / "usr/bin/workflowd"
    editor_manifest = REPO / "editor/dist/asset-manifest.json"
    release_manifest = {
        "schema": 1,
        "product": "Canopy Workbench",
        "version": "0.1.0",
        "apiVersion": "v1",
        "buildCommit": os.environ.get("WORKFLOWD_BUILD_COMMIT", "development"),
        "sourceDateEpoch": epoch,
        "binary": {
            "path": "usr/bin/workflowd",
            "sha256": sha256(binary),
            "bytes": binary.stat().st_size,
            "embeddedEditorManifestSha256": sha256(editor_manifest),
        },
        "runtime": {
            "residentProcesses": ["workflowd"],
            "sqlite": "bundled",
            "tokioCoreWorkers": 1,
            "blockingThreadsMax": 2,
        },
        "ecoProfile": {
            "memoryMaxBytes": 524_288_000,
            "cpuQuotaCores": 0.5,
            "memorySwapMaxBytes": 0,
            "tasksMax": 64,
            "managedDiskMaxBytes": 10_737_418_240,
        },
        "materials": {
            "cargoLockSha256": sha256(REPO / "Cargo.lock"),
            "npmLockSha256": sha256(REPO / "editor/package-lock.json"),
            "sbom": "usr/share/doc/workflowd/sbom.cdx.json",
            "licenses": "usr/share/doc/workflowd/third-party-licenses.json",
            "dependencyChecksums": "usr/share/doc/workflowd/dependency-lock-checksums.json",
            "nodeContractSdk": "usr/share/workflowd/sdk/node-contract",
            "nativeContracts": "usr/share/workflowd/contracts",
        },
    }
    write_json(release_dir / "release-manifest.json", release_manifest)


def cargo_dependencies() -> tuple[list[dict], list[dict]]:
    environment = os.environ.copy()
    environment["PATH"] = f"{Path.home() / '.cargo/bin'}:{environment.get('PATH', '')}"
    metadata = json.loads(
        subprocess.check_output(
            ["cargo", "+1.85.1", "metadata", "--locked", "--format-version", "1"],
            cwd=REPO,
            env=environment,
            text=True,
        )
    )
    lock = tomllib.loads((REPO / "Cargo.lock").read_text())
    checksum_by_key = {
        (package["name"], package["version"], package.get("source", "")): package.get(
            "checksum"
        )
        for package in lock["package"]
    }
    components = []
    locks = []
    for package in metadata["packages"]:
        if package["source"] is None:
            continue
        purl = f"pkg:cargo/{quote(package['name'])}@{quote(package['version'])}"
        license_expression = package.get("license") or ""
        component = {
            "type": "library",
            "bom-ref": purl,
            "name": package["name"],
            "version": package["version"],
            "purl": purl,
            "licenses": ([{"expression": license_expression}] if license_expression else []),
            "scope": "required",
        }
        source = str(package["source"])
        checksum = checksum_by_key.get((package["name"], package["version"], source))
        if checksum:
            component["hashes"] = [{"alg": "SHA-256", "content": checksum}]
            locks.append({"purl": purl, "algorithm": "SHA-256", "checksum": checksum})
        components.append(component)
    return components, locks


def npm_dependencies() -> tuple[list[dict], list[dict]]:
    lock = json.loads((REPO / "editor/package-lock.json").read_text())
    components = []
    locks = []
    for path, package in lock.get("packages", {}).items():
        if not path or "node_modules/" not in path:
            continue
        name = package.get("name") or path.rsplit("node_modules/", 1)[1]
        version = package["version"]
        purl = f"pkg:npm/{quote(name, safe='@/')}@{quote(version)}"
        license_expression = package.get("license") or ""
        if package.get("dev"):
            scope = "excluded"
        elif package.get("optional"):
            scope = "optional"
        else:
            scope = "required"
        component = {
            "type": "library",
            "bom-ref": purl,
            "name": name,
            "version": version,
            "purl": purl,
            "licenses": ([{"expression": license_expression}] if license_expression else []),
            "scope": scope,
        }
        integrity = package.get("integrity")
        if integrity and "-" in integrity:
            algorithm, checksum = integrity.split("-", 1)
            normalized = algorithm.replace("sha", "SHA-").upper()
            component["hashes"] = [{"alg": normalized, "content": checksum}]
            locks.append({"purl": purl, "algorithm": normalized, "checksum": checksum})
        components.append(component)
    return components, locks


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def write_json(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


if __name__ == "__main__":
    main()
