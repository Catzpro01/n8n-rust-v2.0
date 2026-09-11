// SPDX-License-Identifier: AGPL-3.0-or-later

import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import { clearOwnerSession, claimEditorSession, loadOwnerSession, saveOwnerSession, type OwnerSession } from "./editor-session";
import { clearRecoveryCopies, deleteRecoveryCopy, loadRecoveryCopies, purgeExpired, saveRecoveryCopy, type RecoveryCommand } from "./recovery";
import type { Catalog, CompilePreview, DraftDiffView, EditingStatus, PublishedRevision, PublicationView, RecoveryFork, WorkflowDraft } from "./editing-types";
import "./styles.css";

type Release = { product: string; version: string; build_commit: string };
type Readiness = { status: string; checks: { sqlite: { journal_mode: string; synchronous: string } } };
type Resources = { cpu: { available: boolean }; memory: { available: boolean } };
type Snapshot = { release?: Release; readiness?: Readiness; resources?: Resources; catalog?: Catalog; error?: string };
type SaveState = "saved" | "saving" | "offline" | "conflict";

function Mark() {
  return <svg class="mark" viewBox="0 0 48 48" role="img" aria-label="Canopy Workbench placeholder mark"><path d="M8 29 24 7l16 22-16 12Z" fill="none" stroke="currentColor" stroke-width="3" /><circle cx="24" cy="25" r="5" fill="currentColor" /></svg>;
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>({});
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [owner, setOwner] = useState<OwnerSession | undefined>(() => loadOwnerSession());
  const [editorSessionId, setEditorSessionId] = useState("");
  const [workflowId, setWorkflowId] = useState(() => new URLSearchParams(location.search).get("workflow") ?? "");
  const [draft, setDraft] = useState<WorkflowDraft>();
  const [editing, setEditing] = useState<EditingStatus>();
  const [annotation, setAnnotation] = useState("");
  const [forks, setForks] = useState<RecoveryFork[]>([]);
  const [pendingCount, setPendingCount] = useState(0);
  const [saveState, setSaveState] = useState<SaveState>("saved");
  const [publication, setPublication] = useState<PublicationView>();
  const [preview, setPreview] = useState<CompilePreview>();
  const [diffView, setDiffView] = useState<DraftDiffView>();
  const [acked, setAcked] = useState<string[]>([]);
  const [action, setAction] = useState("Sign in as the Owner to author a Draft.");

  useEffect(() => {
    void (owner ? purgeExpired(Date.now()) : clearRecoveryCopies()).catch(showError);
  }, []);

  useEffect(() => {
    let active = true;
    Promise.all([
      fetch("/api/v1/release").then((response) => response.json() as Promise<Release>),
      fetch("/health/ready").then((response) => response.json() as Promise<Readiness>),
      fetch("/api/v1/resources").then((response) => response.json() as Promise<Resources>),
      fetch("/catalog.v1.json").then((response) => response.json() as Promise<Catalog>),
    ]).then(([release, readiness, resources, catalog]) => {
      if (active) setSnapshot({ release, readiness, resources, catalog });
    }).catch((error: unknown) => {
      if (active) setSnapshot({ error: error instanceof Error ? error.message : "Connection failed" });
    });
    return () => { active = false; };
  }, []);

  useEffect(() => {
    let close: () => void = () => undefined;
    void claimEditorSession().then((claim) => { setEditorSessionId(claim.id); close = claim.close; }).catch(showError);
    return () => close();
  }, []);

  useEffect(() => {
    if (!owner) return;
    const expiresIn = owner.expires_at * 1000 - Date.now();
    if (expiresIn <= 0) { void expireLocalSession().catch(showError); return; }
    const timer = window.setTimeout(() => void expireLocalSession().catch(showError), expiresIn);
    return () => clearTimeout(timer);
  }, [owner]);

  useEffect(() => {
    if (!owner || !editorSessionId || !workflowId) return;
    let active = true;
    const open = async () => {
      await purgeExpired(Date.now());
      const loaded = await requestJson<WorkflowDraft>(`/api/v1/workflows/${encodeURIComponent(workflowId)}`);
      const lease = await mutateJson<EditingStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/editing/open`, { editor_session_id: editorSessionId, label: tabLabel(editorSessionId) });
      const recovered = await loadRecoveryCopies(workflowId);
      if (!active) return;
      setDraft(loaded); setAnnotation(loaded.annotation); setEditing(lease); setPendingCount(recovered.length);
      if (recovered.length) { setSaveState("offline"); setAction("Offline recovery copy is encrypted and waiting for reconciliation."); }
      await refreshForks(workflowId, active);
      await refreshPublication(workflowId).catch(() => { /* publication panel stays empty */ });
    };
    void open().catch(showError);
    const polling = window.setInterval(() => {
      void requestJson<EditingStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/editing/status?editor_session_id=${encodeURIComponent(editorSessionId)}`).then((status) => { if (active) setEditing(status); }).catch(() => { if (active) setSaveState("offline"); });
    }, 500);
    const heartbeat = window.setInterval(() => {
      void requestJson<EditingStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/editing/status?editor_session_id=${encodeURIComponent(editorSessionId)}`)
        .then((status) => status.role === "holder" ? mutateJson<EditingStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/editing/heartbeat`, { editor_session_id: editorSessionId, lease_generation: status.lease_generation }) : status)
        .then((status) => { if (active) setEditing(status); })
        .catch(() => { if (active) setSaveState("offline"); });
    }, 10_000);
    return () => { active = false; clearInterval(polling); clearInterval(heartbeat); };
  }, [owner?.csrf_token, editorSessionId, workflowId]);

  useEffect(() => {
    const warn = (event: BeforeUnloadEvent) => { if (pendingCount > 0 || saveState === "saving") { event.preventDefault(); event.returnValue = ""; } };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [pendingCount, saveState]);

  async function expireLocalSession() {
    clearOwnerSession();
    try { await clearRecoveryCopies(); }
    finally { setOwner(undefined); setPendingCount(0); setAction("Session expired. Browser recovery storage was cleared by policy."); }
  }
  function showError(error: unknown) { setAction(error instanceof Error ? error.message : "Request failed"); }
  async function requestJson<T>(path: string): Promise<T> {
    const response = await fetch(path);
    const body = await response.json() as T & { code?: string };
    if (!response.ok) throw new Error(body.code ?? "Request failed");
    return body;
  }
  async function mutateJson<T>(path: string, body: unknown): Promise<T> {
    if (!owner) throw new Error("Owner session required");
    const response = await fetch(path, { method: "POST", headers: { "content-type": "application/json", "x-canopy-csrf": owner.csrf_token }, body: JSON.stringify(body) });
    if (response.status === 204) return undefined as T;
    const value = await response.json() as T & { code?: string };
    if (!response.ok) throw Object.assign(new Error(value.code ?? "Request failed"), { response, value });
    return value;
  }
  async function signIn() {
    setAction("Signing in…");
    const response = await fetch("/api/v1/session/login", { method: "POST", headers: { "content-type": "application/json" }, body: JSON.stringify({ email, password }) });
    const body = await response.json() as OwnerSession & { code?: string };
    if (!response.ok || !body.csrf_token) throw new Error(body.code ?? "Sign in failed");
    saveOwnerSession(body); setOwner(body); setPassword(""); setAction("Owner signed in. This tab now has its own Editor Session.");
  }
  async function logout() {
    try {
      if (owner) await mutateJson<unknown>("/api/v1/session/logout", {});
    } finally {
      clearOwnerSession();
      try { await clearRecoveryCopies(); }
      finally { setOwner(undefined); setPendingCount(0); setAction("Signed out. Browser recovery storage was cleared."); }
    }
  }
  async function createManualDraft() {
    const node = snapshot.catalog?.nodes[0];
    if (!node || !owner || !editorSessionId) return;
    setAction("Creating Workflow and acquiring its Draft Lease…");
    const suffix = crypto.randomUUID(); const id = `wf-${suffix}`;
    await mutateJson<WorkflowDraft>("/api/v1/workflows", { workflow_id: id, name: "Manual Trigger Workflow", annotation: "Created from the native catalog", settings: {}, compatibility_metadata: {} });
    const lease = await mutateJson<EditingStatus>(`/api/v1/workflows/${id}/editing/open`, { editor_session_id: editorSessionId, label: tabLabel(editorSessionId) });
    setEditing(lease); setWorkflowId(id); history.replaceState(null, "", `/?workflow=${encodeURIComponent(id)}`);
    const command: RecoveryCommand = { editor_session_id: editorSessionId, lease_generation: lease.lease_generation, command_id: `cmd-${suffix}`, base_draft_version: 0, operation: { kind: "add_node", node_instance: { id: `manual-trigger-${suffix}`, name: node.display_name, contract_lock: node.contract_lock, configuration: { capture_mode: "manual" }, layout: { x: 160, y: 120 }, annotation: "", compatibility_metadata: {} } } };
    await sendRecoverable(id, command);
  }
  async function sendRecoverable(id: string, command: RecoveryCommand) {
    if (!owner) return;
    const copyId = `copy-${command.command_id}`;
    await saveRecoveryCopy({ recovery_copy_id: copyId, workflow_id: id, editor_session_id: editorSessionId, expires_at: owner.expires_at * 1000, command });
    setPendingCount((count) => count + 1); setSaveState("saving"); setAction("Saving — command is encrypted locally until the daemon acknowledges it.");
    try {
      const accepted = await mutateJson<{ draft_version: number }>(`/api/v1/workflows/${encodeURIComponent(id)}/draft-commands`, command);
      await deleteRecoveryCopy(copyId); setPendingCount((count) => Math.max(0, count - 1)); setSaveState("saved"); setAction(`Saved at Draft Version ${accepted.draft_version}.`); await refreshDraft(id);
      return accepted;
    } catch (error) {
      const online = navigator.onLine && !(error instanceof TypeError);
      setSaveState(online ? "conflict" : "offline");
      setAction(online ? "Conflict — command remains encrypted for explicit recovery." : "Offline — command remains encrypted until reconnect.");
      return undefined;
    }
  }
  async function saveAnnotation() {
    if (!draft) return;
    await sendRecoverable(draft.workflow_id, { editor_session_id: editorSessionId, lease_generation: editing?.lease_generation ?? 0, command_id: `annotation-${crypto.randomUUID()}`, base_draft_version: draft.draft_version, operation: { kind: "set_workflow_annotation", annotation } });
  }
  async function historyCommand(kind: "undo" | "redo") {
    if (!draft) return;
    await sendRecoverable(draft.workflow_id, { editor_session_id: editorSessionId, lease_generation: editing?.lease_generation ?? 0, command_id: `${kind}-${crypto.randomUUID()}`, base_draft_version: draft.draft_version, operation: { kind } });
  }
  async function refreshDraft(id = workflowId) {
    const loaded = await requestJson<WorkflowDraft>(`/api/v1/workflows/${encodeURIComponent(id)}`); setDraft(loaded); setAnnotation(loaded.annotation); await refreshForks(id, true); await refreshPublication(id).catch(() => { /* publication panel stays empty */ });
  }
  async function refreshForks(id: string, active: boolean) {
    const response = await requestJson<{ forks: RecoveryFork[] }>(`/api/v1/workflows/${encodeURIComponent(id)}/recovery-forks`); if (active) setForks(response.forks);
  }
  async function requestTakeover() {
    if (!workflowId) return;
    const status = await mutateJson<EditingStatus>(`/api/v1/workflows/${workflowId}/editing/takeover/request`, { editor_session_id: editorSessionId, request_id: `take-${crypto.randomUUID()}` }); setEditing(status); setAction("Takeover requested. The holder may approve, or the grace timer may elapse.");
  }
  async function respondTakeover(approve: boolean) {
    if (!workflowId || !editing?.takeover) return;
    const status = await mutateJson<EditingStatus>(`/api/v1/workflows/${workflowId}/editing/takeover/respond`, { editor_session_id: editorSessionId, lease_generation: editing.lease_generation, request_id: editing.takeover.request_id, approve }); setEditing(status); setAction(approve ? "Takeover approved; this tab is now read-only." : "Takeover declined.");
  }
  async function claimTakeover() {
    if (!workflowId || !editing?.takeover) return;
    const status = await mutateJson<EditingStatus>(`/api/v1/workflows/${workflowId}/editing/takeover/claim`, { editor_session_id: editorSessionId, request_id: editing.takeover.request_id }); setEditing(status); setAction("Takeover grace elapsed; this tab now holds the Lease.");
  }
  async function releaseLease() {
    const status = await mutateJson<EditingStatus>(`/api/v1/workflows/${workflowId}/editing/release`, { editor_session_id: editorSessionId, lease_generation: editing?.lease_generation ?? 0 }); setEditing(status); setAction("Draft Lease released voluntarily.");
  }
  async function recoverPending() {
    const copies = await loadRecoveryCopies(workflowId); if (!copies.length) return;
    const copy = copies[0];
    const response = await fetch(`/api/v1/workflows/${workflowId}/recovery/reconcile`, { method: "POST", headers: { "content-type": "application/json", "x-canopy-csrf": owner!.csrf_token }, body: JSON.stringify({ recovery_copy_id: copy.recovery_copy_id, editor_session_id: editorSessionId, command: copy.command }) });
    const body = await response.json() as { status: string; accepted?: { draft_version: number }; fork?: RecoveryFork };
    if (body.status === "conflict_fork" && body.fork) { await deleteRecoveryCopy(copy.recovery_copy_id); setPendingCount((count) => Math.max(0, count - 1)); setSaveState("conflict"); setForks((items) => [...items.filter((item) => item.fork_id !== body.fork!.fork_id), body.fork!]); setAction("Conflict — recovered work is safe in an explicit fork with a visible diff."); return; }
    if (response.ok) { await deleteRecoveryCopy(copy.recovery_copy_id); setPendingCount((count) => Math.max(0, count - 1)); setSaveState("saved"); setAction(`Recovered and acknowledged at Draft Version ${body.accepted?.draft_version}.`); await refreshDraft(); return; }
    throw new Error("Recovery reconciliation failed");
  }
  async function applyFork(fork: RecoveryFork) {
    if (!draft) return;
    const accepted = await mutateJson<{ draft_version: number }>(`/api/v1/workflows/${workflowId}/recovery-forks/${fork.fork_id}/apply`, { editor_session_id: editorSessionId, lease_generation: editing?.lease_generation ?? 0, command_id: `apply-${crypto.randomUUID()}`, base_draft_version: draft.draft_version }); setSaveState("saved"); setAction(`Recovery fork applied at Draft Version ${accepted.draft_version}.`); await refreshDraft();
  }
  async function refreshPublication(id = workflowId) {
    const [pub, diff] = await Promise.all([
      requestJson<PublicationView>(`/api/v1/workflows/${encodeURIComponent(id)}/publication`),
      requestJson<DraftDiffView>(`/api/v1/workflows/${encodeURIComponent(id)}/diff`),
    ]);
    setPublication(pub); setDiffView(diff);
  }
  async function compileDraft() {
    const result = await requestJson<CompilePreview>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/compile`);
    setPreview(result);
    if (result.status === "failed") setAction(`Compilation failed with ${result.diagnostics.length} diagnostic(s).`);
    else if (result.status === "warnings") setAction(`Compilation ready with ${result.diagnostics.length} warning(s); designated warnings need acknowledgement.`);
    else setAction("Compilation clean: the deterministic plan is ready.");
  }
  async function publishDraft() {
    if (!draft || !editing) return;
    const designated = (preview?.diagnostics ?? []).filter((diag) => diag.require_acknowledgement).map((diag) => diag.code);
    const published = await mutateJson<PublishedRevision>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/publish`, {
      editor_session_id: editorSessionId,
      lease_generation: editing.lease_generation,
      base_draft_version: draft.draft_version,
      acknowledged_warnings: acked.filter((code) => designated.includes(code)),
    });
    setAcked([]); setPreview(undefined); setSaveState("saved");
    setAction(`Published Revision ${published.revision_number} — document ${published.document_digest.slice(0, 22)}…, plan ${published.plan_digest.slice(0, 22)}…, signed ${published.signature.slice(0, 24)}…`);
    await refreshPublication(); await refreshDraft();
  }
  async function rollbackPublication() {
    const view = await mutateJson<PublicationView>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/rollback`, {});
    setPublication(view);
    setAction(`Rolled back: current Published Revision is now ${view.current_revision}. Neither the old nor the new revision was edited or deleted.`);
    await refreshPublication();
  }
  function toggleAck(code: string) {
    setAcked((current) => current.includes(code) ? current.filter((item) => item !== code) : [...current, code]);
  }

  const canWrite = editing?.role === "holder";
  return <main class="shell">
    <header class="masthead"><div class="identity"><Mark /><div><p class="eyebrow">Independent automation workspace</p><h1>Canopy Workbench</h1></div></div><span class={`health ${snapshot.readiness?.status === "ready" ? "ready" : "waiting"}`}><span aria-hidden="true" />{snapshot.readiness?.status ?? "connecting"}</span></header>
    <section class="welcome" aria-labelledby="welcome-title"><p class="eyebrow">Loss-aware Draft editing</p><h2 id="welcome-title">One writer. Honest recovery.</h2><p>The daemon grants one renewable Draft Lease. Every uncertain command is encrypted locally until acknowledged.</p>
      {!owner ? <form class="owner-login" onSubmit={(event) => { event.preventDefault(); void signIn().catch(showError); }}><label>Owner email<input data-testid="email" type="email" required value={email} onInput={(event) => setEmail(event.currentTarget.value)} /></label><label>Password<input data-testid="password" type="password" required value={password} onInput={(event) => setPassword(event.currentTarget.value)} /></label><button data-testid="sign-in" type="submit">Sign in</button></form> : <button data-testid="logout" type="button" onClick={() => void logout().catch(showError)}>Sign out and clear recovery copies</button>}
      <p class={`action ${saveState}`} data-testid="save-state" data-state={saveState} role="status">{action}</p>
    </section>
    {!workflowId && <section class="cards" aria-label="Installation identity and native catalog"><article><span class="number">01</span><h3>Release</h3><strong>{snapshot.release ? `v${snapshot.release.version}` : "—"}</strong><p>{snapshot.release?.build_commit ?? "Reading build identity…"}</p></article><article><span class="number">02</span><h3>Storage</h3><strong>{snapshot.readiness?.checks.sqlite.journal_mode?.toUpperCase() ?? "—"}</strong><p>Full durability</p></article><article><span class="number">03</span><h3>Resource view</h3><strong>{snapshot.resources?.cpu.available ? "Observed" : "Unavailable"}</strong><p>{snapshot.resources?.memory.available ? "Memory controller visible" : "Reported honestly"}</p></article><article><span class="number">04</span><h3>Native catalog</h3><strong>{snapshot.catalog?.nodes[0]?.display_name ?? "Loading…"}</strong><p>{snapshot.catalog?.nodes[0]?.description ?? "Reading contract metadata…"}</p><button data-testid="create-draft" type="button" disabled={!owner || !snapshot.catalog?.nodes[0] || !editorSessionId} onClick={() => void createManualDraft().catch(showError)}>Add to new Draft</button></article></section>}
    {workflowId && <section class="editor" data-testid="editor" data-workflow-id={workflowId} data-draft-version={draft?.draft_version ?? -1}><div class="editor-head"><div><p class="eyebrow">Mutable Draft</p><h2>{draft?.name ?? workflowId}</h2><code>{workflowId}</code></div><div class={`lease ${editing?.role ?? "waiting"}`} data-testid="lease-role"><strong>{editing?.role === "holder" ? "Lease holder" : editing?.role === "read_only" ? "Read only" : "Lease available"}</strong><span>generation {editing?.lease_generation ?? "—"}</span>{editing?.holder && <small>{editing.holder.label} · expires {new Date(editing.holder.expires_at).toLocaleTimeString()}</small>}</div></div>
      <div class="editor-actions">{editing?.role === "read_only" && !editing.takeover && <button data-testid="request-takeover" onClick={() => void requestTakeover().catch(showError)}>Request takeover</button>}{editing?.role === "read_only" && editing.takeover?.requested_by_me && <button data-testid="claim-takeover" disabled={editing.server_time < editing.takeover.eligible_at} onClick={() => void claimTakeover().catch(showError)}>Claim after grace</button>}{canWrite && editing?.takeover && <><button data-testid="approve-takeover" onClick={() => void respondTakeover(true).catch(showError)}>Approve takeover</button><button onClick={() => void respondTakeover(false).catch(showError)}>Decline</button></>}{canWrite && <button data-testid="release-lease" onClick={() => void releaseLease().catch(showError)}>Release Lease</button>}<button data-testid="refresh-draft" onClick={() => void refreshDraft().catch(showError)}>Refresh Draft</button></div>
      <label class="annotation">Workflow annotation<textarea data-testid="annotation" disabled={!canWrite} value={annotation} onInput={(event) => setAnnotation(event.currentTarget.value)} /></label><div class="editor-actions"><button data-testid="save-annotation" disabled={!canWrite} onClick={() => void saveAnnotation().catch(showError)}>Save annotation</button><button data-testid="undo" disabled={!canWrite} onClick={() => void historyCommand("undo").catch(showError)}>Undo</button><button data-testid="redo" disabled={!canWrite} onClick={() => void historyCommand("redo").catch(showError)}>Redo</button>{pendingCount > 0 && <button data-testid="recover-pending" onClick={() => void recoverPending().catch(showError)}>Reconcile {pendingCount} recovery copy</button>}</div>
      {forks.filter((fork) => fork.status === "open").map((fork) => <article class="recovery-fork" data-testid="recovery-fork" key={fork.fork_id}><h3>Recovery fork</h3><p>Base {fork.diff.base_draft_version} → current {fork.diff.current_draft_version}; authority changed: {String(fork.diff.authority_changed)}</p><pre>{JSON.stringify(fork.pending_operation, null, 2)}</pre><button data-testid="apply-fork" disabled={!canWrite} onClick={() => void applyFork(fork).catch(showError)}>Apply recovered work</button></article>)}
      {draft && <section class="publication" data-testid="publication" data-current-revision={publication?.current_revision ?? null}>
        <div class="pub-states">
          <article data-testid="draft-state"><p class="eyebrow">Mutable Draft</p><strong>v{draft.draft_version}</strong><p>Editable working state</p></article>
          <article data-testid="published-state"><p class="eyebrow">Published Revision</p><strong>{publication?.current_revision != null ? `v${publication.current_revision}` : "none"}</strong><p>{publication?.revisions.find((rev) => rev.revision_number === publication?.current_revision)?.document_digest ?? "Nothing published yet"}</p></article>
        </div>
        <div class="editor-actions">
          <button data-testid="compile-draft" onClick={() => void compileDraft().catch(showError)}>Compile Draft</button>
          <button data-testid="publish-draft" disabled={!canWrite || preview?.status === "failed"} onClick={() => void publishDraft().catch(showError)}>Publish Draft</button>
          <button data-testid="rollback-publication" disabled={publication?.current_revision == null || publication.current_revision < 2} onClick={() => void rollbackPublication().catch(showError)}>Roll back to previous</button>
        </div>
        {preview && preview.diagnostics.length > 0 && <ul class="diagnostics" data-testid="diagnostics">{preview.diagnostics.map((diag) => <li key={`${diag.code}:${diag.path}`} class={`diag ${diag.severity}`} data-testid={`diag-${diag.code}`} data-require-ack={String(diag.require_acknowledgement)}><strong>{diag.code}</strong> {diag.message} <code>{diag.path}</code>{diag.require_acknowledgement && <label data-testid={`ack-${diag.code}`}><input type="checkbox" checked={acked.includes(diag.code)} onChange={() => toggleAck(diag.code)} /> acknowledge in publication evidence</label>}</li>)}</ul>}
        {preview?.plan && <p class="plan-identity" data-testid="plan-identity">{preview.plan.plan_format} · {preview.plan.compiler_algorithm} · {preview.plan.plan_digest}</p>}
        <div class="diff" data-testid="publication-diff" data-published-revision={String(diffView?.published_revision ?? "none")}>
          <h3>Draft versus Published</h3>
          {diffView?.published_revision == null ? <p>No Published Revision yet — publish to see the visual difference.</p> : <>
            <div class="diff-columns">
              <div data-testid="diff-added"><h4>Added</h4><ul>{diffView.nodes.added.map((node) => <li key={node.id} data-testid="diff-node-added">{node.name} <code>{node.id}</code></li>)}{diffView.connections.added.map((connection) => <li key={connection} data-testid="diff-connection-added">{connection}</li>)}{diffView.workflow_fields.annotation && <li data-testid="diff-field-annotation">annotation</li>}{diffView.workflow_fields.settings && <li data-testid="diff-field-settings">settings</li>}{diffView.workflow_fields.name && <li data-testid="diff-field-name">name</li>}{diffView.workflow_fields.compatibility_metadata && <li data-testid="diff-field-metadata">compatibility metadata</li>}{diffView.nodes.added.length === 0 && diffView.connections.added.length === 0 && !diffView.workflow_fields.annotation && !diffView.workflow_fields.settings && !diffView.workflow_fields.name && !diffView.workflow_fields.compatibility_metadata && <li class="muted">none</li>}</ul></div>
              <div data-testid="diff-removed"><h4>Removed</h4><ul>{diffView.nodes.removed.map((node) => <li key={node.id} data-testid="diff-node-removed">{node.name} <code>{node.id}</code></li>)}{diffView.connections.removed.map((connection) => <li key={connection} data-testid="diff-connection-removed">{connection}</li>)}{diffView.nodes.removed.length === 0 && diffView.connections.removed.length === 0 && <li class="muted">none</li>}</ul></div>
              <div data-testid="diff-modified"><h4>Modified</h4><ul>{diffView.nodes.modified.map((node) => <li key={node.id} data-testid="diff-node-modified">{node.name} <code>{node.id}</code> — {node.changed.join(", ")}</li>)}{diffView.nodes.modified.length === 0 && <li class="muted">none</li>}</ul></div>
            </div>
          </>}
        </div>
        {publication && publication.revisions.length > 0 && <ol class="revision-history" data-testid="revision-history">{[...publication.revisions].reverse().map((rev) => <li key={rev.revision_number} data-testid={`revision-${rev.revision_number}`} data-status={rev.status}>v{rev.revision_number} · from draft v{rev.draft_version} · {rev.status} · {rev.document_digest}</li>)}</ol>}
      </section>}
    </section>}
    {snapshot.error && <p role="alert" class="error">Daemon connection failed: {snapshot.error}</p>}<footer>Placeholder identity · independently authored · no third-party editor assets</footer>
  </main>;
}
function tabLabel(id: string): string { return `Editor tab ${id.slice(-6)}`; }
render(<App />, document.getElementById("app")!);
