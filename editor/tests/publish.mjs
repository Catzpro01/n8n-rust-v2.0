// SPDX-License-Identifier: AGPL-3.0-or-later
import assert from "node:assert/strict";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { chromium } from "playwright";

const repo = resolve(import.meta.dirname, "../..");
const binary = process.env.WORKFLOWD_BIN || join(repo, "target/debug/workflowd");
const root = await mkdtemp(join(os.tmpdir(), "canopy-publish-"));
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
  const page = await (await browser.newContext()).newPage();
  await page.goto(origin);
  await page.getByTestId("email").fill("owner@example.test");
  await page.getByTestId("password").fill("correct horse battery staple");
  await page.getByTestId("sign-in").click();
  await page.getByTestId("create-draft").click();
  await page.getByTestId("editor").waitFor();
  await waitContains(page.getByTestId("save-state"), "Saved at Draft Version 1");
  await page.getByTestId("publication").waitFor();
  await page.getByTestId("draft-state").waitFor();
  assert.equal(
    await page.getByTestId("publication").getAttribute("data-current-revision"),
    null,
    "nothing is published before the first publication",
  );

  // Compile shows the designated warning and the plan identity.
  await page.getByTestId("compile-draft").click();
  const warning = page.getByTestId("diag-unconnected_trigger_output");
  await warning.waitFor();
  assert.equal(await warning.getAttribute("data-require-ack"), "true");
  await page.getByTestId("plan-identity").waitFor();

  // Publish without acknowledgement fails with the structured problem.
  await page.getByTestId("publish-draft").click();
  await waitContains(
    page.getByTestId("save-state"),
    "publication_warnings_unacknowledged",
  );
  assert.equal(
    await page.getByTestId("publication").getAttribute("data-current-revision"),
    null,
    "a blocked publication must not create a revision",
  );

  // Acknowledge, publish Revision 1, and verify both states are visible.
  await page
    .locator('[data-testid="ack-unconnected_trigger_output"] input')
    .check();
  await page.getByTestId("publish-draft").click();
  await waitContains(page.getByTestId("save-state"), "Published Revision 1");
  await waitContains(page.getByTestId("published-state"), "v1");
  assert.equal(
    await page.getByTestId("publication").getAttribute("data-current-revision"),
    "1",
  );
  const firstDigest = await page
    .getByTestId("revision-1")
    .textContent();
  assert.match(firstDigest ?? "", /^v1 · from draft v1 · current · sha256:/);

  // Annotate, compile again (acknowledgement is per publication), publish Revision 2.
  await page.getByTestId("annotation").fill("published browser note");
  await page.getByTestId("save-annotation").click();
  await waitDraftVersion(page, 2);
  await page.getByTestId("diff-field-annotation").waitFor();
  await page.getByTestId("compile-draft").click();
  await page.getByTestId("diag-unconnected_trigger_output").waitFor();
  await page
    .locator('[data-testid="ack-unconnected_trigger_output"] input')
    .check();
  await page.getByTestId("publish-draft").click();
  await waitContains(page.getByTestId("save-state"), "Published Revision 2");
  await waitContains(page.getByTestId("published-state"), "v2");
  await page.getByTestId("diff-field-annotation").waitFor({ state: "detached" });
  assert.equal(
    await page.locator('[data-testid="diff-added"] li.muted').count(),
    1,
    "diff is clean after publishing the identical draft",
  );
  assert.equal(
    await page.getByTestId("revision-1").getAttribute("data-status"),
    "superseded",
  );
  assert.equal(
    await page.getByTestId("revision-2").getAttribute("data-status"),
    "current",
  );

  // Rollback selects the preceding revision as current without editing it.
  await page.getByTestId("rollback-publication").click();
  await waitContains(page.getByTestId("save-state"), "Rolled back");
  await waitContains(page.getByTestId("published-state"), "v1");
  assert.equal(
    await page.getByTestId("publication").getAttribute("data-current-revision"),
    "1",
  );
  assert.equal(
    await page.getByTestId("revision-1").getAttribute("data-status"),
    "current",
  );
  assert.equal(
    await page.getByTestId("revision-2").getAttribute("data-status"),
    "superseded",
  );
  assert.match(
    (await page.getByTestId("revision-1").textContent()) ?? "",
    /^v1 · from draft v1 · current · sha256:/,
    "the rolled-back revision is the same immutable record",
  );
  assert.equal(
    await page.getByTestId("revision-2").count(),
    1,
    "rollback never deletes history",
  );
  // At v1 there is no preceding revision: the control is disabled and the
  // daemon-side 409 (no_preceding_revision) is covered by the API journey.
  assert.equal(
    await page.getByTestId("rollback-publication").getAttribute("disabled"),
    "",
    "no preceding revision remains at v1",
  );

  console.log(
    "publication-browser=passed compile-ack-publish=passed rollback-history=passed",
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
  for (let attempt = 0; attempt < 200; attempt += 1) {
    try {
      if ((await fetch(`${origin}/health/live`)).ok) return;
    } catch {
      /* startup */
    }
    await new Promise((resolve) => setTimeout(resolve, 30));
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
