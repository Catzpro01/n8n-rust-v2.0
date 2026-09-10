// SPDX-License-Identifier: AGPL-3.0-or-later

export type OwnerSession = { csrf_token: string; expires_at: number };
const AUTH_KEY = "canopy-owner-session-v1";
const EDITOR_KEY = "canopy-editor-session-v1";

export function loadOwnerSession(): OwnerSession | undefined {
  try {
    const value = JSON.parse(localStorage.getItem(AUTH_KEY) ?? "null") as OwnerSession | null;
    if (value && value.expires_at * 1000 > Date.now() && value.csrf_token) return value;
  } catch { /* invalid local state is discarded */ }
  localStorage.removeItem(AUTH_KEY);
  return undefined;
}
export function saveOwnerSession(value: OwnerSession): void {
  localStorage.setItem(AUTH_KEY, JSON.stringify(value));
}
export function clearOwnerSession(): void { localStorage.removeItem(AUTH_KEY); }

export async function claimEditorSession(): Promise<{ id: string; close: () => void }> {
  let id = sessionStorage.getItem(EDITOR_KEY) || crypto.randomUUID();
  sessionStorage.setItem(EDITOR_KEY, id);
  if (!("BroadcastChannel" in window)) return { id, close: () => undefined };
  const channel = new BroadcastChannel("canopy-editor-sessions-v1");
  let occupied = false;
  channel.onmessage = (event: MessageEvent<{ kind: string; id: string }>) => {
    if (event.data.id !== id) return;
    if (event.data.kind === "probe") channel.postMessage({ kind: "occupied", id });
    if (event.data.kind === "occupied") occupied = true;
  };
  channel.postMessage({ kind: "probe", id });
  await new Promise((resolve) => setTimeout(resolve, 60));
  if (occupied) {
    id = crypto.randomUUID();
    sessionStorage.setItem(EDITOR_KEY, id);
  }
  channel.postMessage({ kind: "claimed", id });
  return { id, close: () => channel.close() };
}
