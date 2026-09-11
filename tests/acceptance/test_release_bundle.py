# SPDX-License-Identifier: AGPL-3.0-or-later
"""External inspection of the production tracer bundle."""

from __future__ import annotations

import json
import os
from pathlib import Path
import stat
import subprocess
import unittest


REPO = Path(__file__).resolve().parents[2]
BUNDLE = Path(os.environ.get("WORKFLOWD_BUNDLE", REPO / "out/tracer-bundle"))


class ReleaseBundleAcceptanceTest(unittest.TestCase):
    def test_bundle_is_runtime_only_locked_and_auditable(self) -> None:
        self.assertTrue(BUNDLE.is_dir(), "build the release bundle first")
        subprocess.run(
            ["sha256sum", "--check", "--strict", "--quiet", "checksums.sha256"],
            cwd=BUNDLE,
            check=True,
        )
        files = sorted(path for path in BUNDLE.rglob("*") if path.is_file())
        relative = [path.relative_to(BUNDLE).as_posix() for path in files]
        self.assertIn("usr/bin/workflowd", relative)
        self.assertIn("release/release-manifest.json", relative)
        self.assertIn("usr/share/doc/workflowd/sbom.cdx.json", relative)
        self.assertIn("usr/share/doc/workflowd/third-party-licenses.json", relative)
        self.assertIn(
            "usr/share/doc/workflowd/dependency-lock-checksums.json", relative
        )
        self.assertIn(
            "usr/share/workflowd/sdk/node-contract/v1alpha1/meta-schema.json", relative
        )
        self.assertIn(
            "usr/share/workflowd/contracts/manual-trigger.v1alpha1.json", relative
        )
        self.assertIn(
            "usr/share/workflowd/contracts/generate-items.v1alpha1.json", relative
        )
        generate_contract = json.loads(
            (BUNDLE / "usr/share/workflowd/contracts/generate-items.v1alpha1.json").read_text()
        )
        self.assertEqual(generate_contract["identity"]["name"], "generate-items")
        self.assertEqual(generate_contract["resources"]["hard"]["output_count"], 50_000)
        self.assertEqual(generate_contract["resources"]["hard"]["output_bytes"], 64 * 1024 * 1024)

        forbidden_suffixes = (".rs", ".tsx", ".ts", ".mjs", ".map", ".pyc")
        self.assertFalse([name for name in relative if name.endswith(forbidden_suffixes)])
        forbidden_parts = {
            "node_modules",
            "target",
            ".cargo",
            ".npm",
            "src",
            "editor",
        }
        self.assertFalse(
            [name for name in relative if forbidden_parts.intersection(name.split("/"))]
        )
        self.assertFalse(
            [
                name
                for name in relative
                if Path(name).name in {"node", "npm", "python", "python3", "cargo", "rustc"}
            ]
        )

        installed_bytes = sum(path.stat().st_size for path in files)
        self.assertLess(installed_bytes, 10 * 1024 * 1024 * 1024)

        executables = [
            path
            for path in files
            if path.stat().st_mode & (stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
        ]
        self.assertEqual(
            [path.relative_to(BUNDLE).as_posix() for path in executables],
            ["usr/bin/workflowd"],
        )

        manifest = json.loads((BUNDLE / "release/release-manifest.json").read_text())
        self.assertEqual(manifest["runtime"]["residentProcesses"], ["workflowd"])
        self.assertEqual(manifest["runtime"]["sqlite"], "bundled")
        self.assertEqual(manifest["ecoProfile"]["memoryMaxBytes"], 524288000)
        self.assertEqual(manifest["ecoProfile"]["cpuQuotaCores"], 0.5)
        self.assertEqual(manifest["ecoProfile"]["memorySwapMaxBytes"], 0)

        sbom = json.loads(
            (BUNDLE / "usr/share/doc/workflowd/sbom.cdx.json").read_text()
        )
        self.assertEqual(sbom["bomFormat"], "CycloneDX")
        self.assertGreater(len(sbom["components"]), 20)
        self.assertFalse(
            [component for component in sbom["components"] if not component["licenses"]]
        )
        playwright = [
            component
            for component in sbom["components"]
            if component["name"] in {"playwright", "playwright-core"}
        ]
        self.assertEqual(len(playwright), 2)
        self.assertTrue(all(component["scope"] == "excluded" for component in playwright))

        dynamic = subprocess.check_output(
            ["ldd", str(BUNDLE / "usr/bin/workflowd")], text=True
        ).lower()
        self.assertNotIn("libsqlite", dynamic)
        self.assertNotIn("libssl", dynamic)


if __name__ == "__main__":
    unittest.main()
