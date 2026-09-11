// SPDX-License-Identifier: AGPL-3.0-or-later

export type ContractLock = { api_version: string; namespace: string; name: string; version: string; digest: string };
export type Catalog = { nodes: Array<{ display_name: string; description: string; contract_lock: ContractLock; configuration_schema: unknown; editor_hints: unknown }> };
export type WorkflowDraft = { workflow_id: string; name: string; draft_version: number; annotation: string; nodes: unknown[] };
export type CompileDiagnostic = {
  code: string;
  severity: "error" | "warning" | "info";
  subject: string;
  message: string;
  arguments: Record<string, unknown>;
  requires_ack: boolean;
  fingerprint: string;
};
export type CompilePreview = {
  workflow_id: string;
  draft_version: number;
  canonicalization: "jcs-rfc8785";
  digest_algorithm: "sha256";
  compiler_abi: string;
  plan_format: string;
  compile_input_digest: string;
  revision_digest: string;
  plan_digest?: string;
  compatibility_profile: { profile_id: string; status: string };
  contract_locks: ContractLock[];
  diagnostics: CompileDiagnostic[];
  can_publish: boolean;
};
export type RevisionSummary = {
  revision_id: string;
  sequence: number;
  source_draft_version: number;
  revision_digest: string;
  plan_digest: string;
  is_current: boolean;
  is_newer_than_current: boolean;
};
export type SignedPublicationEvent = {
  envelope: {
    kind: "publication" | "rollback";
    event_sequence: number;
    previous_revision_id?: string;
    target_revision_id: string;
    signature_identity: { algorithm: string; key_id: string; public_key: string };
  };
  signature: { algorithm: string; canonicalization: string; key_id: string; public_key: string; value: string };
};
export type PublicationStatus = {
  workflow_id: string;
  mutable_draft: { draft_version: number; node_count: number };
  current_published?: RevisionSummary;
  latest_published?: RevisionSummary;
  current_event?: SignedPublicationEvent;
  revisions: RevisionSummary[];
  difference: { state: "unpublished" | "matches" | "changed"; fields: string[] };
};
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
