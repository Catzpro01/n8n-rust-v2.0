# SPDX-License-Identifier: AGPL-3.0-or-later
"""Public-seam acceptance for durable Manual Trigger Runs and Causal Trace."""

from __future__ import annotations

import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

REPO = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get("WORKFLOWD_BIN", REPO / "target/debug/workflowd"))


def free_port() -> int:
    with socket.socket() as listener:
        listener.bind(("127.0.0.1", 0))
        return listener.getsockname()[1]


def api(origin, path, method="GET", body=None, headers=None):
    request = Request(
        origin + path,
        data=None if body is None else json.dumps(body).encode(),
        method=method,
        headers={"Content-Type": "application/json", **(headers or {})},
    )
    try:
        with urlopen(request, timeout=10) as response:
            return (
                response.status,
                {key.lower(): value for key, value in response.headers.items()},
                json.loads(response.read() or b"{}"),
            )
    except HTTPError as error:
        return (
            error.code,
            {key.lower(): value for key, value in error.headers.items()},
            json.loads(error.read() or b"{}"),
        )


class Daemon:
    def __init__(self, state: Path, key: Path):
        self.port = free_port()
        self.origin = f"http://127.0.0.1:{self.port}"
        environment = os.environ.copy()
        environment.update(
            {
                "WORKFLOWD_BIND": f"127.0.0.1:{self.port}",
                "WORKFLOWD_CONTROL_ORIGIN": self.origin,
                "WORKFLOWD_STATE_DIR": str(state),
                "WORKFLOWD_MASTER_KEY_FILE": str(key),
                "WORKFLOWD_ARGON_MEMORY_KIB": "8192",
                "WORKFLOWD_ARGON_ITERATIONS": "1",
                "WORKFLOWD_DRAFT_LEASE_TTL_SECONDS": "30",
            }
        )
        self.process = subprocess.Popen(
            [str(BIN), "serve"],
            cwd=REPO,
            env=environment,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.PIPE,
            text=True,
        )
        for _ in range(400):
            try:
                if api(self.origin, "/health/live")[0] == 200:
                    return
            except (URLError, ConnectionError):
                pass
            time.sleep(0.05)
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(8)
        stderr = self.process.stderr.read() if self.process.stderr else ""
        raise AssertionError(f"daemon did not start: {stderr}")

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(8)
        if self.process.stderr:
            self.process.stderr.close()


def login(origin: str):
    origin_header = {"Origin": origin}
    status, response_headers, body = api(
        origin,
        "/api/v1/session/login",
        "POST",
        {"email": "owner@example.test", "password": "correct horse battery staple"},
        origin_header,
    )
    assert status == 200
    cookie = response_headers["set-cookie"].split(";", 1)[0]
    auth = {"Cookie": cookie}
    mutation = {**auth, **origin_header, "X-Canopy-CSRF": body["csrf_token"]}
    return auth, mutation


def command(session, generation, command_id, base, operation):
    return {
        "editor_session_id": session,
        "lease_generation": generation,
        "command_id": command_id,
        "base_draft_version": base,
        "operation": operation,
    }


def read_sse(origin: str, path: str, headers: dict[str, str], events: int):
    request = Request(origin + path, headers=headers)
    response = urlopen(request, timeout=5)
    parsed = []
    current = {}
    try:
        while len(parsed) < events:
            line = response.readline().decode().rstrip("\r\n")
            if line == "":
                if current:
                    parsed.append(current)
                    current = {}
                continue
            if line.startswith(":"):
                continue
            field, _, value = line.partition(":")
            current[field] = value.lstrip()
    finally:
        response.close()
    return parsed


class RunManualTriggerAcceptance(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        root = Path(self.temporary.name)
        self.state = root / "state"
        self.key = root / "master.key"
        self.key.write_bytes(os.urandom(32))
        self.daemon = Daemon(self.state, self.key)
        self.addCleanup(lambda: self.daemon.stop())
        setup = api(
            self.daemon.origin,
            "/api/v1/setup",
            "POST",
            {
                "email": "owner@example.test",
                "password": "correct horse battery staple",
                "recovery_passphrase": "separate recovery phrase long",
            },
            {"Origin": self.daemon.origin},
        )
        self.assertEqual(setup[0], 201)
        self.auth, self.mutation = login(self.daemon.origin)
        self.publication = self.publish_manual()

    def restart(self):
        self.daemon.stop()
        self.daemon = Daemon(self.state, self.key)
        self.auth, self.mutation = login(self.daemon.origin)

    def publish_manual(self):
        origin = self.daemon.origin
        workflow_id = "wf-runnable-manual"
        catalog = api(origin, "/api/v1/catalog", headers=self.auth)[2]
        contract_lock = catalog["nodes"][0]["contract_lock"]
        self.assertEqual(
            api(
                origin,
                "/api/v1/workflows",
                "POST",
                {
                    "workflow_id": workflow_id,
                    "name": "Runnable Manual Trigger",
                    "annotation": "durable run fixture",
                    "settings": {},
                    "compatibility_metadata": {},
                },
                self.mutation,
            )[0],
            201,
        )
        lease = api(
            origin,
            f"/api/v1/workflows/{workflow_id}/editing/open",
            "POST",
            {"editor_session_id": "run-tab", "label": "Run tab"},
            self.mutation,
        )[2]
        self.lease_generation = lease["lease_generation"]
        added = api(
            origin,
            f"/api/v1/workflows/{workflow_id}/draft-commands",
            "POST",
            command(
                "run-tab",
                lease["lease_generation"],
                "add-runnable-manual",
                0,
                {
                    "kind": "add_node",
                    "node_instance": {
                        "id": "manual-trigger",
                        "name": "Manual Trigger",
                        "contract_lock": contract_lock,
                        "configuration": {"capture_mode": "manual"},
                        "layout": {"x": 160, "y": 120},
                        "annotation": "",
                        "compatibility_metadata": {},
                    },
                },
            ),
            self.mutation,
        )[2]
        self.draft_version = added["draft_version"]
        preview_request = {
            "editor_session_id": "run-tab",
            "lease_generation": self.lease_generation,
            "draft_version": self.draft_version,
        }
        preview = api(
            origin,
            f"/api/v1/workflows/{workflow_id}/compile-preview",
            "POST",
            preview_request,
            self.mutation,
        )[2]
        warnings = [
            item["fingerprint"] for item in preview["diagnostics"] if item["requires_ack"]
        ]
        published = api(
            origin,
            f"/api/v1/workflows/{workflow_id}/publish",
            "POST",
            {
                "publication_id": "publish-runnable-manual",
                **preview_request,
                "compile_input_digest": preview["compile_input_digest"],
                "acknowledged_diagnostics": warnings,
            },
            self.mutation,
        )
        self.assertEqual(published[0], 201)
        return api(
            origin,
            f"/api/v1/workflows/{workflow_id}/publication",
            headers=self.auth,
        )[2]

    def publish_second_revision(self):
        origin = self.daemon.origin
        changed = api(
            origin,
            "/api/v1/workflows/wf-runnable-manual/draft-commands",
            "POST",
            command(
                "run-tab",
                self.lease_generation,
                "second-run-revision",
                self.draft_version,
                {"kind": "set_workflow_annotation", "annotation": "second exact target"},
            ),
            self.mutation,
        )[2]
        self.draft_version = changed["draft_version"]
        preview_request = {
            "editor_session_id": "run-tab",
            "lease_generation": self.lease_generation,
            "draft_version": self.draft_version,
        }
        preview = api(
            origin,
            "/api/v1/workflows/wf-runnable-manual/compile-preview",
            "POST",
            preview_request,
            self.mutation,
        )[2]
        warnings = [
            item["fingerprint"] for item in preview["diagnostics"] if item["requires_ack"]
        ]
        published = api(
            origin,
            "/api/v1/workflows/wf-runnable-manual/publish",
            "POST",
            {
                "publication_id": "publish-runnable-manual-two",
                **preview_request,
                "compile_input_digest": preview["compile_input_digest"],
                "acknowledged_diagnostics": warnings,
            },
            self.mutation,
        )
        self.assertEqual(published[0], 201)
        return api(
            origin,
            "/api/v1/workflows/wf-runnable-manual/publication",
            headers=self.auth,
        )[2]

    def request(self, request_id: str, invocation=None):
        current = self.publication["current_published"]
        event = self.publication["current_event"]["envelope"]
        return {
            "run_request_id": request_id,
            "publication_event_id": event["event_id"],
            "revision_id": current["revision_id"],
            "plan_digest": current["plan_digest"],
            "captured_invocation": invocation or {"manual": True},
        }

    def wait_terminal(self, run_id: str):
        for _ in range(200):
            response = api(
                self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth
            )
            self.assertEqual(response[0], 200)
            if response[2]["durable"]["terminal"]:
                return response[2]
            time.sleep(0.01)
        self.fail("Run did not become terminal")

    def test_durable_run_idempotency_trace_sse_cancel_and_restart(self):
        origin = self.daemon.origin
        path = "/api/v1/workflows/wf-runnable-manual/runs"
        request = self.request("run-request-one")
        admitted = api(origin, path, "POST", request, self.mutation)
        self.assertEqual(admitted[0], 201)
        run = admitted[2]["run"]
        self.assertEqual(run["durable"]["state"], "queued")
        self.assertEqual(run["revision_id"], request["revision_id"])
        self.assertEqual(run["plan_digest"], request["plan_digest"])
        limits = run["queue_profile"]
        self.assertEqual(limits["maximum_nonterminal_runs"], 64)
        self.assertEqual(limits["maximum_hot_runs"], 16)
        self.assertEqual(limits["ready"], {"count": 1024, "bytes": 4 * 1024 * 1024})
        self.assertEqual(limits["results"], {"count": 256, "bytes": 16 * 1024 * 1024})
        self.assertEqual(limits["writer"], {"count": 32, "bytes": 8 * 1024 * 1024})
        self.assertEqual(limits["live_ring_per_run"], {"count": 256, "bytes": 1024 * 1024})
        self.assertEqual(limits["subscribers"]["count"], 32)
        self.assertEqual(
            limits["subscribers"]["mailbox"], {"count": 64, "bytes": 256 * 1024}
        )
        self.assertEqual(limits["maximum_inline_invocation_bytes"], 8 * 1024)
        run_id = run["run_id"]

        stream = read_sse(
            origin, f"/api/v1/runs/{run_id}/events", self.auth, 3
        )
        self.assertEqual([item["event"] for item in stream], ["snapshot", "live", "terminal"])
        self.assertTrue(all(item["id"].startswith(f"v1.{run_id}.") for item in stream))

        terminal = self.wait_terminal(run_id)
        reconnect_replay = read_sse(
            origin,
            f"/api/v1/runs/{run_id}/events",
            {**self.auth, "Last-Event-ID": stream[0]["id"]},
            2,
        )
        self.assertEqual(
            [item["event"] for item in reconnect_replay], ["live", "terminal"]
        )
        replay = api(origin, path, "POST", request, self.mutation)
        self.assertEqual(replay[0], 200)
        self.assertFalse(replay[2]["created"])
        self.assertEqual(replay[2]["run"]["run_id"], run_id)
        conflict = api(
            origin,
            path,
            "POST",
            {**request, "captured_invocation": {"manual": False}},
            self.mutation,
        )
        self.assertEqual(conflict[0], 409)
        self.assertEqual(conflict[2]["code"], "request_identity_conflict")

        self.assertEqual(terminal["durable"]["state"], "succeeded")
        self.assertEqual(terminal["durable"]["logical_order"], 1)
        self.assertEqual(terminal["correctness"]["attempted"], 1)
        self.assertEqual(terminal["correctness"]["succeeded"], 1)
        self.assertEqual(terminal["correctness"]["output_count"], 1)
        self.assertTrue(terminal["correctness"]["complete"])
        self.assertRegex(terminal["correctness"]["digest"], r"^sha256:[0-9a-f]{64}$")

        trace = api(origin, f"/api/v1/runs/{run_id}/trace", headers=self.auth)
        self.assertEqual(trace[0], 200)
        evidence = trace[2]
        self.assertTrue(evidence["integrity_verified"])
        self.assertEqual(evidence["terminal_state"], "succeeded")
        self.assertEqual(len(evidence["activations"]), 1)
        activation = evidence["activations"][0]
        self.assertEqual(activation["outcome"], "success")
        self.assertEqual(activation["input"], {"manual": True})
        self.assertEqual(activation["output"], {"manual": True})
        self.assertEqual(activation["logical_order"], 1)
        self.assertEqual(evidence["run"]["revision_id"], request["revision_id"])
        self.assertEqual(evidence["run"]["plan_digest"], request["plan_digest"])
        self.assertEqual([item["sequence"] for item in evidence["checkpoints"]], [1, 2])
        self.assertEqual(
            [item["event_type"] for item in evidence["events"]],
            ["run_admitted", "activation_outcome", "checkpoint_committed"],
        )

        snapshot = read_sse(
            origin, f"/api/v1/runs/{run_id}/events", self.auth, 1
        )[0]
        self.assertEqual(snapshot["event"], "snapshot")
        self.assertTrue(snapshot["id"].startswith(f"v1.{run_id}."))
        gap_resync = read_sse(
            origin,
            f"/api/v1/runs/{run_id}/events",
            {**self.auth, "Last-Event-ID": "v1.unknown.999.old.999"},
            2,
        )
        self.assertEqual([item["event"] for item in gap_resync], ["gap", "resync"])
        self.assertTrue(json.loads(gap_resync[1]["data"])["run"]["durable"]["terminal"])

        # A stale exact target creates no Run; the same request identity remains usable when corrected.
        exact_target = self.request("run-request-exact-target")
        stale_target = api(
            origin,
            path,
            "POST",
            {**exact_target, "publication_event_id": "event-that-is-not-current"},
            self.mutation,
        )
        self.assertEqual(stale_target[0], 409)
        self.assertEqual(stale_target[2]["code"], "stale_publication_target")
        corrected = api(origin, path, "POST", exact_target, self.mutation)
        self.assertEqual(corrected[0], 201)
        same_output = self.wait_terminal(corrected[2]["run"]["run_id"])
        self.assertEqual(same_output["correctness"]["digest"], terminal["correctness"]["digest"])

        # Exact retries remain idempotent even after a newer publication becomes current.
        newer_publication = self.publish_second_revision()
        retry_after_publish = api(origin, path, "POST", request, self.mutation)
        self.assertEqual(retry_after_publish[0], 200)
        self.assertEqual(retry_after_publish[2]["run"]["run_id"], run_id)
        old_target_new_identity = api(
            origin,
            path,
            "POST",
            {**request, "run_request_id": "run-request-old-target-after-publish"},
            self.mutation,
        )
        self.assertEqual(old_target_new_identity[0], 409)
        self.assertEqual(old_target_new_identity[2]["code"], "stale_publication_target")
        self.publication = newer_publication

        oversized = api(
            origin,
            path,
            "POST",
            self.request("run-request-oversized", {"value": "x" * 9000}),
            self.mutation,
        )
        self.assertEqual(oversized[0], 413)
        self.assertEqual(oversized[2]["code"], "run_value_too_large")
        sensitive = api(
            origin,
            path,
            "POST",
            self.request("run-request-sensitive", {"password": "must-not-be-retained"}),
            self.mutation,
        )
        self.assertEqual(sensitive[0], 422)
        self.assertEqual(sensitive[2]["field"], "captured_invocation_sensitive_field")

        cancel_request = self.request("run-request-cancel", {"manual": "cancel"})
        cancel_admission = api(origin, path, "POST", cancel_request, self.mutation)
        self.assertEqual(cancel_admission[0], 201)
        cancel_run_id = cancel_admission[2]["run"]["run_id"]
        cancelled = api(
            origin,
            f"/api/v1/runs/{cancel_run_id}/cancel",
            "POST",
            {"cancellation_request_id": "cancel-request-one"},
            self.mutation,
        )
        self.assertEqual(cancelled[0], 200)
        cancelled_terminal = self.wait_terminal(cancel_run_id)
        self.assertEqual(cancelled_terminal["durable"]["state"], "cancelled")
        self.assertFalse(cancelled_terminal["correctness"]["complete"])
        repeat_cancel = api(
            origin,
            f"/api/v1/runs/{cancel_run_id}/cancel",
            "POST",
            {"cancellation_request_id": "cancel-request-one"},
            self.mutation,
        )
        self.assertEqual(repeat_cancel[0], 200)
        self.assertTrue(repeat_cancel[2]["already_terminal"])

        before_restart_status = terminal
        before_restart_trace = evidence
        self.restart()
        after_restart_status = api(
            self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth
        )[2]
        after_restart_trace = api(
            self.daemon.origin, f"/api/v1/runs/{run_id}/trace", headers=self.auth
        )[2]
        before_restart_status.pop("live", None)
        after_restart_status.pop("live", None)
        self.assertEqual(after_restart_status, before_restart_status)
        self.assertEqual(after_restart_trace, before_restart_trace)
        restarted_stream = read_sse(
            self.daemon.origin,
            f"/api/v1/runs/{run_id}/events",
            {**self.auth, "Last-Event-ID": stream[-1]["id"]},
            2,
        )
        self.assertEqual(
            [item["event"] for item in restarted_stream], ["gap", "resync"]
        )

        capabilities = set(api(self.daemon.origin, "/api/v1/capabilities")[2]["capabilities"])
        self.assertTrue(
            {
                "durable-run-admission",
                "deterministic-manual-trigger",
                "checkpointed-causal-trace",
                "reconnectable-run-sse",
                "cooperative-run-cancellation",
            }.issubset(capabilities)
        )


if __name__ == "__main__":
    unittest.main()
