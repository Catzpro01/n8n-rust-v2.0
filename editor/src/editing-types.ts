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
    event_id: string;
    event_sequence: number;
    previous_revision_id?: string;
    target_revision_id: string;
    revision_digest: string;
    plan_digest: string;
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
export type ArtifactReference = {
  artifact_id: string;
  format: string;
  media_type: string;
  logical_bytes: number;
  content_digest_algorithm: string;
};
export type ArtifactView = {
  reference: ArtifactReference;
  deduplicated: boolean;
  chunk_bytes: number;
  chunk_count: number;
  integrity_verified: boolean;
};
export type RunView = {
  schema: string;
  run_id: string;
  run_request_id: string;
  workflow_id: string;
  publication_event_id: string;
  revision_id: string;
  revision_digest: string;
  plan_id: string;
  plan_digest: string;
  durable: { state: "queued" | "cancel_requested" | "suspended" | "succeeded" | "failed" | "cancelled"; checkpoint_sequence: number; logical_order: number; terminal: boolean; updated_at: number };
  live?: { state: string; speculative: boolean; boot_epoch: string; sequence: number };
  correctness: { canonicalization: string; algorithm: string; digest?: string; complete: boolean; attempted: number; succeeded: number; cancelled: number; failed: number; output_count: number };
  generation?: { state: "running" | "suspended" | "succeeded" | "failed" | "cancelled"; generated_count: number; logical_bytes: number; stream_digest: string; backpressure_events: number; artifact?: ArtifactReference };
  admitted_at: number;
  started_at?: number;
  terminal_at?: number;
  queue_profile: { profile: string; maximum_inline_invocation_bytes: number; envelopes: { count: number; bytes: number } };
};
export type TraceView = {
  schema: string;
  terminal_state: string;
  integrity_verified: boolean;
  run: { run_id: string; workflow_id: string; publication_event_id: string; revision_id: string; revision_digest: string; plan_id: string; plan_digest: string };
  correctness: RunView["correctness"];
  checkpoints: Array<{ sequence: number; state: string; logical_order: number; checkpoint_hash: string; trace_head_hash: string; committed_at: number }>;
  activations: Array<{ activation_id: string; node_instance_id: string; logical_order: number; attempt: number; outcome: string; input: unknown; output?: unknown; input_digest: string; output_digest?: string; checkpoint_sequence: number; timing: { started_at: number; completed_at: number; elapsed_micros: number } }>;
  events: Array<{ event_sequence: number; logical_order?: number; checkpoint_sequence: number; phase: string; event_type: string; event_hash: string }>;
  safe_resource_facts: Record<string, unknown>;
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
