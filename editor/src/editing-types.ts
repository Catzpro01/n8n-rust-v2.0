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
