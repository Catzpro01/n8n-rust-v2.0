# SPDX-License-Identifier: AGPL-3.0-or-later
"""Public-seam acceptance for bounded Generate Items and encrypted Artifacts."""

from __future__ import annotations

import json
import os
from pathlib import Path
import tempfile
import time
import unittest
from urllib.error import HTTPError
from urllib.request import Request, urlopen

from tests.acceptance.test_run_manual_trigger import BIN, Daemon, api, command, login


def raw_api(origin: str, path: str, data: bytes, headers: dict[str, str]):
    request = Request(origin + path, data=data, method="POST", headers=headers)
    try:
        with urlopen(request, timeout=30) as response:
            return response.status, {key.lower(): value for key, value in response.headers.items()}, json.loads(response.read())
    except HTTPError as error:
        return error.code, {key.lower(): value for key, value in error.headers.items()}, json.loads(error.read() or b"{}")


def read_rss_bytes(pid: int) -> int:
    for line in Path(f"/proc/{pid}/status").read_text().splitlines():
        if line.startswith("VmRSS:"):
            return int(line.split()[1]) * 1024
    raise AssertionError("VmRSS is unavailable")


class GenerateItemsArtifactAcceptance(unittest.TestCase):
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

    def publish_generate(self):
        origin = self.daemon.origin
        workflow_id = "wf-eco-generate"
        catalog = api(origin, "/api/v1/catalog", headers=self.auth)[2]
        locks = {item["contract_lock"]["name"]: item["contract_lock"] for item in catalog["nodes"]}
        self.assertEqual(set(locks), {"manual-trigger", "generate-items"})
        self.assertEqual(api(origin, "/api/v1/workflows", "POST", {
            "workflow_id": workflow_id,
            "name": "Eco Generate 49,998",
            "annotation": "bounded Envelope acceptance",
            "settings": {},
            "compatibility_metadata": {},
        }, self.mutation)[0], 201)
        lease = api(origin, f"/api/v1/workflows/{workflow_id}/editing/open", "POST", {
            "editor_session_id": "generate-tab", "label": "Generate tab"
        }, self.mutation)[2]
        generation = lease["lease_generation"]
        version = 0
        operations = [
            {
                "kind": "add_node",
                "node_instance": {
                    "id": "manual-trigger", "name": "Manual Trigger",
                    "contract_lock": locks["manual-trigger"],
                    "configuration": {"capture_mode": "manual"},
                    "layout": {"x": 100, "y": 120}, "annotation": "", "compatibility_metadata": {},
                },
            },
            {
                "kind": "add_node",
                "node_instance": {
                    "id": "generate-items", "name": "Generate Items",
                    "contract_lock": locks["generate-items"],
                    "configuration": {"count": 49_998, "start": 0, "step": 1, "data": None, "storage_mode": "artifact"},
                    "layout": {"x": 420, "y": 120}, "annotation": "", "compatibility_metadata": {},
                },
            },
            {
                "kind": "connect",
                "connection": {
                    "id": "manual-to-generate",
                    "source": {"node_id": "manual-trigger", "port_id": "invocation"},
                    "target": {"node_id": "generate-items", "port_id": "input"},
                },
            },
        ]
        for index, operation in enumerate(operations):
            response = api(origin, f"/api/v1/workflows/{workflow_id}/draft-commands", "POST", command(
                "generate-tab", generation, f"generate-command-{index}", version, operation
            ), self.mutation)
            self.assertEqual(response[0], 200, response[2])
            version = response[2]["draft_version"]
        preview_request = {"editor_session_id": "generate-tab", "lease_generation": generation, "draft_version": version}
        preview = api(origin, f"/api/v1/workflows/{workflow_id}/compile-preview", "POST", preview_request, self.mutation)
        self.assertEqual(preview[0], 200, preview[2])
        self.assertTrue(preview[2]["can_publish"])
        warnings = [item["fingerprint"] for item in preview[2]["diagnostics"] if item["requires_ack"]]
        published = api(origin, f"/api/v1/workflows/{workflow_id}/publish", "POST", {
            "publication_id": "publish-eco-generate",
            **preview_request,
            "compile_input_digest": preview[2]["compile_input_digest"],
            "acknowledged_diagnostics": warnings,
        }, self.mutation)
        self.assertEqual(published[0], 201, published[2])
        return api(origin, f"/api/v1/workflows/{workflow_id}/publication", headers=self.auth)[2]

    def wait_terminal_with_rss(self, run_id: str):
        baseline = read_rss_bytes(self.daemon.process.pid)
        maximum = baseline
        observed_progress = False
        for _ in range(6_000):
            maximum = max(maximum, read_rss_bytes(self.daemon.process.pid))
            response = api(self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth)
            self.assertEqual(response[0], 200)
            run = response[2]
            if run.get("generation", {}).get("generated_count", 0) > 0:
                observed_progress = True
            if run["durable"]["terminal"]:
                return run, baseline, maximum, observed_progress
            time.sleep(0.01)
        self.fail("Generate Items Run did not become terminal")

    def test_exact_eco_stream_artifact_api_dedup_denial_and_memory_bound(self):
        publication = self.publish_generate()
        current = publication["current_published"]
        event = publication["current_event"]["envelope"]
        request = {
            "run_request_id": "run-eco-49998",
            "publication_event_id": event["event_id"],
            "revision_id": current["revision_id"],
            "plan_digest": current["plan_digest"],
            "captured_invocation": {"manual": True, "fixture": "eco"},
        }
        admitted = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/runs", "POST", request, self.mutation)
        self.assertEqual(admitted[0], 201, admitted[2])
        run_id = admitted[2]["run"]["run_id"]
        self.assertEqual(admitted[2]["run"]["queue_profile"]["envelopes"], {"count": 256, "bytes": 4 * 1024 * 1024})
        terminal, baseline, maximum, observed_progress = self.wait_terminal_with_rss(run_id)
        self.assertEqual(terminal["durable"]["state"], "succeeded")
        self.assertEqual(terminal["durable"]["logical_order"], 2)
        self.assertEqual(terminal["correctness"]["output_count"], 49_998)
        self.assertEqual(terminal["correctness"]["succeeded"], 2)
        self.assertTrue(terminal["correctness"]["complete"])
        self.assertEqual(terminal["generation"]["generated_count"], 49_998)
        self.assertEqual(terminal["generation"]["state"], "succeeded")
        self.assertGreater(terminal["generation"]["backpressure_events"], 0)
        self.assertRegex(terminal["generation"]["stream_digest"], r"^sha256:[0-9a-f]{64}$")
        self.assertIsNotNone(terminal["generation"]["artifact"])
        self.assertTrue(observed_progress)
        self.assertLess(maximum - baseline, 64 * 1024 * 1024)
        print(
            "generate-eco=passed"
            f" items={terminal['generation']['generated_count']}"
            f" logical_bytes={terminal['generation']['logical_bytes']}"
            f" backpressure_events={terminal['generation']['backpressure_events']}"
            f" rss_baseline={baseline} rss_peak={maximum} rss_delta={maximum - baseline}"
            " queue_items=256 queue_bytes=4194304"
        )

        trace = api(self.daemon.origin, f"/api/v1/runs/{run_id}/trace", headers=self.auth)
        self.assertEqual(trace[0], 200)
        self.assertTrue(trace[2]["integrity_verified"])
        self.assertEqual([item["logical_order"] for item in trace[2]["activations"]], [1, 2])
        self.assertGreater(len(trace[2]["checkpoints"]), 2)
        generation_events = [item for item in trace[2]["events"] if item["event_type"] == "generation_checkpoint"]
        self.assertGreater(len(generation_events), 0)
        expected_first = 0
        for item in generation_events:
            payload = item["payload"]
            first, last = payload["contiguous_ordinal_range"]
            self.assertEqual(first, expected_first)
            self.assertEqual(last + 1, payload["generated_count"])
            expected_first = payload["generated_count"]

        payload = json.dumps({"large": "z" * 70_000}, separators=(",", ":")).encode()
        upload_headers = {**self.mutation, "Content-Type": "application/json", "Content-Length": str(len(payload))}
        first = raw_api(self.daemon.origin, "/api/v1/artifacts", payload, upload_headers)
        second = raw_api(self.daemon.origin, "/api/v1/artifacts", payload, upload_headers)
        self.assertEqual(first[0], 201, first[2])
        self.assertEqual(second[0], 201, second[2])
        self.assertEqual(first[2]["reference"]["artifact_id"], second[2]["reference"]["artifact_id"])
        self.assertFalse(first[2]["deduplicated"])
        self.assertTrue(second[2]["deduplicated"])
        artifact_id = first[2]["reference"]["artifact_id"]
        metadata = api(self.daemon.origin, f"/api/v1/artifacts/{artifact_id}", headers=self.auth)
        self.assertEqual(metadata[0], 200)
        self.assertNotIn("path", json.dumps(metadata[2]).lower())
        preview_request = Request(self.daemon.origin + f"/api/v1/artifacts/{artifact_id}/preview?bytes=65536", headers=self.auth)
        with urlopen(preview_request, timeout=10) as response:
            preview = response.read()
            self.assertEqual(response.headers["x-canopy-artifact-integrity"], "verified")
        self.assertEqual(len(preview), 65_536)
        range_request = Request(self.daemon.origin + f"/api/v1/artifacts/{artifact_id}/content", headers={**self.auth, "Range": "bytes=10-109"})
        with urlopen(range_request, timeout=10) as response:
            self.assertEqual(response.status, 206)
            self.assertEqual(len(response.read()), 100)
            self.assertEqual(response.headers["Content-Range"], f"bytes 10-109/{len(payload)}")
        content_request = Request(
            self.daemon.origin + f"/api/v1/artifacts/{artifact_id}/content",
            headers=self.auth,
        )
        with urlopen(content_request, timeout=10) as response:
            self.assertEqual(response.status, 200)
            self.assertEqual(response.headers["x-canopy-artifact-integrity"], "verified")
            self.assertEqual(response.read(), payload)
        denied_known = api(self.daemon.origin, f"/api/v1/artifacts/{artifact_id}")
        denied_unknown = api(self.daemon.origin, "/api/v1/artifacts/artifact-does-not-exist")
        self.assertEqual((denied_known[0], denied_known[2]), (403, denied_unknown[2]))
        capabilities = set(api(self.daemon.origin, "/api/v1/capabilities")[2]["capabilities"])
        self.assertTrue({
            "bounded-generate-items", "progressive-generate-checkpoints",
            "encrypted-owner-artifacts", "authorized-artifact-content",
        }.issubset(capabilities))

        cancel = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/runs", "POST", {**request, "run_request_id": "run-eco-cancel"}, self.mutation)
        self.assertEqual(cancel[0], 201)
        cancel_id = cancel[2]["run"]["run_id"]
        cancelled = api(self.daemon.origin, f"/api/v1/runs/{cancel_id}/cancel", "POST", {"cancellation_request_id": "cancel-eco-now"}, self.mutation)
        self.assertEqual(cancelled[0], 200)
        for _ in range(500):
            state = api(self.daemon.origin, f"/api/v1/runs/{cancel_id}", headers=self.auth)[2]
            if state["durable"]["terminal"]:
                break
            time.sleep(0.01)
        self.assertEqual(state["durable"]["state"], "cancelled")
        self.assertFalse(state["correctness"]["complete"])

        draft = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate", headers=self.auth)[2]
        editing = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/editing/open", "POST", {"editor_session_id": "generate-tab", "label": "Generate tab"}, self.mutation)[2]
        sensitive = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/draft-commands", "POST", command(
            "generate-tab", editing["lease_generation"], "reject-secret-artifact-data", draft["draft_version"],
            {"kind": "configure_node", "node_instance_id": "generate-items", "configuration": {"count": 1, "start": 0, "step": 1, "data": {"password": "never-store-me"}, "storage_mode": "auto"}},
        ), self.mutation)
        self.assertEqual(sensitive[0], 422)
        self.assertNotIn("never-store-me", json.dumps(sensitive[2]))
        denied_configuration = {"count": 1, "start": 0, "step": 1, "data": {"$artifact": "artifact-reference-not-retained"}, "storage_mode": "artifact"}
        configured = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/draft-commands", "POST", command(
            "generate-tab", editing["lease_generation"], "configure-denied-artifact", draft["draft_version"],
            {"kind": "configure_node", "node_instance_id": "generate-items", "configuration": denied_configuration},
        ), self.mutation)
        self.assertEqual(configured[0], 200)
        preview_request = {"editor_session_id": "generate-tab", "lease_generation": editing["lease_generation"], "draft_version": configured[2]["draft_version"]}
        preview = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/compile-preview", "POST", preview_request, self.mutation)[2]
        denied_publication = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/publish", "POST", {
            "publication_id": "publish-denied-artifact", **preview_request,
            "compile_input_digest": preview["compile_input_digest"],
            "acknowledged_diagnostics": [item["fingerprint"] for item in preview["diagnostics"] if item["requires_ack"]],
        }, self.mutation)
        self.assertEqual(denied_publication[0], 201)
        denied_status = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/publication", headers=self.auth)[2]
        denied_current = denied_status["current_published"]
        denied_event = denied_status["current_event"]["envelope"]
        denied_run = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/runs", "POST", {
            "run_request_id": "run-denied-artifact", "publication_event_id": denied_event["event_id"],
            "revision_id": denied_current["revision_id"], "plan_digest": denied_current["plan_digest"],
            "captured_invocation": {"manual": True},
        }, self.mutation)
        self.assertEqual(denied_run[0], 201)
        denied_terminal, _, _, _ = self.wait_terminal_with_rss(denied_run[2]["run"]["run_id"])
        self.assertEqual(denied_terminal["durable"]["state"], "failed")
        self.assertFalse(denied_terminal["correctness"]["complete"])

    def test_disk_pressure_is_durably_suspended_then_revalidated_on_restart(self):
        publication = self.publish_generate()
        current = publication["current_published"]
        event = publication["current_event"]["envelope"]
        staging = self.state / "artifacts" / "staging"
        staging.chmod(0o500)
        try:
            admitted = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/runs", "POST", {
                "run_request_id": "run-eco-disk-pressure", "publication_event_id": event["event_id"],
                "revision_id": current["revision_id"], "plan_digest": current["plan_digest"],
                "captured_invocation": {"manual": True, "fixture": "disk-pressure"},
            }, self.mutation)
            self.assertEqual(admitted[0], 201)
            run_id = admitted[2]["run"]["run_id"]
            for _ in range(1_000):
                suspended = api(self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth)[2]
                if suspended["durable"]["state"] == "suspended":
                    break
                time.sleep(0.01)
            self.assertEqual(suspended["durable"]["state"], "suspended")
            self.assertFalse(suspended["durable"]["terminal"])
        finally:
            staging.chmod(0o700)
        self.daemon.stop()
        self.daemon = Daemon(self.state, self.key)
        self.auth, self.mutation = login(self.daemon.origin)
        terminal, _, _, _ = self.wait_terminal_with_rss(run_id)
        self.assertEqual(terminal["durable"]["state"], "succeeded")
        self.assertEqual(terminal["generation"]["generated_count"], 49_998)
        print("generate-suspension=passed durable_state=suspended revalidated_state=succeeded")

    def test_restart_regenerates_only_the_uncommitted_suffix(self):
        publication = self.publish_generate()
        current = publication["current_published"]
        event = publication["current_event"]["envelope"]
        request = {
            "run_request_id": "run-eco-restart-suffix",
            "publication_event_id": event["event_id"],
            "revision_id": current["revision_id"],
            "plan_digest": current["plan_digest"],
            "captured_invocation": {"manual": True, "fixture": "restart"},
        }
        admitted = api(self.daemon.origin, "/api/v1/workflows/wf-eco-generate/runs", "POST", request, self.mutation)
        self.assertEqual(admitted[0], 201)
        run_id = admitted[2]["run"]["run_id"]
        durable_before = None
        for _ in range(4_000):
            current_run = api(self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth)[2]
            progress = current_run.get("generation")
            if progress and progress["generated_count"] >= 1_024 and not current_run["durable"]["terminal"]:
                durable_before = progress["generated_count"]
                break
            time.sleep(0.005)
        self.assertIsNotNone(durable_before, "no progressive checkpoint was observable before terminal completion")
        self.daemon.stop()
        self.daemon = Daemon(self.state, self.key)
        self.auth, self.mutation = login(self.daemon.origin)
        resumed_first = api(self.daemon.origin, f"/api/v1/runs/{run_id}", headers=self.auth)[2]
        if resumed_first.get("generation"):
            self.assertGreaterEqual(resumed_first["generation"]["generated_count"], durable_before)
        terminal, _, _, _ = self.wait_terminal_with_rss(run_id)
        self.assertEqual(terminal["durable"]["state"], "succeeded")
        self.assertEqual(terminal["generation"]["generated_count"], 49_998)
        trace = api(self.daemon.origin, f"/api/v1/runs/{run_id}/trace", headers=self.auth)[2]
        self.assertTrue(trace["integrity_verified"])
        self.assertEqual(len(trace["activations"]), 2)
        self.assertEqual(len({(item["logical_order"], item["attempt"]) for item in trace["activations"]}), 2)
        sequences = [item["sequence"] for item in trace["checkpoints"]]
        self.assertEqual(sequences, sorted(set(sequences)))
        print(
            f"generate-restart=passed durable_cursor_before={durable_before}"
            f" terminal_count={terminal['generation']['generated_count']}"
            f" activations={len(trace['activations'])} unique_checkpoints={len(sequences)}"
        )


if __name__ == "__main__":
    unittest.main()
