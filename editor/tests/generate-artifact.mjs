// SPDX-License-Identifier: AGPL-3.0-or-later
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import net from "node:net";
import os from "node:os";
import { join, resolve } from "node:path";
import { spawn } from "node:child_process";
import { chromium } from "playwright";

const require = createRequire(import.meta.url);
const repo = resolve(import.meta.dirname, "../..");
const binary = process.env.WORKFLOWD_BIN || join(repo, "target/debug/workflowd");
const root = await mkdtemp(join(os.tmpdir(), "canopy-generate-ui-"));
const state = join(root, "state");
const key = join(root, "master.key");
const baselines = join(import.meta.dirname, "baselines");
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
    body: JSON.stringify({ email: "owner@example.test", password: "correct horse battery staple", recovery_passphrase: "separate recovery phrase long" }),
  });
  assert.equal(setup.status, 201);
  browser = await chromium.launch({ headless: true });
  const context = await browser.newContext({ viewport: { width: 1280, height: 1050 }, locale: "en-US", timezoneId: "UTC", colorScheme: "dark", reducedMotion: "reduce", bypassCSP: true });
  const page = await context.newPage();
  await page.goto(origin);
  await page.getByTestId("email").fill("owner@example.test");
  await page.getByTestId("password").fill("correct horse battery staple");
  await page.getByTestId("sign-in").click();
  await page.waitForFunction(() => Boolean(sessionStorage.getItem("canopy-editor-session-v1")));
  await page.waitForFunction(() => Boolean(localStorage.getItem("canopy-owner-session-v1")));

  const publication = await page.evaluate(async () => {
    const owner = JSON.parse(localStorage.getItem("canopy-owner-session-v1"));
    const session = sessionStorage.getItem("canopy-editor-session-v1");
    const csrf = owner.csrf_token;
    const mutate = async (path, body) => {
      const response = await fetch(path, { method: "POST", headers: { "content-type": "application/json", "x-canopy-csrf": csrf }, body: JSON.stringify(body) });
      const value = await response.json();
      if (!response.ok) throw new Error(`${path}: ${response.status} ${JSON.stringify(value)}`);
      return value;
    };
    const catalog = await (await fetch("/api/v1/catalog")).json();
    const locks = Object.fromEntries(catalog.nodes.map((node) => [node.contract_lock.name, node.contract_lock]));
    const workflowId = "wf-generate-browser";
    await mutate("/api/v1/workflows", { workflow_id: workflowId, name: "Generated Envelope Browser", annotation: "UI progress fixture", settings: {}, compatibility_metadata: {} });
    const lease = await mutate(`/api/v1/workflows/${workflowId}/editing/open`, { editor_session_id: session, label: "Generate browser tab" });
    let version = 0;
    const operations = [
      { kind: "add_node", node_instance: { id: "manual-trigger", name: "Manual Trigger", contract_lock: locks["manual-trigger"], configuration: { capture_mode: "manual" }, layout: { x: 100, y: 120 }, annotation: "", compatibility_metadata: {} } },
      { kind: "add_node", node_instance: { id: "generate-items", name: "Generate Items", contract_lock: locks["generate-items"], configuration: { count: 49_998, start: 0, step: 1, data: null, storage_mode: "artifact" }, layout: { x: 420, y: 120 }, annotation: "", compatibility_metadata: {} } },
      { kind: "connect", connection: { id: "manual-to-generate", source: { node_id: "manual-trigger", port_id: "invocation" }, target: { node_id: "generate-items", port_id: "input" } } },
    ];
    for (let index = 0; index < operations.length; index += 1) {
      const accepted = await mutate(`/api/v1/workflows/${workflowId}/draft-commands`, { editor_session_id: session, lease_generation: lease.lease_generation, command_id: `browser-generate-${index}`, base_draft_version: version, operation: operations[index] });
      version = accepted.draft_version;
    }
    const previewRequest = { editor_session_id: session, lease_generation: lease.lease_generation, draft_version: version };
    const preview = await mutate(`/api/v1/workflows/${workflowId}/compile-preview`, previewRequest);
    const published = await mutate(`/api/v1/workflows/${workflowId}/publish`, {
      publication_id: "publish-generate-browser",
      ...previewRequest,
      compile_input_digest: preview.compile_input_digest,
      acknowledged_diagnostics: preview.diagnostics.filter((item) => item.requires_ack).map((item) => item.fingerprint),
    });
    return { workflowId, published };
  });
  assert.ok(publication.published);
  await page.goto(`${origin}/?workflow=${publication.workflowId}`);
  await page.getByTestId("start-run").click();
  await page.getByTestId("generation-progress").waitFor({ timeout: 30_000 });
  await waitAttribute(page.getByTestId("run-durable-state"), "data-state", "succeeded", 900);
  const progressText = await page.getByTestId("generation-progress").textContent();
  assert.match(progressText ?? "", /Generated 49,998 items/);
  assert.match(progressText ?? "", /Encrypted spill/);
  assert.match(progressText ?? "", /256 Envelope queue cap/);
  await page.getByTestId("load-artifact-preview").click();
  await page.getByTestId("artifact-detail").waitFor();
  assert.match((await page.getByTestId("artifact-detail").textContent()) ?? "", /Verified before release/);
  assert.equal((await page.getByLabel("Bounded Artifact preview").textContent())?.trim(), "null");

  const axeSource = await readFile(require.resolve("axe-core/axe.min.js"), "utf8");
  await page.addScriptTag({ content: axeSource });
  const violations = await page.evaluate(async () => {
    const result = await globalThis.axe.run(document, { resultTypes: ["violations"], rules: { region: { enabled: false } } });
    return result.violations.filter((item) => item.impact === "critical" || item.impact === "serious").map((item) => ({ id: item.id, impact: item.impact, targets: item.nodes.map((node) => node.target) }));
  });
  assert.deepEqual(violations, []);

  await page.addStyleTag({ content: ".artifact-detail{display:none!important}" });
  await page.getByTestId("generation-progress").evaluate((element) => {
    const values = element.querySelectorAll(".generation-facts strong");
    values[0].textContent = "bounded";
    values[3].textContent = "sha256:verified…";
  });
  const desktop = await page.getByTestId("generation-progress").screenshot({ animations: "disabled", caret: "hide", scale: "css" });
  await page.setViewportSize({ width: 390, height: 900 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, "generated progress must fit 390px");
  const mobile = await page.getByTestId("generation-progress").screenshot({ animations: "disabled", caret: "hide", scale: "css" });
  await compareOrWrite("generate-progress.desktop.png", desktop);
  await compareOrWrite("generate-progress.mobile.png", mobile);
  console.log("generate-ui=passed lazy-preview-bytes=4 axe-serious=0 desktop-visual=passed mobile-visual=passed mobile-overflow=0");
} finally {
  if (browser) await browser.close();
  daemon.kill("SIGTERM");
  await new Promise((resolve) => daemon.once("exit", resolve));
  await rm(root, { recursive: true, force: true });
}

async function compareOrWrite(name, actual) {
  await mkdir(baselines, { recursive: true });
  const path = join(baselines, name);
  if (process.env.UPDATE_VISUAL_BASELINE === "1") {
    await writeFile(path, actual);
    return;
  }
  const expected = await readFile(path);
  assert.deepEqual(actual, expected, `${name} differs; review and run UPDATE_VISUAL_BASELINE=1 only to approve it`);
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
    try { if ((await fetch(`${origin}/health/live`)).ok) return; } catch { /* startup race */ }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("daemon did not start");
}
async function waitAttribute(locator, name, value, attempts = 200) {
  for (let attempt = 0; attempt < attempts; attempt += 1) {
    if ((await locator.getAttribute(name)) === value) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${name}=${value}; got ${await locator.getAttribute(name)}`);
}
