// SPDX-License-Identifier: AGPL-3.0-or-later

export type ContractLock = { api_version: string; namespace: string; name: string; version: string; digest: string };
export type Catalog = { nodes: Array<{ display_name: string; description: string; contract_lock: ContractLock; configuration_schema: unknown; editor_hints: unknown }> };
export type WorkflowDraft = { workflow_id: string; name: string; draft_version: number; annotation: string; nodes: unknown[] };
export type EditingStatus = {
  workflow_id: string;
  role: "holder" | "read_only" | "available";
  lease_generation: number;
  holder?: { label: string; expires_at: number };
  takeover?: { request_id: string; state: "pending"; requester_label: string; eligible_at: number; requested_by_me: boolean };
  server_time: number;
};
export type RecoveryFork = {
  fork_id: string;
  workflow_id: string;
  status: "open" | "applied";
  original_command_id: string;
  pending_operation: Record<string, unknown>;
  diff: { base_draft_version: number; current_draft_version: number; authority_changed: boolean; pending_operation: Record<string, unknown> };
};
export type CompileDiagnostic = {
  code: string;
  severity: "error" | "warning";
  require_acknowledgement: boolean;
  path: string;
  message: string;
};
export type CompilePreview = {
  workflow_id: string;
  draft_version: number;
  status: "ok" | "warnings" | "failed";
  diagnostics: CompileDiagnostic[];
  plan?: { plan_format: string; compiler_algorithm: string; plan_digest: string; node_count: number; connection_count: number };
};
export type RevisionSummary = {
  revision_number: number;
  status: "current" | "superseded";
  draft_version: number;
  document_digest: string;
  plan_digest: string;
  plan_format: string;
  compiler_algorithm: string;
  signature: string;
  published_at: number;
};
export type PublicationView = {
  workflow_id: string;
  current_revision: number | null;
  revisions: RevisionSummary[];
};
export type PublishedRevision = {
  workflow_id: string;
  revision_number: number;
  document_digest: string;
  plan_digest: string;
  plan_format: string;
  compiler_algorithm: string;
  signature: string;
  published_at: number;
  evidence: { acknowledged_warnings: string[]; diagnostics: CompileDiagnostic[] };
};
export type DraftDiffView = {
  workflow_id: string;
  draft_version: number;
  published_revision: number | null;
  nodes: {
    added: Array<{ id: string; name: string }>;
    removed: Array<{ id: string; name: string }>;
    modified: Array<{ id: string; name: string; changed: string[] }>;
  };
  connections: { added: string[]; removed: string[] };
  workflow_fields: { name: boolean; annotation: boolean; settings: boolean; compatibility_metadata: boolean };
};
