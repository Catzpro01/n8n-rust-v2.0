// SPDX-License-Identifier: AGPL-3.0-or-later
import assert from "node:assert/strict";
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
const root = await mkdtemp(join(os.tmpdir(), "canopy-run-trace-ui-"));
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
  const context = await browser.newContext({ viewport: { width: 1280, height: 1100 }, bypassCSP: true });
  const page = await context.newPage();
  await page.goto(origin);
  await page.getByTestId("email").fill("owner@example.test");
  await page.getByTestId("password").fill("correct horse battery staple");
  await page.getByTestId("sign-in").click();
  await page.getByTestId("create-draft").click();
  await waitAttribute(page.getByTestId("editor"), "data-draft-version", "1");
  await page.getByTestId("compile-preview").click();
  await page.getByTestId("compile-diagnostics").waitFor();
  await page.getByTestId("ack-W_OUTPUT_UNUSED").check();
  await page.getByTestId("publish-revision").click();
  await waitText(page.getByTestId("current-publication"), "Revision 1");

  // Start is browser-visible, binds the exact current publication, and converges to one Activation.
  await page.getByTestId("start-run").click();
  await page.getByTestId("run-id").waitFor();
  await waitAttribute(page.getByTestId("run-durable-state"), "data-state", "succeeded");
  await page.getByTestId("causal-trace").waitFor();
  await waitText(page.getByTestId("activation-outcome"), "success");
  await waitText(page.getByTestId("checkpoint-count"), "2");
  await waitText(page.getByTestId("run-stream-state"), "complete");
  const firstRunId = (await page.getByTestId("run-id").textContent())?.trim();
  assert.match(firstRunId ?? "", /^run-/);

  // A browser fetch through the public SSE seam sees an explicit gap and resync.
  const gapResync = await page.evaluate(async (runId) => {
    const controller = new AbortController();
    const response = await fetch(`/api/v1/runs/${encodeURIComponent(runId)}/events`, {
      headers: { "Last-Event-ID": "v1.unknown.999.old.999" },
      signal: controller.signal,
    });
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let text = "";
    try {
      while (!text.includes("event: resync")) {
        const next = await reader.read();
        if (next.done) break;
        text += decoder.decode(next.value, { stream: true });
      }
    } finally {
      controller.abort();
    }
    return { status: response.status, text };
  }, firstRunId);
  assert.equal(gapResync.status, 200);
  assert.match(gapResync.text, /event: gap/);
  assert.match(gapResync.text, /event: resync/);
  assert.match(gapResync.text, /"terminal":true/);

  // A second Run is cancelled while durably queued; terminal history is explicit and immutable.
  await page.getByTestId("start-run").click();
  await waitNotText(page.getByTestId("run-id"), firstRunId);
  await page.getByTestId("cancel-run").waitFor();
  await page.getByTestId("cancel-run").click();
  await waitAttribute(page.getByTestId("run-durable-state"), "data-state", "cancelled");
  await page.getByTestId("causal-trace").waitFor();
  await waitText(page.getByTestId("causal-trace"), "No Activation was dispatched");
  await waitText(page.getByTestId("checkpoint-count"), "2");

  // Automated accessibility and narrow-layout gates include the Run and Causal Trace experience.
  const axeSource = await readFile(require.resolve("axe-core/axe.min.js"), "utf8");
  await page.addScriptTag({ content: axeSource });
  const violations = await page.evaluate(async () => {
    const result = await globalThis.axe.run(document, {
      resultTypes: ["violations"],
      rules: { region: { enabled: false } },
    });
    return result.violations
      .filter((item) => item.impact === "critical" || item.impact === "serious")
      .map((item) => ({ id: item.id, impact: item.impact, nodes: item.nodes.length, targets: item.nodes.map((node) => ({ target: node.target, summary: node.failureSummary, html: node.html })) }));
  });
  assert.deepEqual(violations, []);
  await page.setViewportSize({ width: 390, height: 1000 });
  assert.equal(
    await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth),
    true,
    "Run evidence must fit a 390px viewport",
  );

  // The URL retains Run identity; reload reconstructs durable state and the same trace pixels.
  await waitText(page.getByTestId("run-stream-state"), "complete");
  await page.mouse.move(0, 0);
  const beforeMarkup = await page.getByTestId("run-panel").evaluate((element) => element.innerHTML);
  await page.reload();
  await waitAttribute(page.getByTestId("run-durable-state"), "data-state", "cancelled");
  await page.getByTestId("causal-trace").waitFor();
  await waitText(page.getByTestId("run-stream-state"), "complete");
  await waitText(page.getByTestId("current-publication"), "Revision 1");
  await page.mouse.move(0, 0);
  const afterMarkup = await page.getByTestId("run-panel").evaluate((element) => element.innerHTML);
  assert.equal(afterMarkup, beforeMarkup, "Run evidence markup changed after durable reload");

  console.log("run-browser=passed sse-gap-resync=passed axe-serious=0 reload-markup-diff=0 mobile-overflow=0");
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
      // Startup race.
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error("daemon did not start");
}
async function waitText(locator, text) {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await locator.textContent())?.includes(text)) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${text}: ${await locator.textContent()}`);
}
async function waitNotText(locator, text) {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await locator.textContent())?.trim() !== text) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected text to change from ${text}`);
}
async function waitAttribute(locator, name, value) {
  for (let attempt = 0; attempt < 150; attempt += 1) {
    if ((await locator.getAttribute(name)) === value) return;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`expected ${name}=${value}; got ${await locator.getAttribute(name)}`);
}
