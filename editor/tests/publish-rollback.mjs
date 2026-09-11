// SPDX-License-Identifier: AGPL-3.0-or-later
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { createRequire } from "node:module";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { chromium } from "playwright";

const require = createRequire(import.meta.url);
const repo = resolve(import.meta.dirname, "../..");
const binary = process.env.WORKFLOWD_BIN || join(repo, "target/debug/workflowd");
const root = await mkdtemp(join(os.tmpdir(), "canopy-publication-ui-"));
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
  const context = await browser.newContext({ viewport: { width: 1280, height: 1000 }, bypassCSP: true });
  const page = await context.newPage();
  await page.goto(origin);
  await page.getByTestId("email").fill("owner@example.test");
  await page.getByTestId("password").fill("correct horse battery staple");
  await page.getByTestId("sign-in").click();
  await page.getByTestId("create-draft").click();
  await waitAttribute(page.getByTestId("editor"), "data-draft-version", "1");
  await waitText(page.getByTestId("publication-difference"), "Not published");
  await waitText(page.getByTestId("current-publication"), "None yet");

  // Keyboard activation, structured diagnostics, and required acknowledgement.
  const compile = page.getByTestId("compile-preview");
  await compile.focus();
  await page.keyboard.press("Enter");
  await page.getByTestId("compile-diagnostics").waitFor();
  await waitText(page.getByTestId("compile-diagnostics"), "W_OUTPUT_UNUSED");
  await waitText(page.getByTestId("compile-diagnostics"), "not connected or consumed");
  const publish = page.getByTestId("publish-revision");
  assert.equal(await publish.isDisabled(), true);
  await page.getByTestId("ack-W_OUTPUT_UNUSED").check();
  assert.equal(await publish.isEnabled(), true);
  await publish.click();
  await waitText(page.getByTestId("current-publication"), "Revision 1");
  await waitText(page.getByTestId("publication-difference"), "Matches published");
  await waitText(page.getByTestId("signature-identity"), "Ed25519 verified");

  // A new Draft command is visibly different, then becomes immutable Revision 2.
  await page.getByTestId("annotation").fill("second immutable revision");
  assert.equal(await page.getByTestId("compile-preview").isDisabled(), true, "unsaved editor input must block compilation");
  await page.getByTestId("save-annotation").click();
  await waitAttribute(page.getByTestId("editor"), "data-draft-version", "2");
  await waitText(page.getByTestId("publication-difference"), "Changed");
  await waitText(page.getByTestId("publication-difference"), "annotation");
  await page.getByTestId("compile-preview").click();
  await page.getByTestId("ack-W_OUTPUT_UNUSED").check();
  await page.getByTestId("publish-revision").click();
  await waitText(page.getByTestId("current-publication"), "Revision 2");
  await waitText(page.getByTestId("publication-difference"), "Matches published");
  await waitText(page.getByTestId("revision-history"), "Revision 1");
  await waitText(page.getByTestId("revision-history"), "Revision 2");

  // Rollback changes only the signed current pointer; the Mutable Draft stays intact.
  await page.getByTestId("rollback-revision-1").click();
  await waitText(page.getByTestId("current-publication"), "Revision 1");
  await waitText(page.getByTestId("newer-history"), "1 newer");
  assert.equal(await page.getByTestId("annotation").inputValue(), "second immutable revision");
  await waitText(page.getByTestId("publication-difference"), "Changed");
  await waitText(page.getByTestId("signature-identity"), "Signed rollback");
  if (process.env.WORKFLOWD_VISUAL_OUTPUT) {
    await page.screenshot({ path: process.env.WORKFLOWD_VISUAL_OUTPUT, fullPage: true });
  }

  // Automated accessibility gate with an exact-pinned, test-only axe engine.
  const axeSource = await readFile(require.resolve("axe-core/axe.min.js"), "utf8");
  await page.addScriptTag({ content: axeSource });
  const violations = await page.evaluate(async () => {
    const result = await globalThis.axe.run(document, {
      resultTypes: ["violations"],
      rules: { region: { enabled: false } },
    });
    return result.violations
      .filter((item) => item.impact === "critical" || item.impact === "serious")
      .map((item) => ({ id: item.id, impact: item.impact, nodes: item.nodes.length }));
  });
  assert.deepEqual(violations, []);

  // Narrow layout must not overflow, and the signed state is visually stable on reload.
  await page.setViewportSize({ width: 390, height: 1000 });
  const noOverflow = await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth);
  assert.equal(noOverflow, true, "publication editor must fit a 390px viewport");
  await page.reload();
  await page.getByTestId("publication-panel").waitFor();
  await waitText(page.getByTestId("current-publication"), "Revision 1");
  await waitText(page.getByTestId("newer-history"), "1 newer");
  const before = await page.getByTestId("publication-panel").screenshot();
  await page.reload();
  await waitText(page.getByTestId("current-publication"), "Revision 1");
  await waitText(page.getByTestId("newer-history"), "1 newer");
  const after = await page.getByTestId("publication-panel").screenshot();
  assert.equal(sha256(after), sha256(before), "publication panel pixels changed after persistence reload");

  console.log("publication-browser=passed axe-serious=0 visual-reload-diff=0 mobile-overflow=0");
} finally {
  if (browser) await browser.close();
  daemon.kill("SIGTERM");
  await new Promise((resolve) => daemon.once("exit", resolve));
  await rm(root, { recursive: true, force: true });
}

function sha256(value) { return createHash("sha256").update(value).digest("hex"); }
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
      // Startup race.
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("daemon did not start");
}
async function waitText(locator, text) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await locator.textContent())?.includes(text)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${text}: ${await locator.textContent()}`);
}
async function waitAttribute(locator, name, value) {
  for (let attempt = 0; attempt < 100; attempt += 1) {
    if ((await locator.getAttribute(name)) === value) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${name}=${value}; got ${await locator.getAttribute(name)}`);
}
