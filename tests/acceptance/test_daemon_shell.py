# SPDX-License-Identifier: AGPL-3.0-or-later
"""External acceptance tests for the production-shaped daemon shell."""

from __future__ import annotations

import json
import os
from pathlib import Path
import re
import socket
import ssl
import subprocess
import tempfile
import time
import unittest
from urllib.error import URLError
from urllib.request import Request, urlopen


REPO = Path(__file__).resolve().parents[2]
DAEMON = Path(os.environ.get("WORKFLOWD_BIN", REPO / "target/debug/workflowd"))


def free_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return int(listener.getsockname()[1])


def request(
    url: str, context: ssl.SSLContext | None = None
) -> tuple[int, dict[str, str], bytes]:
    with urlopen(Request(url), timeout=2, context=context) as response:
        return (
            response.status,
            {key.lower(): value for key, value in response.headers.items()},
            response.read(),
        )


class RunningDaemon:
    def __init__(self, state_dir: Path, **environment: str) -> None:
        if not DAEMON.is_file():
            raise AssertionError(
                f"daemon is missing at {DAEMON}; build it before running acceptance tests"
            )
        self.port = free_port()
        env = os.environ.copy()
        env.update(
            {
                "WORKFLOWD_BIND": f"127.0.0.1:{self.port}",
                "WORKFLOWD_STATE_DIR": str(state_dir),
                **environment,
            }
        )
        self.scheme = "https" if "WORKFLOWD_TLS_CERT" in environment else "http"
        self.context = (
            ssl._create_unverified_context() if self.scheme == "https" else None
        )
        self.process = subprocess.Popen(
            [str(DAEMON), "serve"],
            cwd=REPO,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
        )
        self.wait_until_ready()

    @property
    def origin(self) -> str:
        return f"{self.scheme}://127.0.0.1:{self.port}"

    def get(self, path: str) -> tuple[int, dict[str, str], bytes]:
        return request(f"{self.origin}{path}", self.context)

    def wait_until_ready(self) -> None:
        deadline = time.monotonic() + 10
        while time.monotonic() < deadline:
            if self.process.poll() is not None:
                output = self.process.stdout.read() if self.process.stdout else ""
                self._close_output()
                raise AssertionError(
                    f"daemon exited with {self.process.returncode}:\n{output}"
                )
            try:
                status, _, _ = self.get("/health/ready")
                if status == 200:
                    return
            except (ConnectionError, URLError, TimeoutError):
                pass
            time.sleep(0.05)
        self.stop()
        raise AssertionError("daemon did not become ready")

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
        self._close_output()

    def _close_output(self) -> None:
        if self.process.stdout and not self.process.stdout.closed:
            self.process.stdout.close()

    def __enter__(self) -> "RunningDaemon":
        return self

    def __exit__(self, *_: object) -> None:
        self.stop()


class DaemonShellAcceptanceTest(unittest.TestCase):
    def test_http_surface_reports_ready_and_serves_embedded_shell(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            with RunningDaemon(state_dir) as daemon:
                live_status, _, live_body = daemon.get("/health/live")
                self.assertEqual(live_status, 200)
                self.assertEqual(json.loads(live_body)["status"], "alive")

                ready_status, _, ready_body = daemon.get("/health/ready")
                ready = json.loads(ready_body)
                self.assertEqual(ready_status, 200)
                self.assertEqual(ready["status"], "ready")
                self.assertEqual(ready["checks"]["sqlite"]["journal_mode"], "wal")
                self.assertEqual(ready["checks"]["sqlite"]["synchronous"], "full")
                self.assertTrue(ready["checks"]["sqlite"]["foreign_keys"])
                self.assertEqual(
                    ready["checks"]["sqlite"]["busy_timeout_millis"], 5000
                )
                self.assertEqual(
                    ready["checks"]["sqlite"]["max_bound_parameters"], 999
                )
                self.assertTrue(ready["checks"]["sqlite"]["approved_runtime"])

                _, _, release_body = daemon.get("/api/v1/release")
                release = json.loads(release_body)
                self.assertEqual(release["product"], "Canopy Workbench")
                self.assertRegex(release["version"], r"^0\.1\.0$")
                self.assertEqual(release["api_version"], "v1")
                self.assertTrue(release["editor_manifest_sha256"])

                _, _, capability_body = daemon.get("/api/v1/capabilities")
                capability = json.loads(capability_body)
                self.assertEqual(capability["api_version"], "v1")
                self.assertIn("embedded-editor-shell", capability["capabilities"])
                self.assertEqual(capability["runtime"]["tokio_core_workers"], 1)
                self.assertLessEqual(capability["runtime"]["blocking_threads_max"], 2)
                self.assertEqual(
                    capability["runtime"]["database_queue_capacity"], 32
                )

                shell_status, shell_headers, shell_body = daemon.get("/")
                shell = shell_body.decode("utf-8")
                self.assertEqual(shell_status, 200)
                self.assertIn("Canopy Workbench", shell)
                self.assertEqual(shell_headers["cache-control"], "no-cache")
                asset_path = re.search(r'src="([^"]*main-[^"]+\.js)"', shell)
                self.assertIsNotNone(asset_path)

                asset_status, asset_headers, asset_body = daemon.get(
                    asset_path.group(1)
                )
                self.assertEqual(asset_status, 200)
                self.assertGreater(len(asset_body), 100)
                self.assertIn("immutable", asset_headers["cache-control"])
                self.assertTrue(asset_headers["etag"].startswith('"sha256-'))

            database = state_dir / "workflow.sqlite3"
            self.assertTrue(database.is_file())
            self.assertEqual(oct(state_dir.stat().st_mode & 0o777), "0o700")
            self.assertEqual(oct(database.stat().st_mode & 0o777), "0o600")

    def test_direct_https_uses_the_same_versioned_surface(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            certificate = root / "certificate.pem"
            private_key = root / "private-key.pem"
            subprocess.run(
                [
                    "openssl",
                    "req",
                    "-x509",
                    "-newkey",
                    "rsa:2048",
                    "-nodes",
                    "-days",
                    "1",
                    "-subj",
                    "/CN=localhost",
                    "-keyout",
                    str(private_key),
                    "-out",
                    str(certificate),
                ],
                check=True,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            with RunningDaemon(
                root / "state",
                WORKFLOWD_TLS_CERT=str(certificate),
                WORKFLOWD_TLS_KEY=str(private_key),
            ) as daemon:
                status, headers, body = daemon.get("/api/v1/capabilities")
                self.assertEqual(status, 200)
                self.assertEqual(headers["content-type"], "application/json")
                self.assertEqual(json.loads(body)["api_version"], "v1")

    def test_startup_fails_closed_for_an_unapproved_sqlite_runtime(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            state_dir = Path(temporary) / "state"
            environment = os.environ.copy()
            environment.update(
                {
                    "WORKFLOWD_STATE_DIR": str(state_dir),
                    "WORKFLOWD_BIND": f"127.0.0.1:{free_port()}",
                    "WORKFLOWD_SQLITE_MIN_VERSION": "3999999",
                }
            )
            result = subprocess.run(
                [str(DAEMON), "serve"],
                cwd=REPO,
                env=environment,
                capture_output=True,
                text=True,
                timeout=5,
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("outside approved range", result.stderr)
            self.assertFalse((state_dir / "workflow.sqlite3").exists())

    def test_cgroup_v2_values_and_missing_controllers_are_reported_honestly(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixture = root / "cgroup"
            fixture.mkdir()
            values = {
                "cpu.max": "50000 100000\n",
                "cpu.stat": "usage_usec 1234\nnr_throttled 2\nthrottled_usec 88\n",
                "memory.current": "1048576\n",
                "memory.max": "524288000\n",
                "memory.high": "471859200\n",
                "memory.peak": "2097152\n",
                "memory.swap.max": "0\n",
                "pids.current": "4\n",
                "pids.max": "64\n",
            }
            for name, value in values.items():
                (fixture / name).write_text(value)

            with RunningDaemon(
                root / "state", WORKFLOWD_CGROUP_DIR=str(fixture)
            ) as daemon:
                _, _, body = daemon.get("/api/v1/resources")
                resources = json.loads(body)
                self.assertTrue(resources["cpu"]["available"])
                self.assertEqual(resources["cpu"]["values"]["quota_cores"], 0.5)
                self.assertEqual(
                    resources["memory"]["values"]["max_bytes"], 524288000
                )
                self.assertEqual(resources["tasks"]["values"]["max"], 64)

            missing = root / "missing-cgroup"
            missing.mkdir()
            with RunningDaemon(
                root / "second-state", WORKFLOWD_CGROUP_DIR=str(missing)
            ) as daemon:
                _, _, body = daemon.get("/api/v1/resources")
                resources = json.loads(body)
                self.assertFalse(resources["cpu"]["available"])
                self.assertEqual(resources["cpu"]["reason"], "cpu.max unavailable")
                self.assertFalse(resources["memory"]["available"])
                self.assertFalse(resources["tasks"]["available"])


if __name__ == "__main__":
    unittest.main()
