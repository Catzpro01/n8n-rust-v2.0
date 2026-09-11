# SPDX-License-Identifier: AGPL-3.0-or-later
"""Public-seam acceptance for deterministic signed publication and rollback."""

from __future__ import annotations

import base64
import hashlib
import json
import math
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen

REPO = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get("WORKFLOWD_BIN", REPO / "target/debug/workflowd"))
SIGNING_DOMAIN = b"canopy-publication-event-v1\0"


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
            stderr=subprocess.DEVNULL,
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
        raise AssertionError("daemon did not start")

    def stop(self) -> None:
        if self.process.poll() is None:
            self.process.terminate()
            self.process.wait(8)


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


def b64url(value: str) -> bytes:
    return base64.urlsafe_b64decode(value + "=" * (-len(value) % 4))


def jcs(value) -> bytes:
    """Sufficient independent RFC 8785 encoder for this acceptance fixture."""

    def encode(item):
        if item is None:
            return "null"
        if item is True:
            return "true"
        if item is False:
            return "false"
        if isinstance(item, str):
            return json.dumps(item, ensure_ascii=False, separators=(",", ":"))
        if isinstance(item, int):
            if abs(item) > 9_007_199_254_740_991:
                raise AssertionError("fixture integer is outside I-JSON exact range")
            return str(item)
        if isinstance(item, float):
            if not math.isfinite(item):
                raise AssertionError("non-finite number is not JCS")
            if item == 0:
                return "0"
            if item.is_integer() and abs(item) < 1e21:
                return str(int(item))
            raise AssertionError(f"fixture needs a fuller independent float encoder: {item}")
        if isinstance(item, list):
            return "[" + ",".join(encode(element) for element in item) + "]"
        if isinstance(item, dict):
            keys = sorted(item, key=lambda key: key.encode("utf-16-be"))
            return "{" + ",".join(f"{encode(key)}:{encode(item[key])}" for key in keys) + "}"
        raise AssertionError(f"unsupported JCS fixture type: {type(item)}")

    return encode(value).encode()


def digest(value) -> str:
    return "sha256:" + hashlib.sha256(jcs(value)).hexdigest()


P = 2**255 - 19
Q = 2**252 + 27742317777372353535851937790883648493
D = (-121665 * pow(121666, P - 2, P)) % P
I = pow(2, (P - 1) // 4, P)


def recover_x(y: int, sign: int) -> int:
    xx = (y * y - 1) * pow(D * y * y + 1, P - 2, P) % P
    x = pow(xx, (P + 3) // 8, P)
    if (x * x - xx) % P:
        x = x * I % P
    if (x * x - xx) % P:
        raise AssertionError("invalid Ed25519 point")
    if x & 1 != sign:
        x = P - x
    return x


def decode_point(encoded: bytes):
    assert len(encoded) == 32
    number = int.from_bytes(encoded, "little")
    y = number & ((1 << 255) - 1)
    assert y < P
    point = (recover_x(y, number >> 255), y)
    x, y = point
    assert (-x * x + y * y - 1 - D * x * x * y * y) % P == 0
    return point


def point_add(left, right):
    x1, y1 = left
    x2, y2 = right
    product = D * x1 * x2 * y1 * y2 % P
    x3 = (x1 * y2 + y1 * x2) * pow(1 + product, P - 2, P) % P
    y3 = (y1 * y2 + x1 * x2) * pow(1 - product, P - 2, P) % P
    return x3, y3


def scalar_multiply(point, scalar: int):
    result = (0, 1)
    addend = point
    while scalar:
        if scalar & 1:
            result = point_add(result, addend)
        addend = point_add(addend, addend)
        scalar >>= 1
    return result


def verify_published_record(record) -> None:
    assert record["revision"]["digest"] == digest(record["revision"]["payload"])
    assert record["plan"]["digest"] == digest(record["plan"]["payload"])
    evidence = record["evidence"]
    evidence_identity = {
        "compile_input_digest": evidence["compile_input_digest"],
        "compiler_result_digest": evidence["compiler_result_digest"],
        "diagnostic_fingerprints": evidence["diagnostic_fingerprints"],
        "acknowledged_diagnostics": evidence["acknowledged_diagnostics"],
        "compiler_abi": evidence["compiler_abi"],
        "plan_format": evidence["plan_format"],
        "canonicalization": evidence["canonicalization"],
        "digest_algorithm": evidence["digest_algorithm"],
    }
    assert evidence["digest"] == digest(evidence_identity)
    envelope = record["event"]["envelope"]
    assert envelope["target_revision_id"] == record["revision"]["revision_id"]
    assert envelope["revision_digest"] == record["revision"]["digest"]
    assert envelope["plan_digest"] == record["plan"]["digest"]
    assert envelope["evidence_digest"] == evidence["digest"]
    assert envelope["compile_input_digest"] == evidence["compile_input_digest"]
    assert record["plan"]["payload"]["revision_digest"] == record["revision"]["digest"]
    verify_signed_event(record["event"])


def verify_signed_event(event) -> None:
    signature = event["signature"]
    assert signature["algorithm"] == "ed25519-rfc8032"
    assert signature["canonicalization"] == "jcs-rfc8785"
    public_key = b64url(signature["public_key"])
    signed = b64url(signature["value"])
    assert len(public_key) == 32 and len(signed) == 64
    assert signature["key_id"].startswith("ed25519:")
    assert signature["key_id"] == "ed25519:" + hashlib.sha256(public_key).hexdigest()
    scalar = int.from_bytes(signed[32:], "little")
    assert scalar < Q
    base_y = 4 * pow(5, P - 2, P) % P
    base = (recover_x(base_y, 0), base_y)
    public_point = decode_point(public_key)
    r_point = decode_point(signed[:32])
    message = SIGNING_DOMAIN + jcs(event["envelope"])
    challenge = int.from_bytes(
        hashlib.sha512(signed[:32] + public_key + message).digest(), "little"
    ) % Q
    assert scalar_multiply(base, scalar) == point_add(
        r_point, scalar_multiply(public_point, challenge)
    )


class PublicationRollbackAcceptance(unittest.TestCase):
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

    def restart(self):
        self.daemon.stop()
        self.daemon = Daemon(self.state, self.key)
        return login(self.daemon.origin)

    def test_publish_sign_verify_restart_and_non_destructive_rollback(self):
        origin = self.daemon.origin
        auth_a, mutation_a = login(origin)
        auth_b, mutation_b = login(origin)
        capabilities = set(api(origin, "/api/v1/capabilities")[2]["capabilities"])
        self.assertTrue({
            "deterministic-pure-compiler",
            "signed-immutable-revisions",
            "pinned-execution-plans",
            "non-destructive-signed-rollback",
        }.issubset(capabilities))

        # Invalid but durable Drafts compile to structured errors and cannot publish.
        self.assertEqual(
            api(
                origin,
                "/api/v1/workflows",
                "POST",
                {
                    "workflow_id": "wf-invalid-publication",
                    "name": "Invalid publication",
                    "annotation": "",
                    "settings": {},
                    "compatibility_metadata": {},
                },
                mutation_a,
            )[0],
            201,
        )
        invalid_lease = api(
            origin,
            "/api/v1/workflows/wf-invalid-publication/editing/open",
            "POST",
            {"editor_session_id": "invalid-tab", "label": "Invalid tab"},
            mutation_a,
        )[2]
        invalid_preview_request = {
            "editor_session_id": "invalid-tab",
            "lease_generation": invalid_lease["lease_generation"],
            "draft_version": 0,
        }
        invalid_preview = api(
            origin,
            "/api/v1/workflows/wf-invalid-publication/compile-preview",
            "POST",
            invalid_preview_request,
            mutation_a,
        )
        self.assertEqual(invalid_preview[0], 200)
        self.assertFalse(invalid_preview[2]["can_publish"])
        self.assertEqual(invalid_preview[2]["diagnostics"][0]["code"], "E_GRAPH_EMPTY")
        invalid_publish = api(
            origin,
            "/api/v1/workflows/wf-invalid-publication/publish",
            "POST",
            {
                "publication_id": "publication-invalid",
                **invalid_preview_request,
                "compile_input_digest": invalid_preview[2]["compile_input_digest"],
                "acknowledged_diagnostics": [],
            },
            mutation_a,
        )
        self.assertEqual(invalid_publish[0], 422)
        self.assertEqual(invalid_publish[2]["code"], "compile_failed")

        catalog = api(origin, "/api/v1/catalog", headers=auth_a)[2]
        contract_lock = catalog["nodes"][0]["contract_lock"]
        self.assertEqual(
            api(
                origin,
                "/api/v1/workflows",
                "POST",
                {
                    "workflow_id": "wf-published",
                    "name": "Signed Manual Trigger",
                    "annotation": "first publication",
                    "settings": {},
                    "compatibility_metadata": {},
                },
                mutation_a,
            )[0],
            201,
        )
        lease_a = api(
            origin,
            "/api/v1/workflows/wf-published/editing/open",
            "POST",
            {"editor_session_id": "tab-a", "label": "Publisher tab"},
            mutation_a,
        )[2]
        generation = lease_a["lease_generation"]
        reader = api(
            origin,
            "/api/v1/workflows/wf-published/editing/open",
            "POST",
            {"editor_session_id": "tab-b", "label": "Reader tab"},
            mutation_b,
        )[2]
        self.assertEqual(reader["role"], "read_only")
        added = api(
            origin,
            "/api/v1/workflows/wf-published/draft-commands",
            "POST",
            command(
                "tab-a",
                generation,
                "add-manual-trigger",
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
            mutation_a,
        )
        self.assertEqual(added[2]["draft_version"], 1)

        preview_request = {
            "editor_session_id": "tab-a",
            "lease_generation": generation,
            "draft_version": 1,
        }
        first_preview = api(
            origin,
            "/api/v1/workflows/wf-published/compile-preview",
            "POST",
            preview_request,
            mutation_a,
        )
        second_preview = api(
            origin,
            "/api/v1/workflows/wf-published/compile-preview",
            "POST",
            preview_request,
            mutation_a,
        )
        self.assertEqual(first_preview, second_preview)
        preview = first_preview[2]
        self.assertTrue(preview["can_publish"])
        self.assertEqual(preview["canonicalization"], "jcs-rfc8785")
        self.assertEqual(preview["digest_algorithm"], "sha256")
        self.assertEqual(preview["compiler_abi"], "canopy.compiler/v1alpha1")
        self.assertEqual(preview["plan_format"], "canopy.plan+jcs/v1alpha1")
        self.assertRegex(preview["compile_input_digest"], r"^sha256:[0-9a-f]{64}$")
        warning = next(item for item in preview["diagnostics"] if item["requires_ack"])
        self.assertEqual(warning["code"], "W_OUTPUT_UNUSED")
        self.assertEqual(warning["severity"], "warning")
        self.assertRegex(warning["fingerprint"], r"^sha256:[0-9a-f]{64}$")

        publish_request = {
            "publication_id": "publication-one",
            **preview_request,
            "compile_input_digest": preview["compile_input_digest"],
            "acknowledged_diagnostics": [],
        }
        missing_ack = api(
            origin,
            "/api/v1/workflows/wf-published/publish",
            "POST",
            publish_request,
            mutation_a,
        )
        self.assertEqual(missing_ack[0], 422)
        self.assertEqual(missing_ack[2]["code"], "warning_ack_required")
        self.assertEqual(missing_ack[2]["required_acknowledgements"], [warning["fingerprint"]])

        reader_publish = api(
            origin,
            "/api/v1/workflows/wf-published/publish",
            "POST",
            {
                **publish_request,
                "publication_id": "reader-publication",
                "editor_session_id": "tab-b",
                "acknowledged_diagnostics": [warning["fingerprint"]],
            },
            mutation_b,
        )
        self.assertEqual(reader_publish[0], 423)

        # A changed Draft invalidates an earlier preview and produces no partial publication.
        changed = api(
            origin,
            "/api/v1/workflows/wf-published/draft-commands",
            "POST",
            command(
                "tab-a",
                generation,
                "change-before-publish",
                1,
                {"kind": "set_workflow_annotation", "annotation": "published one"},
            ),
            mutation_a,
        )[2]
        self.assertEqual(changed["draft_version"], 2)
        stale = api(
            origin,
            "/api/v1/workflows/wf-published/publish",
            "POST",
            {**publish_request, "acknowledged_diagnostics": [warning["fingerprint"]]},
            mutation_a,
        )
        self.assertEqual(stale[0], 409)
        self.assertEqual(stale[2]["code"], "compile_input_changed")
        empty_status = api(
            origin,
            "/api/v1/workflows/wf-published/publication",
            headers=auth_a,
        )[2]
        self.assertEqual(empty_status["revisions"], [])
        self.assertIsNone(empty_status["current_event"])

        preview_request["draft_version"] = 2
        preview_one = api(
            origin,
            "/api/v1/workflows/wf-published/compile-preview",
            "POST",
            preview_request,
            mutation_a,
        )[2]
        warning_one = next(
            item for item in preview_one["diagnostics"] if item["requires_ack"]
        )
        publication_one = api(
            origin,
            "/api/v1/workflows/wf-published/publish",
            "POST",
            {
                "publication_id": "publication-one",
                **preview_request,
                "compile_input_digest": preview_one["compile_input_digest"],
                "acknowledged_diagnostics": [warning_one["fingerprint"]],
            },
            mutation_a,
        )
        self.assertEqual(publication_one[0], 201)
        published_one = publication_one[2]
        self.assertEqual(published_one["revision"]["sequence"], 1)
        self.assertEqual(published_one["revision"]["source_draft_version"], 2)
        self.assertEqual(
            published_one["revision"]["digest"],
            digest(published_one["revision"]["payload"]),
        )
        self.assertEqual(
            published_one["plan"]["digest"], digest(published_one["plan"]["payload"])
        )
        self.assertEqual(
            published_one["evidence"]["acknowledged_diagnostics"],
            [warning_one["fingerprint"]],
        )
        verify_published_record(published_one)
        revision_one_id = published_one["revision"]["revision_id"]

        status_one = api(
            origin, "/api/v1/workflows/wf-published/publication", headers=auth_a
        )[2]
        self.assertEqual(status_one["current_published"]["revision_id"], revision_one_id)
        self.assertEqual(status_one["difference"]["state"], "matches")

        # Immutable payloads and the pinned plan remain byte-for-byte readable after restart.
        revision_path = (
            "/api/v1/workflows/wf-published/revisions/" + quote(revision_one_id, safe="")
        )
        before_restart = api(origin, revision_path, headers=auth_a)[2]
        auth_a, mutation_a = self.restart()
        origin = self.daemon.origin
        auth_b, mutation_b = login(origin)
        after_restart = api(origin, revision_path, headers=auth_a)[2]
        self.assertEqual(after_restart, before_restart)
        verify_published_record(after_restart)

        lease_a = api(
            origin,
            "/api/v1/workflows/wf-published/editing/open",
            "POST",
            {"editor_session_id": "tab-a", "label": "Publisher tab"},
            mutation_a,
        )[2]
        generation = lease_a["lease_generation"]
        changed_again = api(
            origin,
            "/api/v1/workflows/wf-published/draft-commands",
            "POST",
            command(
                "tab-a",
                generation,
                "change-for-second-publication",
                2,
                {"kind": "set_workflow_annotation", "annotation": "published two"},
            ),
            mutation_a,
        )[2]
        self.assertEqual(changed_again["draft_version"], 3)
        changed_status = api(
            origin, "/api/v1/workflows/wf-published/publication", headers=auth_a
        )[2]
        self.assertEqual(changed_status["difference"]["state"], "changed")
        self.assertIn("annotation", changed_status["difference"]["fields"])

        preview_two_request = {
            "editor_session_id": "tab-a",
            "lease_generation": generation,
            "draft_version": 3,
        }
        preview_two = api(
            origin,
            "/api/v1/workflows/wf-published/compile-preview",
            "POST",
            preview_two_request,
            mutation_a,
        )[2]
        warning_two = next(
            item for item in preview_two["diagnostics"] if item["requires_ack"]
        )
        publication_two = api(
            origin,
            "/api/v1/workflows/wf-published/publish",
            "POST",
            {
                "publication_id": "publication-two",
                **preview_two_request,
                "compile_input_digest": preview_two["compile_input_digest"],
                "acknowledged_diagnostics": [warning_two["fingerprint"]],
            },
            mutation_a,
        )
        self.assertEqual(publication_two[0], 201)
        published_two = publication_two[2]
        self.assertEqual(published_two["revision"]["sequence"], 2)
        verify_published_record(published_two)
        revision_two_id = published_two["revision"]["revision_id"]
        self.assertNotEqual(revision_two_id, revision_one_id)

        rollback_reader = api(
            origin,
            "/api/v1/workflows/wf-published/rollback",
            "POST",
            {
                "rollback_id": "rollback-reader",
                "editor_session_id": "tab-b",
                "lease_generation": generation,
                "draft_version": 3,
                "target_revision_id": revision_one_id,
            },
            mutation_b,
        )
        self.assertEqual(rollback_reader[0], 423)

        rollback = api(
            origin,
            "/api/v1/workflows/wf-published/rollback",
            "POST",
            {
                "rollback_id": "rollback-to-one",
                "editor_session_id": "tab-a",
                "lease_generation": generation,
                "draft_version": 3,
                "target_revision_id": revision_one_id,
            },
            mutation_a,
        )
        self.assertEqual(rollback[0], 200)
        self.assertEqual(rollback[2]["event"]["envelope"]["kind"], "rollback")
        self.assertEqual(
            rollback[2]["event"]["envelope"]["previous_revision_id"], revision_two_id
        )
        self.assertEqual(
            rollback[2]["event"]["envelope"]["target_revision_id"], revision_one_id
        )
        verify_signed_event(rollback[2]["event"])

        rolled = api(
            origin, "/api/v1/workflows/wf-published/publication", headers=auth_a
        )[2]
        self.assertEqual(rolled["current_published"]["revision_id"], revision_one_id)
        self.assertEqual(rolled["latest_published"]["revision_id"], revision_two_id)
        self.assertEqual(len(rolled["revisions"]), 2)
        self.assertEqual(rolled["mutable_draft"]["draft_version"], 3)
        self.assertEqual(rolled["difference"]["state"], "changed")
        self.assertIn("annotation", rolled["difference"]["fields"])
        self.assertEqual(rolled["current_event"]["envelope"]["kind"], "rollback")
        self.assertEqual(rolled["current_event"]["envelope"]["target_revision_id"], rolled["current_published"]["revision_id"])
        self.assertEqual(rolled["current_event"]["envelope"]["revision_digest"], rolled["current_published"]["revision_digest"])
        self.assertEqual(rolled["current_event"]["envelope"]["plan_digest"], rolled["current_published"]["plan_digest"])
        verify_signed_event(rolled["current_event"])
        current_draft = api(origin, "/api/v1/workflows/wf-published", headers=auth_a)[2]
        self.assertEqual(current_draft["annotation"], "published two")

        auth_a, _ = self.restart()
        origin = self.daemon.origin
        persisted = api(
            origin, "/api/v1/workflows/wf-published/publication", headers=auth_a
        )[2]
        self.assertEqual(persisted, rolled)
        verify_signed_event(persisted["current_event"])
        persisted_one = api(origin, revision_path, headers=auth_a)[2]
        revision_two_path = (
            "/api/v1/workflows/wf-published/revisions/"
            + quote(revision_two_id, safe="")
        )
        persisted_two = api(origin, revision_two_path, headers=auth_a)[2]
        self.assertEqual(persisted_one, published_one)
        self.assertEqual(persisted_two, published_two)
        verify_published_record(persisted_one)
        verify_published_record(persisted_two)


if __name__ == "__main__":
    unittest.main()
