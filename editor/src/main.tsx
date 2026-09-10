// SPDX-License-Identifier: AGPL-3.0-or-later

import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import "./styles.css";

type Release = { product: string; version: string; build_commit: string };
type Readiness = { status: string; checks: { sqlite: { journal_mode: string; synchronous: string } } };
type Resources = {
  cpu: { available: boolean; values: Record<string, number | null> };
  memory: { available: boolean; values: Record<string, number | null> };
};
type ContractLock = { api_version: string; namespace: string; name: string; version: string; digest: string };
type Catalog = {
  nodes: Array<{
    display_name: string;
    description: string;
    contract_lock: ContractLock;
    configuration_schema: unknown;
    editor_hints: unknown;
  }>;
};
type Snapshot = { release?: Release; readiness?: Readiness; resources?: Resources; catalog?: Catalog; error?: string };

function Mark() {
  return (
    <svg class="mark" viewBox="0 0 48 48" role="img" aria-label="Canopy Workbench placeholder mark">
      <path d="M8 29 24 7l16 22-16 12Z" fill="none" stroke="currentColor" stroke-width="3" />
      <circle cx="24" cy="25" r="5" fill="currentColor" />
    </svg>
  );
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>({});
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [csrf, setCsrf] = useState("");
  const [action, setAction] = useState("Sign in as the Owner to author a Draft.");

  useEffect(() => {
    let active = true;
    Promise.all([
      fetch("/api/v1/release").then((response) => response.json() as Promise<Release>),
      fetch("/health/ready").then((response) => response.json() as Promise<Readiness>),
      fetch("/api/v1/resources").then((response) => response.json() as Promise<Resources>),
      fetch("/catalog.v1.json").then((response) => response.json() as Promise<Catalog>),
    ])
      .then(([release, readiness, resources, catalog]) => {
        if (active) setSnapshot({ release, readiness, resources, catalog });
      })
      .catch((error: unknown) => {
        if (active) setSnapshot({ error: error instanceof Error ? error.message : "Connection failed" });
      });
    return () => { active = false; };
  }, []);

  async function signIn() {
    setAction("Signing in…");
    const response = await fetch("/api/v1/session/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ email, password }),
    });
    const body = await response.json() as { csrf_token?: string; code?: string };
    if (!response.ok || !body.csrf_token) throw new Error(body.code ?? "Sign in failed");
    setCsrf(body.csrf_token);
    setPassword("");
    setAction("Owner signed in. The catalog can now create a durable Draft.");
  }

  async function createManualDraft() {
    const node = snapshot.catalog?.nodes[0];
    if (!node || !csrf) return;
    setAction("Creating Workflow and adding Manual Trigger…");
    const suffix = crypto.randomUUID();
    const workflowId = `wf-${suffix}`;
    const headers = { "content-type": "application/json", "x-canopy-csrf": csrf };
    const created = await fetch("/api/v1/workflows", {
      method: "POST",
      headers,
      body: JSON.stringify({
        workflow_id: workflowId,
        name: "Manual Trigger Workflow",
        annotation: "Created from the native catalog",
        settings: {},
        compatibility_metadata: {},
      }),
    });
    if (!created.ok) throw new Error("Workflow creation failed");
    const command = await fetch(`/api/v1/workflows/${encodeURIComponent(workflowId)}/draft-commands`, {
      method: "POST",
      headers,
      body: JSON.stringify({
        command_id: `cmd-${suffix}`,
        base_draft_version: 0,
        operation: {
          kind: "add_node",
          node_instance: {
            id: `manual-trigger-${suffix}`,
            name: node.display_name,
            contract_lock: node.contract_lock,
            configuration: { capture_mode: "manual" },
            layout: { x: 160, y: 120 },
            annotation: "",
            compatibility_metadata: {},
          },
        },
      }),
    });
    const accepted = await command.json() as { draft_version?: number; code?: string };
    if (!command.ok) throw new Error(accepted.code ?? "Manual Trigger command failed");
    setAction(`${node.display_name} added to ${workflowId} at Draft Version ${accepted.draft_version}.`);
  }

  return (
    <main class="shell">
      <header class="masthead">
        <div class="identity"><Mark /><div><p class="eyebrow">Independent automation workspace</p><h1>Canopy Workbench</h1></div></div>
        <span class={`health ${snapshot.readiness?.status === "ready" ? "ready" : "waiting"}`}><span aria-hidden="true" />{snapshot.readiness?.status ?? "connecting"}</span>
      </header>

      <section class="welcome" aria-labelledby="welcome-title">
        <p class="eyebrow">A calm place for dependable work</p>
        <h2 id="welcome-title">Shape the work. See the proof.</h2>
        <p>The local daemon owns every Draft. This editor sends semantic commands rather than storage-shaped patches.</p>
        {!csrf && <form class="owner-login" onSubmit={(event) => { event.preventDefault(); void signIn().catch((error: Error) => setAction(error.message)); }}>
          <label>Owner email<input type="email" required value={email} onInput={(event) => setEmail(event.currentTarget.value)} /></label>
          <label>Password<input type="password" required value={password} onInput={(event) => setPassword(event.currentTarget.value)} /></label>
          <button type="submit">Sign in</button>
        </form>}
        <p class="action" role="status">{action}</p>
      </section>

      <section class="cards" aria-label="Installation identity and native catalog">
        <article><span class="number">01</span><h3>Release</h3><strong>{snapshot.release ? `v${snapshot.release.version}` : "—"}</strong><p>{snapshot.release?.build_commit ?? "Reading build identity…"}</p></article>
        <article><span class="number">02</span><h3>Storage</h3><strong>{snapshot.readiness?.checks.sqlite.journal_mode?.toUpperCase() ?? "—"}</strong><p>{snapshot.readiness ? `${snapshot.readiness.checks.sqlite.synchronous.toUpperCase()} durability` : "Checking SQLite…"}</p></article>
        <article><span class="number">03</span><h3>Resource view</h3><strong>{snapshot.resources?.cpu.available ? "Observed" : "Unavailable"}</strong><p>{snapshot.resources?.memory.available ? "Memory controller visible" : "Reported honestly"}</p></article>
        <article><span class="number">04</span><h3>Native catalog</h3><strong>{snapshot.catalog?.nodes[0]?.display_name ?? "Loading…"}</strong><p>{snapshot.catalog?.nodes[0]?.description ?? "Reading declarative contract metadata…"}</p><button type="button" disabled={!csrf || !snapshot.catalog?.nodes[0]} onClick={() => void createManualDraft().catch((error: Error) => setAction(error.message))}>Add to new Draft</button></article>
      </section>

      {snapshot.error && <p role="alert" class="error">Daemon connection failed: {snapshot.error}</p>}
      <footer>Placeholder identity · independently authored · no third-party editor assets</footer>
    </main>
  );
}

render(<App />, document.getElementById("app")!);
