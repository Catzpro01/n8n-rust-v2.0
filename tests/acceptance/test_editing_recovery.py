# SPDX-License-Identifier: AGPL-3.0-or-later
"""External two-session acceptance for leases, durable history, and recovery forks."""
import json
import os
import socket
import subprocess
import tempfile
import time
import unittest
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import Request, urlopen
from urllib.error import HTTPError, URLError

REPO = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get("WORKFLOWD_BIN", REPO / "target/debug/workflowd"))


def free_port():
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def api(origin, path, method="GET", body=None, headers=None):
    req = Request(
        origin + path,
        data=None if body is None else json.dumps(body).encode(),
        method=method,
        headers={"Content-Type": "application/json", **(headers or {})},
    )
    try:
        with urlopen(req, timeout=10) as r:
            return (
                r.status,
                {k.lower(): v for k, v in r.headers.items()},
                json.loads(r.read() or b"{}"),
            )
    except HTTPError as e:
        return (
            e.code,
            {k.lower(): v for k, v in e.headers.items()},
            json.loads(e.read() or b"{}"),
        )


class Daemon:
    def __init__(self, state, key):
        self.port = free_port()
        self.origin = f"http://127.0.0.1:{self.port}"
        env = os.environ.copy()
        env.update(
            {
                "WORKFLOWD_BIND": f"127.0.0.1:{self.port}",
                "WORKFLOWD_CONTROL_ORIGIN": self.origin,
                "WORKFLOWD_STATE_DIR": str(state),
                "WORKFLOWD_MASTER_KEY_FILE": str(key),
                "WORKFLOWD_ARGON_MEMORY_KIB": "8192",
                "WORKFLOWD_ARGON_ITERATIONS": "1",
                "WORKFLOWD_DRAFT_LEASE_TTL_SECONDS": "3",
                "WORKFLOWD_DRAFT_TAKEOVER_GRACE_SECONDS": "1",
                "WORKFLOWD_DRAFT_SNAPSHOT_INTERVAL": "2",
                "WORKFLOWD_DRAFT_HISTORY_LIMIT": "4",
                "WORKFLOWD_DRAFT_UNDO_LIMIT": "3",
            }
        )
        self.p = subprocess.Popen(
            [str(BIN), "serve"],
            cwd=REPO,
            env=env,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        for _ in range(400):
            try:
                if api(self.origin, "/health/live")[0] == 200:
                    return
            except (URLError, ConnectionError):
                pass
            time.sleep(0.05)
        if self.p.poll() is None:
            self.p.terminate()
            self.p.wait(8)
        raise AssertionError("not started")

    def stop(self):
        if self.p.poll() is None:
            self.p.terminate()
            self.p.wait(8)


def login(origin, email="owner@example.test"):
    oh = {"Origin": origin}
    status, rh, body = api(
        origin,
        "/api/v1/session/login",
        "POST",
        {"email": email, "password": "correct horse battery staple"},
        oh,
    )
    assert status == 200
    auth = {"Cookie": rh["set-cookie"].split(";", 1)[0]}
    return auth, {**auth, **oh, "X-Canopy-CSRF": body["csrf_token"]}


def command(session, generation, command_id, base, operation):
    return {
        "editor_session_id": session,
        "lease_generation": generation,
        "command_id": command_id,
        "base_draft_version": base,
        "operation": operation,
    }


class EditingRecoveryAcceptance(unittest.TestCase):
    def test_two_sessions_takeover_history_and_recovery_are_loss_aware(self):
        with tempfile.TemporaryDirectory() as td:
            root = Path(td)
            state = root / "state"
            key = root / "key"
            key.write_bytes(os.urandom(32))
            d = Daemon(state, key)
            self.addCleanup(d.stop)
            origin = d.origin
            oh = {"Origin": origin}
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/setup",
                    "POST",
                    {
                        "email": "owner@example.test",
                        "password": "correct horse battery staple",
                        "recovery_passphrase": "separate recovery phrase long",
                    },
                    oh,
                )[0],
                201,
            )
            auth_a, mut_a = login(origin)
            auth_b, mut_b = login(origin)
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows",
                    "POST",
                    {
                        "workflow_id": "wf-arbitrated",
                        "name": "Arbitrated Draft",
                        "annotation": "initial",
                        "settings": {},
                        "compatibility_metadata": {},
                    },
                    mut_a,
                )[0],
                201,
            )
            open_a = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/open",
                "POST",
                {"editor_session_id": "tab-a", "label": "Primary tab"},
                mut_a,
            )
            self.assertEqual(open_a[0], 200)
            self.assertEqual(open_a[2]["role"], "holder")
            generation_a = open_a[2]["lease_generation"]
            open_b = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/open",
                "POST",
                {"editor_session_id": "tab-b", "label": "Second tab"},
                mut_b,
            )
            self.assertEqual(open_b[2]["role"], "read_only")
            self.assertEqual(open_b[2]["holder"]["label"], "Primary tab")
            self.assertNotIn("editor_session_id", open_b[2]["holder"])
            rejected = command(
                "tab-b",
                generation_a,
                "readonly-write",
                0,
                {"kind": "set_workflow_annotation", "annotation": "must not land"},
            )
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    rejected,
                    mut_b,
                )[0],
                423,
            )
            self.assertEqual(
                api(origin, "/api/v1/workflows/wf-arbitrated", headers=auth_a)[2][
                    "annotation"
                ],
                "initial",
            )
            old_expiry = open_a[2]["holder"]["expires_at"]
            heart = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/heartbeat",
                "POST",
                {"editor_session_id": "tab-a", "lease_generation": generation_a},
                mut_a,
            )[2]
            self.assertGreaterEqual(heart["holder"]["expires_at"], old_expiry)
            requested = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/takeover/request",
                "POST",
                {"editor_session_id": "tab-b", "request_id": "take-approved"},
                mut_b,
            )[2]
            self.assertEqual(requested["takeover"]["state"], "pending")
            holder_view = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/status?"
                + urlencode({"editor_session_id": "tab-a"}),
                headers=auth_a,
            )[2]
            self.assertEqual(holder_view["takeover"]["requester_label"], "Second tab")
            approved = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/takeover/respond",
                "POST",
                {
                    "editor_session_id": "tab-a",
                    "lease_generation": generation_a,
                    "request_id": "take-approved",
                    "approve": True,
                },
                mut_a,
            )[2]
            self.assertEqual(approved["role"], "read_only")
            generation_b = approved["lease_generation"]
            status_b = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/status?"
                + urlencode({"editor_session_id": "tab-b"}),
                headers=auth_b,
            )[2]
            self.assertEqual(status_b["role"], "holder")
            first = command(
                "tab-b",
                generation_b,
                "b-first",
                0,
                {"kind": "set_workflow_annotation", "annotation": "online-b"},
            )
            accepted = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/draft-commands",
                "POST",
                first,
                mut_b,
            )[2]
            self.assertEqual(accepted["draft_version"], 1)
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    first,
                    mut_b,
                )[2],
                accepted,
            )
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/editing/release",
                    "POST",
                    {"editor_session_id": "tab-b", "lease_generation": generation_b},
                    mut_b,
                )[2]["role"],
                "available",
            )
            acquired_a = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/acquire",
                "POST",
                {"editor_session_id": "tab-a", "label": "Primary tab"},
                mut_a,
            )[2]
            self.assertEqual(acquired_a["role"], "holder")
            generation_a = acquired_a["lease_generation"]
            pending = command(
                "tab-a",
                generation_a,
                "offline-a",
                1,
                {
                    "kind": "set_workflow_annotation",
                    "annotation": "offline-a-recovered",
                },
            )
            grace = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/takeover/request",
                "POST",
                {"editor_session_id": "tab-b", "request_id": "take-grace"},
                mut_b,
            )[2]
            self.assertEqual(grace["role"], "read_only")
            time.sleep(1.1)
            claimed_b = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/takeover/claim",
                "POST",
                {"editor_session_id": "tab-b", "request_id": "take-grace"},
                mut_b,
            )[2]
            self.assertEqual(claimed_b["role"], "holder")
            generation_b = claimed_b["lease_generation"]
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    pending,
                    mut_a,
                )[0],
                423,
            )
            second = command(
                "tab-b",
                generation_b,
                "b-second",
                1,
                {
                    "kind": "set_workflow_annotation",
                    "annotation": "new-authority-state",
                },
            )
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    second,
                    mut_b,
                )[2]["draft_version"],
                2,
            )
            recovery = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/recovery/reconcile",
                "POST",
                {
                    "recovery_copy_id": "copy-offline-a",
                    "editor_session_id": "tab-a",
                    "command": pending,
                },
                mut_a,
            )
            self.assertEqual(recovery[0], 409)
            fork = recovery[2]["fork"]
            self.assertEqual(recovery[2]["status"], "conflict_fork")
            self.assertEqual(fork["diff"]["base_draft_version"], 1)
            self.assertEqual(fork["diff"]["current_draft_version"], 2)
            self.assertTrue(fork["diff"]["authority_changed"])
            self.assertNotIn("document_json", json.dumps(fork))
            applied = api(
                origin,
                f"/api/v1/workflows/wf-arbitrated/recovery-forks/{fork['fork_id']}/apply",
                "POST",
                {
                    "editor_session_id": "tab-b",
                    "lease_generation": generation_b,
                    "command_id": "apply-recovery-a",
                    "base_draft_version": 2,
                },
                mut_b,
            )[2]
            self.assertEqual(applied["draft_version"], 3)
            self.assertEqual(
                api(origin, "/api/v1/workflows/wf-arbitrated", headers=auth_b)[2][
                    "annotation"
                ],
                "offline-a-recovered",
            )
            undone = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/draft-commands",
                "POST",
                command("tab-b", generation_b, "undo-recovery", 3, {"kind": "undo"}),
                mut_b,
            )[2]
            self.assertEqual(undone["draft_version"], 4)
            self.assertEqual(
                api(origin, "/api/v1/workflows/wf-arbitrated", headers=auth_b)[2][
                    "annotation"
                ],
                "new-authority-state",
            )
            d.stop()
            d = Daemon(state, key)
            self.addCleanup(d.stop)
            origin = d.origin
            auth_b, mut_b = login(origin)
            redone = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/draft-commands",
                "POST",
                command("tab-b", generation_b, "redo-recovery", 4, {"kind": "redo"}),
                mut_b,
            )[2]
            self.assertEqual(redone["draft_version"], 5)
            self.assertEqual(
                api(origin, "/api/v1/workflows/wf-arbitrated", headers=auth_b)[2][
                    "annotation"
                ],
                "offline-a-recovered",
            )
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/editing/release",
                    "POST",
                    {"editor_session_id": "tab-b", "lease_generation": generation_b},
                    mut_b,
                )[2]["role"],
                "available",
            )
            auth_a, mut_a = login(origin)
            old_generation_b = generation_b
            acquired_a = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/acquire",
                "POST",
                {"editor_session_id": "tab-a", "label": "Primary tab"},
                mut_a,
            )[2]
            self.assertEqual(acquired_a["role"], "holder")
            time.sleep(3.1)
            acquired_b = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/editing/acquire",
                "POST",
                {"editor_session_id": "tab-b", "label": "Second tab"},
                mut_b,
            )[2]
            self.assertEqual(acquired_b["role"], "holder")
            generation_b = acquired_b["lease_generation"]
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    command(
                        "tab-b",
                        old_generation_b,
                        "fenced-old-generation",
                        5,
                        {
                            "kind": "set_workflow_annotation",
                            "annotation": "must not land",
                        },
                    ),
                    mut_b,
                )[0],
                423,
            )
            exact = command(
                "tab-b",
                generation_b,
                "exact-replay",
                5,
                {"kind": "set_workflow_annotation", "annotation": "exact-replayed"},
            )
            replayed = api(
                origin,
                "/api/v1/workflows/wf-arbitrated/recovery/reconcile",
                "POST",
                {
                    "recovery_copy_id": "copy-exact",
                    "editor_session_id": "tab-b",
                    "command": exact,
                },
                mut_b,
            )
            self.assertEqual(replayed[0], 200)
            self.assertEqual(replayed[2]["status"], "auto_replayed")
            version = replayed[2]["accepted"]["draft_version"]
            stale = command(
                "tab-b",
                generation_b,
                "stale-after-replay",
                5,
                {"kind": "set_workflow_annotation", "annotation": "stale"},
            )
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    stale,
                    mut_b,
                )[0],
                409,
            )
            self.assertEqual(
                api(origin, "/api/v1/workflows/wf-arbitrated", headers=auth_b)[2][
                    "annotation"
                ],
                "exact-replayed",
            )
            for index in range(7):
                body = command(
                    "tab-b",
                    generation_b,
                    f"compact-{index}",
                    version,
                    {
                        "kind": "set_workflow_annotation",
                        "annotation": f"compact-{index}",
                    },
                )
                version = api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    body,
                    mut_b,
                )[2]["draft_version"]
            history = api(
                origin, "/api/v1/workflows/wf-arbitrated/history", headers=auth_b
            )[2]
            self.assertLessEqual(history["retained_event_count"], 4)
            self.assertLessEqual(history["undo_depth"], 3)
            self.assertGreaterEqual(history["snapshot_count"], 2)
            self.assertLessEqual(history["snapshot_count"], history["snapshot_limit"])
            self.assertEqual(history["draft_version"], version)
            for index in range(3):
                version = api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    command(
                        "tab-b",
                        generation_b,
                        f"bounded-undo-{index}",
                        version,
                        {"kind": "undo"},
                    ),
                    mut_b,
                )[2]["draft_version"]
            self.assertEqual(
                api(
                    origin,
                    "/api/v1/workflows/wf-arbitrated/draft-commands",
                    "POST",
                    command(
                        "tab-b",
                        generation_b,
                        "beyond-undo-floor",
                        version,
                        {"kind": "undo"},
                    ),
                    mut_b,
                )[0],
                409,
            )


if __name__ == "__main__":
    unittest.main()
