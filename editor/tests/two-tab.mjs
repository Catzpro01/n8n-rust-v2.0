// SPDX-License-Identifier: AGPL-3.0-or-later
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { chromium } from "playwright";

const repo = resolve(import.meta.dirname, "../..");
const binary =
  process.env.WORKFLOWD_BIN || join(repo, "target/debug/workflowd");
const root = await mkdtemp(join(os.tmpdir(), "canopy-two-tab-"));
const state = join(root, "state");
const key = join(root, "master.key");
await writeFile(key, crypto.getRandomValues(new Uint8Array(32)));
const port = await freePort();
const origin = `http://localhost:${port}`;
const daemon = spawn(binary, ["serve"], {
  cwd: repo,
  env: {
    ...process.env,
    WORKFLOWD_BIND: `127.0.0.1:${port}`,
    WORKFLOWD_CONTROL_ORIGIN: origin,
    WORKFLOWD_STATE_DIR: state,
    WORKFLOWD_MASTER_KEY_FILE: key,
    WORKFLOWD_ARGON_MEMORY_KIB: "8192",
    WORKFLOWD_ARGON_ITERATIONS: "1",
    WORKFLOWD_DRAFT_LEASE_TTL_SECONDS: "30",
    WORKFLOWD_DRAFT_TAKEOVER_GRACE_SECONDS: "1",
  },
  stdio: ["ignore", "pipe", "pipe"],
});
let browser;
try {
  await ready(origin);
  const setup = await fetch(`${origin}/api/v1/setup`, {
    method: "POST",
    headers: { origin, "content-type": "application/json" },
    body: JSON.stringify({
      email: "owner@example.test",
      password: "correct horse battery staple",
      recovery_passphrase: "separate recovery phrase long",
    }),
  });
  assert.equal(setup.status, 201);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext();
  const tabA = await context.newPage();
  await tabA.goto(origin);
  await tabA.getByTestId("email").fill("owner@example.test");
  await tabA.getByTestId("password").fill("correct horse battery staple");
  await tabA.getByTestId("sign-in").click();
  await tabA.getByTestId("create-draft").click();
  await tabA.getByTestId("editor").waitFor();
  await waitContains(
    tabA.getByTestId("save-state"),
    "Saved at Draft Version 1",
  );
  await waitContains(tabA.getByTestId("lease-role"), "Lease holder");
  const workflowId = await tabA
    .getByTestId("editor")
    .getAttribute("data-workflow-id");
  assert.match(workflowId ?? "", /^wf-/);

  const tabB = await context.newPage();
  await tabB.goto(`${origin}/?workflow=${encodeURIComponent(workflowId)}`);
  await tabB.getByTestId("editor").waitFor();
  await waitContains(tabB.getByTestId("lease-role"), "Read only");
  const sessionA = await tabA.evaluate(() =>
    sessionStorage.getItem("canopy-editor-session-v1"),
  );
  assert.ok(sessionA);
  await tabB.evaluate(
    (copied) => sessionStorage.setItem("canopy-editor-session-v1", copied),
    sessionA,
  );
  await tabB.reload();
  await waitContains(tabB.getByTestId("lease-role"), "Read only");
  const sessionB = await tabB.evaluate(() =>
    sessionStorage.getItem("canopy-editor-session-v1"),
  );
  assert.ok(
    sessionB && sessionA !== sessionB,
    "copied tab identity must be detected and replaced",
  );

  await tabA.route("**/draft-commands", (route) =>
    route.abort("internetdisconnected"),
  );
  await tabA.getByTestId("annotation").fill("offline work from tab A");
  await tabA.getByTestId("save-annotation").click();
  await waitState(tabA, "offline");
  const encrypted = await recoveryStorage(tabA);
  assert.equal(encrypted.copies, 1);
  assert.equal(encrypted.keys, 1);
  assert.equal(encrypted.extractable, false);
  assert.equal(encrypted.algorithm, "AES-GCM");
  assert.equal(encrypted.keyBits, 256);
  assert.ok(encrypted.ciphertextBytes > 24);
  assert.equal(encrypted.recordHasCommand, false);
  assert.equal(encrypted.ciphertextContainsAnnotation, false);

  let beforeUnload = false;
  tabA.once("dialog", async (dialog) => {
    beforeUnload = dialog.type() === "beforeunload";
    await dialog.accept();
  });
  await tabA.reload();
  await tabA.getByTestId("recover-pending").waitFor();
  assert.equal(
    beforeUnload,
    true,
    "reload with unacknowledged work must warn honestly",
  );
  await waitState(tabA, "offline");

  await tabB.getByTestId("request-takeover").click();
  await tabA.getByTestId("approve-takeover").waitFor();
  await tabA.getByTestId("approve-takeover").click();
  await waitContains(tabB.getByTestId("lease-role"), "Lease holder");
  await waitContains(tabA.getByTestId("lease-role"), "Read only");

  await tabB.getByTestId("annotation").fill("online authority from tab B");
  await tabB.getByTestId("save-annotation").click();
  await waitDraftVersion(tabB, 2);
  await waitState(tabB, "saved");
  await tabA.unroute("**/draft-commands");
  await tabA.getByTestId("recover-pending").click();
  await waitState(tabA, "conflict");
  await tabA.getByTestId("recovery-fork").waitFor();
  await tabB.getByTestId("refresh-draft").click();
  await tabB.getByTestId("recovery-fork").waitFor();
  await tabB.getByTestId("apply-fork").click();
  await waitDraftVersion(tabB, 3);
  await waitValue(tabB.getByTestId("annotation"), "offline work from tab A");

  await tabB.getByTestId("undo").click();
  await waitDraftVersion(tabB, 4);
  await waitValue(
    tabB.getByTestId("annotation"),
    "online authority from tab B",
  );
  await tabB.reload();
  await waitContains(tabB.getByTestId("lease-role"), "Lease holder");
  await tabB.getByTestId("redo").click();
  await waitDraftVersion(tabB, 5);
  await waitValue(tabB.getByTestId("annotation"), "offline work from tab A");

  await tabB.route("**/draft-commands", (route) =>
    route.abort("internetdisconnected"),
  );
  await tabB
    .getByTestId("annotation")
    .fill("logout must clear this pending plaintext");
  await tabB.getByTestId("save-annotation").click();
  await waitState(tabB, "offline");
  assert.equal((await recoveryStorage(tabB)).copies, 1);
  await tabB.getByTestId("logout").click();
  await tabB.getByTestId("sign-in").waitFor();
  assert.deepEqual(await recoveryStorage(tabB), {
    copies: 0,
    keys: 0,
    extractable: null,
    algorithm: null,
    keyBits: null,
    ciphertextBytes: 0,
    recordHasCommand: false,
    ciphertextContainsAnnotation: false,
  });

  await tabB.unroute("**/draft-commands");
  await tabB.getByTestId("email").fill("owner@example.test");
  await tabB.getByTestId("password").fill("correct horse battery staple");
  await tabB.getByTestId("sign-in").click();
  await tabB.getByTestId("logout").waitFor();
  await waitContains(tabB.getByTestId("lease-role"), "Lease holder");
  await tabB.route("**/draft-commands", (route) =>
    route.abort("internetdisconnected"),
  );
  await tabB
    .getByTestId("annotation")
    .fill("expiry must clear this pending plaintext");
  await tabB.getByTestId("save-annotation").click();
  await waitState(tabB, "offline");
  await tabB.evaluate(() => {
    const session = JSON.parse(localStorage.getItem("canopy-owner-session-v1"));
    session.expires_at = Math.floor(Date.now() / 1000) + 1;
    localStorage.setItem("canopy-owner-session-v1", JSON.stringify(session));
  });
  tabB.once("dialog", (dialog) => dialog.accept());
  await tabB.reload();
  await tabB.getByTestId("sign-in").waitFor({ timeout: 5_000 });
  const expired = await waitRecoveryCleared(tabB);
  assert.equal(expired.copies, 0);
  assert.equal(expired.keys, 0);

  console.log(
    "two-tab-browser=passed encrypted-recovery=passed undo-reload-redo=passed",
  );
} finally {
  if (browser) await browser.close();
  daemon.kill("SIGTERM");
  await new Promise((resolve) => daemon.once("exit", resolve));
  await rm(root, { recursive: true, force: true });
}

async function freePort() {
  const server = net.createServer();
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const address = server.address();
  const port = typeof address === "object" && address ? address.port : 0;
  await new Promise((resolve) => server.close(resolve));
  return port;
}
async function ready(origin) {
  for (let attempt = 0; attempt < 400; attempt += 1) {
    try {
      if ((await fetch(`${origin}/health/live`)).ok) return;
    } catch {
      /* startup */
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("daemon did not start");
}
async function waitContains(locator, text) {
  await locator.waitFor();
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await locator.textContent())?.includes(text)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${text}: ${await locator.textContent()}`);
}
async function waitState(page, state) {
  const locator = page.getByTestId("save-state");
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await locator.getAttribute("data-state")) === state) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(
    `expected state ${state}; got ${await locator.getAttribute("data-state")}: ${await locator.textContent()}`,
  );
}
async function waitDraftVersion(page, version) {
  const editor = page.getByTestId("editor");
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await editor.getAttribute("data-draft-version")) === String(version))
      return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(
    `expected Draft Version ${version}; got ${await editor.getAttribute("data-draft-version")}`,
  );
}
async function waitValue(locator, value) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await locator.inputValue()) === value) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(
    `expected input value ${value}; got ${await locator.inputValue()}`,
  );
}
async function waitRecoveryCleared(page) {
  for (let attempt = 0; attempt < 50; attempt += 1) {
    const state = await recoveryStorage(page);
    if (state.copies === 0 && state.keys === 0) return state;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return recoveryStorage(page);
}
async function recoveryStorage(page) {
  return page.evaluate(async () => {
    const opening = indexedDB.open("canopy-recovery-v1", 1);
    const database = await new Promise((resolve, reject) => {
      opening.onsuccess = () => resolve(opening.result);
      opening.onerror = () => reject(opening.error);
    });
    const read = (store) =>
      new Promise((resolve, reject) => {
        const request = database
          .transaction(store, "readonly")
          .objectStore(store)
          .getAll();
        request.onsuccess = () => resolve(request.result);
        request.onerror = () => reject(request.error);
      });
    const [copies, keys] = await Promise.all([read("copies"), read("keys")]);
    database.close();
    const record = copies[0];
    const key = keys[0]?.key;
    const ciphertextText = record
      ? new TextDecoder().decode(new Uint8Array(record.ciphertext))
      : "";
    return {
      copies: copies.length,
      keys: keys.length,
      extractable: key ? key.extractable : null,
      algorithm: key?.algorithm?.name ?? null,
      keyBits: key?.algorithm?.length ?? null,
      ciphertextBytes: record?.ciphertext?.byteLength ?? 0,
      recordHasCommand: record
        ? Object.hasOwn(record, "command") ||
          JSON.stringify(Object.keys(record)).includes("annotation")
        : false,
      ciphertextContainsAnnotation: ciphertextText.includes("annotation"),
    };
  });
}
