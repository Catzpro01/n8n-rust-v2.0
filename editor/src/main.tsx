// SPDX-License-Identifier: AGPL-3.0-or-later

import { render } from "preact";
import { useEffect, useState } from "preact/hooks";
import { clearOwnerSession, claimEditorSession, loadOwnerSession, saveOwnerSession, type OwnerSession } from "./editor-session";
import { clearRecoveryCopies, deleteRecoveryCopy, loadRecoveryCopies, purgeExpired, saveRecoveryCopy, type RecoveryCommand } from "./recovery";
import type { ArtifactView, Catalog, CompilePreview, EditingStatus, PublicationStatus, RecoveryFork, RevisionSummary, RunView, TraceView, WorkflowDraft } from "./editing-types";
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
  const [action, setAction] = useState("Sign in as the Owner to author a Draft.");
  const [publication, setPublication] = useState<PublicationStatus>();
  const [compilePreview, setCompilePreview] = useState<CompilePreview>();
  const [acknowledged, setAcknowledged] = useState<string[]>([]);
  const [publicationBusy, setPublicationBusy] = useState(false);
  const [activeRunId, setActiveRunId] = useState(() => new URLSearchParams(location.search).get("run") ?? "");
  const [run, setRun] = useState<RunView>();
  const [trace, setTrace] = useState<TraceView>();
  const [runBusy, setRunBusy] = useState(false);
  const [streamState, setStreamState] = useState<"idle" | "connecting" | "connected" | "gap" | "resynced" | "complete" | "disconnected">("idle");
  const [artifactView, setArtifactView] = useState<ArtifactView>();
  const [artifactPreview, setArtifactPreview] = useState("");
  const [artifactBusy, setArtifactBusy] = useState(false);

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
    setCompilePreview(undefined);
    setAcknowledged([]);
    const open = async () => {
      await purgeExpired(Date.now());
      const loaded = await requestJson<WorkflowDraft>(`/api/v1/workflows/${encodeURIComponent(workflowId)}`);
      const lease = await mutateJson<EditingStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/editing/open`, { editor_session_id: editorSessionId, label: tabLabel(editorSessionId) });
      const recovered = await loadRecoveryCopies(workflowId);
      const publicationState = await requestJson<PublicationStatus>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/publication`);
      if (!active) return;
      setDraft(loaded); setAnnotation(loaded.annotation); setEditing(lease); setPendingCount(recovered.length); setPublication(publicationState);
      if (recovered.length) { setSaveState("offline"); setAction("Offline recovery copy is encrypted and waiting for reconciliation."); }
      await refreshForks(workflowId, active);
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
    setArtifactView(undefined);
    setArtifactPreview("");
  }, [activeRunId]);

  useEffect(() => {
    if (!owner || !activeRunId) { setStreamState("idle"); return; }
    let active = true;
    let source: EventSource | undefined;
    const refresh = async () => {
      const current = await requestJson<RunView>(`/api/v1/runs/${encodeURIComponent(activeRunId)}`);
      if (!active) return;
      setRun(current);
      if (current.durable.terminal) {
        const evidence = await requestJson<TraceView>(`/api/v1/runs/${encodeURIComponent(activeRunId)}/trace`);
        if (active) {
          setTrace(evidence);
          setStreamState("complete");
          source?.close();
        }
      }
    };
    setStreamState("connecting");
    void refresh().catch(showError);
    source = new EventSource(`/api/v1/runs/${encodeURIComponent(activeRunId)}/events`);
    source.onopen = () => { if (active) setStreamState("connected"); };
    const update = (event: Event) => {
      if (!active) return;
      const message = event as MessageEvent<string>;
      try {
        const payload = JSON.parse(message.data) as { run?: RunView };
        if (payload.run) {
          setRun(payload.run);
          if (payload.run.durable.terminal) void refresh().catch(showError);
        } else {
          void refresh().catch(showError);
        }
      } catch {
        void refresh().catch(showError);
      }
    };
    source.addEventListener("snapshot", update);
    source.addEventListener("live", update);
    source.addEventListener("durable", update);
    source.addEventListener("checkpoint", update);
    source.addEventListener("generation-progress", update);
    source.addEventListener("terminal", update);
    source.addEventListener("gap", (event) => { if (active) setStreamState("gap"); update(event); });
    source.addEventListener("resync", (event) => { if (active) setStreamState("resynced"); update(event); });
    source.onerror = () => { if (active) setStreamState("disconnected"); };
    return () => { active = false; source.close(); };
  }, [owner?.csrf_token, activeRunId]);

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
    setEditing(lease);
    const command: RecoveryCommand = { editor_session_id: editorSessionId, lease_generation: lease.lease_generation, command_id: `cmd-${suffix}`, base_draft_version: 0, operation: { kind: "add_node", node_instance: { id: `manual-trigger-${suffix}`, name: node.display_name, contract_lock: node.contract_lock, configuration: { capture_mode: "manual" }, layout: { x: 160, y: 120 }, annotation: "", compatibility_metadata: {} } } };
    await sendRecoverable(id, command);
    setWorkflowId(id);
    setActiveRunId(""); setRun(undefined); setTrace(undefined);
    history.replaceState(null, "", `/?workflow=${encodeURIComponent(id)}`);
  }
  async function sendRecoverable(id: string, command: RecoveryCommand) {
    if (!owner) return;
    setCompilePreview(undefined);
    setAcknowledged([]);
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
    const loaded = await requestJson<WorkflowDraft>(`/api/v1/workflows/${encodeURIComponent(id)}`);
    setDraft(loaded);
    setAnnotation(loaded.annotation);
    setCompilePreview((preview) => preview?.draft_version === loaded.draft_version ? preview : undefined);
    if (compilePreview?.draft_version !== loaded.draft_version) setAcknowledged([]);
    await Promise.all([refreshForks(id, true), refreshPublication(id)]);
  }
  async function refreshPublication(id = workflowId) {
    if (!id) return;
    const status = await requestJson<PublicationStatus>(`/api/v1/workflows/${encodeURIComponent(id)}/publication`);
    setPublication(status);
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
  async function runCompilePreview() {
    if (!draft || !editing) return;
    setPublicationBusy(true);
    setAction("Compiling the exact Mutable Draft…");
    try {
      const preview = await mutateJson<CompilePreview>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/compile-preview`, {
        editor_session_id: editorSessionId,
        lease_generation: editing.lease_generation,
        draft_version: draft.draft_version,
      });
      setCompilePreview(preview);
      setAcknowledged([]);
      setAction(preview.can_publish ? "Compile Preview is ready. Review diagnostics and acknowledge designated warnings." : "Compile Preview found blocking errors. Nothing was published.");
    } finally {
      setPublicationBusy(false);
    }
  }
  async function publishRevision() {
    if (!draft || !editing || !compilePreview) return;
    setPublicationBusy(true);
    setAction("Recompiling and atomically publishing the exact Draft…");
    try {
      await mutateJson<unknown>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/publish`, {
        publication_id: `publication-${crypto.randomUUID()}`,
        editor_session_id: editorSessionId,
        lease_generation: editing.lease_generation,
        draft_version: draft.draft_version,
        compile_input_digest: compilePreview.compile_input_digest,
        acknowledged_diagnostics: acknowledged,
      });
      setCompilePreview(undefined);
      setAcknowledged([]);
      await refreshPublication();
      setAction("Signed immutable Revision published with its pinned Execution Plan.");
    } finally {
      setPublicationBusy(false);
    }
  }
  async function rollbackRevision(revision: RevisionSummary) {
    if (!draft || !editing) return;
    setPublicationBusy(true);
    setAction(`Signing rollback to immutable Revision ${revision.sequence}…`);
    try {
      await mutateJson<unknown>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/rollback`, {
        rollback_id: `rollback-${crypto.randomUUID()}`,
        editor_session_id: editorSessionId,
        lease_generation: editing.lease_generation,
        draft_version: draft.draft_version,
        target_revision_id: revision.revision_id,
      });
      await refreshPublication();
      setAction(`Signed rollback completed. Mutable Draft Version ${draft.draft_version} was not replaced.`);
    } finally {
      setPublicationBusy(false);
    }
  }
  async function startManualRun() {
    const current = publication?.current_published;
    const event = publication?.current_event?.envelope;
    if (!current || !event) return;
    setRunBusy(true);
    setTrace(undefined);
    setAction("Durably admitting the exact Published Revision…");
    try {
      const admitted = await mutateJson<{ created: boolean; run: RunView }>(`/api/v1/workflows/${encodeURIComponent(workflowId)}/runs`, {
        run_request_id: `run-request-${crypto.randomUUID()}`,
        publication_event_id: event.event_id,
        revision_id: current.revision_id,
        plan_digest: current.plan_digest,
        captured_invocation: { manual: true },
      });
      setRun(admitted.run);
      setActiveRunId(admitted.run.run_id);
      const query = new URLSearchParams(location.search);
      query.set("workflow", workflowId);
      query.set("run", admitted.run.run_id);
      history.replaceState(null, "", `/?${query.toString()}`);
      setAction("Queued Run committed. Live progress is labelled separately until its checkpoint is durable.");
    } finally {
      setRunBusy(false);
    }
  }
  async function cancelRun() {
    if (!run || run.durable.terminal) return;
    setRunBusy(true);
    setAction("Committing the Cancellation Request…");
    try {
      const result = await mutateJson<{ accepted: boolean; already_terminal: boolean; run: RunView }>(`/api/v1/runs/${encodeURIComponent(run.run_id)}/cancel`, {
        cancellation_request_id: `cancellation-${crypto.randomUUID()}`,
      });
      setRun(result.run);
      setAction(result.already_terminal ? `Run was already terminal: ${result.run.durable.state}.` : "Cancellation is durable. New work is stopped while known facts settle.");
    } finally {
      setRunBusy(false);
    }
  }
  async function inspectTrace() {
    if (!run) return;
    setRunBusy(true);
    try {
      const evidence = await requestJson<TraceView>(`/api/v1/runs/${encodeURIComponent(run.run_id)}/trace`);
      setTrace(evidence);
      setAction(evidence.integrity_verified ? "Checkpointed Causal Trace verified." : "Causal Trace could not be verified.");
    } finally {
      setRunBusy(false);
    }
  }
  async function inspectArtifact() {
    const artifact = run?.generation?.artifact;
    if (!artifact) return;
    setArtifactBusy(true);
    setArtifactPreview("");
    setAction("Verifying Artifact metadata and a bounded 64 KiB preview…");
    try {
      const metadata = await requestJson<ArtifactView>(`/api/v1/artifacts/${encodeURIComponent(artifact.artifact_id)}`);
      const response = await fetch(`/api/v1/artifacts/${encodeURIComponent(artifact.artifact_id)}/preview?bytes=65536`);
      if (!response.ok) {
        const problem = await response.json() as { code?: string };
        throw new Error(problem.code ?? "Artifact preview failed");
      }
      const bytes = new Uint8Array(await response.arrayBuffer());
      if (bytes.byteLength > 65_536) throw new Error("Artifact preview exceeded its browser bound");
      setArtifactView(metadata);
      setArtifactPreview(new TextDecoder("utf-8", { fatal: false }).decode(bytes));
      setAction(`Artifact verified. Loaded ${bytes.byteLength.toLocaleString()} bytes; the full generated stream stayed unloaded.`);
    } finally {
      setArtifactBusy(false);
    }
  }
  function toggleAcknowledgement(fingerprint: string, checked: boolean) {
    setAcknowledged((current) => checked
      ? [...new Set([...current, fingerprint])]
      : current.filter((item) => item !== fingerprint));
  }

  const canWrite = editing?.role === "holder";
  const hasUnsavedAnnotation = Boolean(draft && annotation !== draft.annotation);
  const queueReady = pendingCount === 0 && saveState !== "saving" && !hasUnsavedAnnotation;
  const requiredAcknowledgements = compilePreview?.diagnostics.filter((item) => item.requires_ack) ?? [];
  const acknowledgementsComplete = requiredAcknowledgements.every((item) => acknowledged.includes(item.fingerprint));
  const previewIsCurrent = Boolean(compilePreview && draft && compilePreview.draft_version === draft.draft_version);
  const canPublish = Boolean(canWrite && queueReady && previewIsCurrent && compilePreview?.can_publish && acknowledgementsComplete && !publicationBusy);
  const newerCount = publication?.revisions.filter((item) => item.is_newer_than_current).length ?? 0;
  return <main class="shell">
    <header class="masthead"><div class="identity"><Mark /><div><p class="eyebrow">Independent automation workspace</p><h1>Canopy Workbench</h1></div></div><span class={`health ${snapshot.readiness?.status === "ready" ? "ready" : "waiting"}`}><span aria-hidden="true" />{snapshot.readiness?.status ?? "connecting"}</span></header>
    <section class={`welcome ${workflowId ? "editor-context" : ""}`} aria-labelledby="welcome-title"><p class="eyebrow">Loss-aware Draft editing</p><h2 id="welcome-title">One writer. Honest recovery.</h2><p>The daemon grants one renewable Draft Lease. Every uncertain command is encrypted locally until acknowledged.</p>
      {!owner ? <form class="owner-login" onSubmit={(event) => { event.preventDefault(); void signIn().catch(showError); }}><label>Owner email<input data-testid="email" type="email" required value={email} onInput={(event) => setEmail(event.currentTarget.value)} /></label><label>Password<input data-testid="password" type="password" required value={password} onInput={(event) => setPassword(event.currentTarget.value)} /></label><button data-testid="sign-in" type="submit">Sign in</button></form> : <button data-testid="logout" type="button" onClick={() => void logout().catch(showError)}>Sign out and clear recovery copies</button>}
      <p class={`action ${saveState}`} data-testid="save-state" data-state={saveState} role="status">{action}</p>
    </section>
    {!workflowId && <section class="cards" aria-label="Installation identity and native catalog"><article><span class="number">01</span><h3>Release</h3><strong>{snapshot.release ? `v${snapshot.release.version}` : "—"}</strong><p>{snapshot.release?.build_commit ?? "Reading build identity…"}</p></article><article><span class="number">02</span><h3>Storage</h3><strong>{snapshot.readiness?.checks.sqlite.journal_mode?.toUpperCase() ?? "—"}</strong><p>Full durability</p></article><article><span class="number">03</span><h3>Resource view</h3><strong>{snapshot.resources?.cpu.available ? "Observed" : "Unavailable"}</strong><p>{snapshot.resources?.memory.available ? "Memory controller visible" : "Reported honestly"}</p></article><article><span class="number">04</span><h3>Native catalog</h3><strong>{snapshot.catalog?.nodes[0]?.display_name ?? "Loading…"}</strong><p>{snapshot.catalog?.nodes[0]?.description ?? "Reading contract metadata…"}</p><button data-testid="create-draft" type="button" disabled={!owner || !snapshot.catalog?.nodes[0] || !editorSessionId} onClick={() => void createManualDraft().catch(showError)}>Add to new Draft</button></article></section>}
    {workflowId && <section class="editor" data-testid="editor" data-workflow-id={workflowId} data-draft-version={draft?.draft_version ?? -1}><div class="editor-head"><div><p class="eyebrow">Mutable Draft</p><h2>{draft?.name ?? workflowId}</h2><code>{workflowId}</code></div><div class={`lease ${editing?.role ?? "waiting"}`} data-testid="lease-role"><strong>{editing?.role === "holder" ? "Lease holder" : editing?.role === "read_only" ? "Read only" : "Lease available"}</strong><span>generation {editing?.lease_generation ?? "—"}</span>{editing?.holder && <small>{editing.holder.label} · expires {new Date(editing.holder.expires_at).toLocaleTimeString()}</small>}</div></div>
      <div class="editor-actions">{editing?.role === "read_only" && !editing.takeover && <button data-testid="request-takeover" onClick={() => void requestTakeover().catch(showError)}>Request takeover</button>}{editing?.role === "read_only" && editing.takeover?.requested_by_me && <button data-testid="claim-takeover" disabled={editing.server_time < editing.takeover.eligible_at} onClick={() => void claimTakeover().catch(showError)}>Claim after grace</button>}{canWrite && editing?.takeover && <><button data-testid="approve-takeover" onClick={() => void respondTakeover(true).catch(showError)}>Approve takeover</button><button onClick={() => void respondTakeover(false).catch(showError)}>Decline</button></>}{canWrite && <button data-testid="release-lease" onClick={() => void releaseLease().catch(showError)}>Release Lease</button>}<button data-testid="refresh-draft" onClick={() => void refreshDraft().catch(showError)}>Refresh Draft</button></div>
      <label class="annotation">Workflow annotation<textarea data-testid="annotation" disabled={!canWrite} value={annotation} onInput={(event) => setAnnotation(event.currentTarget.value)} /></label><div class="editor-actions"><button data-testid="save-annotation" disabled={!canWrite} onClick={() => void saveAnnotation().catch(showError)}>Save annotation</button><button data-testid="undo" disabled={!canWrite} onClick={() => void historyCommand("undo").catch(showError)}>Undo</button><button data-testid="redo" disabled={!canWrite} onClick={() => void historyCommand("redo").catch(showError)}>Redo</button>{pendingCount > 0 && <button data-testid="recover-pending" onClick={() => void recoverPending().catch(showError)}>Reconcile {pendingCount} recovery copy</button>}</div>
      <section class="publication-panel" data-testid="publication-panel" aria-labelledby="publication-title">
        <div class="publication-heading">
          <div><p class="eyebrow">Review · sign · retain</p><h3 id="publication-title">Publication</h3><p>Compile the exact Draft, then pin a signed plan. Rollback moves only the current pointer.</p></div>
          <div class={`publication-difference ${publication?.difference.state ?? "unpublished"}`} data-testid="publication-difference" data-state={publication?.difference.state ?? "unpublished"}>
            <strong>{publication?.difference.state === "matches" ? "Matches published" : publication?.difference.state === "changed" ? "Changed" : "Not published"}</strong>
            <span>{publication?.difference.fields.length ? publication.difference.fields.join(" · ") : "No content differences"}</span>
          </div>
        </div>
        <div class="publication-states" aria-label="Draft and publication states">
          <article><span class="state-symbol" aria-hidden="true">D</span><div><p>Mutable Draft</p><strong>Version {draft?.draft_version ?? "—"}</strong><small>{draft?.nodes.length ?? 0} node · editable history</small></div></article>
          <article data-testid="current-publication"><span class="state-symbol published" aria-hidden="true">P</span><div><p>Current Published</p><strong>{publication?.current_published ? `Revision ${publication.current_published.sequence}` : "None yet"}</strong><small>{publication?.current_published ? shortIdentity(publication.current_published.revision_digest) : "Compile Preview required"}</small></div></article>
          <article data-testid="newer-history"><span class="state-symbol history" aria-hidden="true">H</span><div><p>Retained History</p><strong>{newerCount} newer</strong><small>{publication?.revisions.length ?? 0} immutable revision{publication?.revisions.length === 1 ? "" : "s"}</small></div></article>
        </div>
        {publication?.current_event && <p class="signature-line" data-testid="signature-identity"><span aria-hidden="true">✓</span><strong>{publication.current_event.envelope.kind === "rollback" ? "Signed rollback" : "Published event"}</strong> · Ed25519 verified · <code>{shortIdentity(publication.current_event.signature.key_id)}</code></p>}
        <div class="publication-actions">
          <button data-testid="compile-preview" type="button" disabled={!canWrite || !queueReady || publicationBusy} onClick={() => void runCompilePreview().catch(showError)}>Compile Preview</button>
          <button class="publish-button" data-testid="publish-revision" type="button" disabled={!canPublish} onClick={() => void publishRevision().catch(showError)}>Publish signed Revision</button>
          <p>{!canWrite ? "Read-only tabs cannot compile or publish." : hasUnsavedAnnotation ? "Save the annotation before compiling." : pendingCount > 0 ? `Reconcile ${pendingCount} pending command first.` : compilePreview ? "Preview is bound to this exact Draft Version." : "Nothing is published until you review a full Compile Preview."}</p>
        </div>
        {compilePreview && <section class="compile-result" data-testid="compile-diagnostics" aria-labelledby="diagnostics-title">
          <div class="compile-summary"><div><p class="eyebrow">Compile result</p><h4 id="diagnostics-title">{compilePreview.can_publish ? "Ready after review" : "Blocked by errors"}</h4></div><dl><div><dt>Compiler</dt><dd>{compilePreview.compiler_abi}</dd></div><div><dt>Plan format</dt><dd>{compilePreview.plan_format}</dd></div><div><dt>Input</dt><dd><code>{shortIdentity(compilePreview.compile_input_digest)}</code></dd></div><div><dt>Plan</dt><dd><code>{compilePreview.plan_digest ? shortIdentity(compilePreview.plan_digest) : "not emitted"}</code></dd></div></dl></div>
          <ul class="diagnostic-list">{compilePreview.diagnostics.length ? compilePreview.diagnostics.map((diagnostic) => <li class={diagnostic.severity} key={diagnostic.fingerprint}><div><strong>{diagnostic.code}</strong><span>{diagnostic.severity}</span></div><p>{diagnostic.message}</p><code>{diagnostic.subject}</code>{diagnostic.requires_ack && <label class="warning-ack"><input data-testid={`ack-${diagnostic.code}`} type="checkbox" checked={acknowledged.includes(diagnostic.fingerprint)} onChange={(event) => toggleAcknowledgement(diagnostic.fingerprint, event.currentTarget.checked)} /><span>I reviewed this warning for Draft Version {compilePreview.draft_version}.</span></label>}</li>) : <li class="clean"><strong>No diagnostics</strong><p>The exact input passed all compiler checks.</p></li>}</ul>
        </section>}
        <section class="revision-history" data-testid="revision-history" aria-labelledby="history-title"><div class="history-heading"><div><p class="eyebrow">Append-only record</p><h4 id="history-title">Revision history</h4></div><span>{publication?.revisions.length ?? 0} total</span></div>
          {publication?.revisions.length ? <ol>{[...publication.revisions].reverse().map((revision) => <li key={revision.revision_id}><div><strong>Revision {revision.sequence}</strong><span>{revision.is_current ? "Current" : revision.is_newer_than_current ? "Newer history" : "Preceding"}</span><small>Draft Version {revision.source_draft_version} · <code>{shortIdentity(revision.revision_digest)}</code></small></div>{publication.current_published && revision.sequence < publication.current_published.sequence && <button data-testid={`rollback-revision-${revision.sequence}`} type="button" disabled={!canWrite || !queueReady || publicationBusy} onClick={() => void rollbackRevision(revision).catch(showError)}>Roll back to Revision {revision.sequence}</button>}</li>)}</ol> : <p class="empty-history">No Published Revision yet. The Mutable Draft remains the only state.</p>}
        </section>
      </section>
      <section class="run-panel" data-testid="run-panel" aria-labelledby="run-title">
        <div class="run-heading"><div><p class="eyebrow">Admit · observe · verify</p><h3 id="run-title">Manual Run</h3><p>Run exactly the Published Revision and pinned plan. Live work stays visibly speculative until SQLite commits its checkpoint.</p></div><div class={`stream-state ${streamState}`} data-testid="run-stream-state"><span aria-hidden="true" />SSE {streamState}</div></div>
        <div class="run-actions"><button data-testid="start-run" type="button" disabled={!publication?.current_published || !publication.current_event || runBusy} onClick={() => void startManualRun().catch(showError)}>Start Manual Run</button>{run && !run.durable.terminal && <button class="cancel-button" data-testid="cancel-run" type="button" disabled={runBusy} onClick={() => void cancelRun().catch(showError)}>Cancel durably</button>}{run && <button class="trace-button" data-testid="inspect-trace" type="button" disabled={runBusy} onClick={() => void inspectTrace().catch(showError)}>Inspect Causal Trace</button>}<p>{publication?.current_published ? `Current Revision ${publication.current_published.sequence} is eligible.` : "Publish a Revision before starting a Run."}</p></div>
        {run ? <div class="run-evidence">
          <div class="run-states" aria-label="Run progress states"><article data-testid="run-durable-state" data-state={run.durable.state}><p>Durable state</p><strong>{run.durable.state.replaceAll("_", " ")}</strong><small>Checkpoint {run.durable.checkpoint_sequence} · Logical Order {run.durable.logical_order}</small></article><article data-testid="run-live-state" data-state={run.live?.state ?? "none"}><p>Live state</p><strong>{run.live?.state ?? "No speculative work"}</strong><small>{run.live?.speculative ? "Speculative — replay is allowed" : "No uncommitted claim"}</small></article><article><p>Correctness</p><strong>{run.correctness.complete ? "Complete" : "Incomplete"}</strong><small>{run.correctness.digest ? shortIdentity(run.correctness.digest) : "Digest waits for checkpoint"}</small></article></div>
          {run.generation && <section class="generation-card" data-testid="generation-progress" aria-labelledby="generation-progress-title">
            <div class="generation-summary"><div><p class="eyebrow">Bounded Envelope stream</p><h4 id="generation-progress-title">Generated {run.generation.generated_count.toLocaleString()} items</h4><small>{formatBytes(run.generation.logical_bytes)} logical output · {run.generation.state}</small></div><strong class={run.generation.artifact ? "spill artifact" : "spill inline"}>{run.generation.artifact ? "Encrypted spill" : "Inline data"}</strong></div>
            <progress max={50_000} value={run.generation.generated_count} aria-label={`${run.generation.generated_count.toLocaleString()} of the 50,000 item hard limit generated`} />
            <div class="generation-facts"><span><strong>{run.generation.backpressure_events.toLocaleString()}</strong> backpressure waits</span><span><strong>{run.queue_profile.envelopes.count}</strong> Envelope queue cap</span><span><strong>{formatBytes(run.queue_profile.envelopes.bytes)}</strong> queue byte cap</span><span><strong>{shortIdentity(run.generation.stream_digest)}</strong> stream chain</span></div>
            {run.generation.artifact && <div class="artifact-summary"><div><span class="artifact-mark" aria-hidden="true">A</span><div><strong>Owner-scoped Artifact</strong><small>{formatBytes(run.generation.artifact.logical_bytes)} · {run.generation.artifact.media_type} · opaque identity</small></div></div><button data-testid="load-artifact-preview" type="button" disabled={artifactBusy} onClick={() => void inspectArtifact().catch(showError)}>{artifactBusy ? "Verifying…" : artifactView ? "Refresh 64 KiB preview" : "Verify and preview"}</button></div>}
            {artifactView && <section class="artifact-detail" data-testid="artifact-detail" aria-labelledby="artifact-detail-title"><div><h5 id="artifact-detail-title">Verified Artifact detail</h5><span>{artifactView.chunk_count} authenticated chunk{artifactView.chunk_count === 1 ? "" : "s"}</span></div><dl><div><dt>Artifact</dt><dd><code>{artifactView.reference.artifact_id}</code></dd></div><div><dt>Integrity</dt><dd>{artifactView.integrity_verified ? "Verified before release" : "Not verified"}</dd></div><div><dt>Preview limit</dt><dd>65,536 bytes maximum</dd></div><div><dt>Content digest</dt><dd>{artifactView.reference.content_digest_algorithm}</dd></div></dl><pre tabIndex={0} aria-label="Bounded Artifact preview">{artifactPreview || "(empty Artifact)"}</pre></section>}
          </section>}
          <dl class="run-identity"><div><dt>Run</dt><dd data-testid="run-id"><code>{run.run_id}</code></dd></div><div><dt>Revision</dt><dd><code>{shortIdentity(run.revision_digest)}</code></dd></div><div><dt>Plan</dt><dd><code>{shortIdentity(run.plan_digest)}</code></dd></div><div><dt>Activation counters</dt><dd>{run.correctness.succeeded} succeeded · {run.correctness.cancelled} cancelled · {run.correctness.failed} failed</dd></div></dl>
          {trace && <section class="trace-view" data-testid="causal-trace" aria-labelledby="trace-title"><div class="trace-heading"><div><p class="eyebrow">Hash-chained evidence</p><h4 id="trace-title">Causal Trace</h4></div><strong>{trace.integrity_verified ? "Integrity verified" : "Verification failed"}</strong></div><dl><div><dt>Terminal</dt><dd>{trace.terminal_state}</dd></div><div><dt>Checkpoints</dt><dd data-testid="checkpoint-count">{trace.checkpoints.length}</dd></div><div><dt>Trace events</dt><dd>{trace.events.length}</dd></div><div><dt>Output digest</dt><dd><code>{trace.correctness.digest ? shortIdentity(trace.correctness.digest) : "incomplete"}</code></dd></div></dl>{trace.activations.length ? trace.activations.map((activation) => <article key={activation.activation_id}><div><p>Activation · Logical Order {activation.logical_order}</p><strong data-testid="activation-outcome">{activation.outcome}</strong></div><small><code>{activation.activation_id}</code> · attempt {activation.attempt} · checkpoint {activation.checkpoint_sequence} · {activation.timing.elapsed_micros} µs</small><div class="trace-io"><div><span>Input</span><pre>{JSON.stringify(activation.input, null, 2)}</pre></div><div><span>Output</span><pre>{JSON.stringify(activation.output, null, 2)}</pre></div></div></article>) : <p class="empty-history">No Activation was dispatched before this terminal state.</p>}</section>}
        </div> : <p class="empty-run">No Run admitted yet. Start uses a unique request identity and commits <strong>queued</strong> before success.</p>}
      </section>
      {forks.filter((fork) => fork.status === "open").map((fork) => <article class="recovery-fork" data-testid="recovery-fork" key={fork.fork_id}><h3>Recovery fork</h3><p>Base {fork.diff.base_draft_version} → current {fork.diff.current_draft_version}; authority changed: {String(fork.diff.authority_changed)}</p><pre>{JSON.stringify(fork.pending_operation, null, 2)}</pre><button data-testid="apply-fork" disabled={!canWrite} onClick={() => void applyFork(fork).catch(showError)}>Apply recovered work</button></article>)}
    </section>}
    {snapshot.error && <p role="alert" class="error">Daemon connection failed: {snapshot.error}</p>}<footer>Placeholder identity · independently authored · no third-party editor assets</footer>
  </main>;
}
function tabLabel(id: string): string { return `Editor tab ${id.slice(-6)}`; }
function formatBytes(value: number): string {
  if (value < 1024) return `${value.toLocaleString()} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KiB`;
  return `${(value / (1024 * 1024)).toFixed(2)} MiB`;
}
function shortIdentity(value: string): string {
  const [algorithm, identity = value] = value.split(":", 2);
  return `${algorithm}:${identity.slice(0, 12)}…${identity.slice(-6)}`;
}
render(<App />, document.getElementById("app")!);
