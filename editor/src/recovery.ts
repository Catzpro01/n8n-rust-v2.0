// SPDX-License-Identifier: AGPL-3.0-or-later

export type RecoveryCommand = {
  editor_session_id: string;
  lease_generation: number;
  command_id: string;
  base_draft_version: number;
  operation: Record<string, unknown>;
};
export type RecoveryCopy = {
  recovery_copy_id: string;
  workflow_id: string;
  editor_session_id: string;
  expires_at: number;
  command: RecoveryCommand;
};
type StoredCopy = {
  id: string;
  workflow_id: string;
  editor_session_id: string;
  expires_at: number;
  iv: ArrayBuffer;
  ciphertext: ArrayBuffer;
};

const DATABASE = "canopy-recovery-v1";
const KEY_ID = "local-aes-gcm-v1";
const encoder = new TextEncoder();
const decoder = new TextDecoder();

export async function saveRecoveryCopy(copy: RecoveryCopy): Promise<void> {
  await purgeExpired(Date.now());
  const key = await recoveryKey();
  const iv = crypto.getRandomValues(new Uint8Array(new ArrayBuffer(12)));
  const ciphertext = await crypto.subtle.encrypt(
    { name: "AES-GCM", iv, additionalData: aad(copy.recovery_copy_id, copy.workflow_id) },
    key,
    encoder.encode(JSON.stringify(copy.command)),
  );
  const database = await openDatabase();
  await transactionDone(database, "copies", "readwrite", (store) => store.put({
    id: copy.recovery_copy_id,
    workflow_id: copy.workflow_id,
    editor_session_id: copy.editor_session_id,
    expires_at: copy.expires_at,
    iv: iv.buffer,
    ciphertext,
  } satisfies StoredCopy));
  database.close();
}

export async function loadRecoveryCopies(workflowId: string): Promise<RecoveryCopy[]> {
  await purgeExpired(Date.now());
  const database = await openDatabase();
  const records = await request<StoredCopy[]>(database.transaction("copies", "readonly").objectStore("copies").getAll());
  database.close();
  const selected = records.filter((record) => record.workflow_id === workflowId);
  if (selected.length === 0) return [];
  const key = await existingRecoveryKey();
  if (!key) {
    await clearRecoveryCopies();
    return [];
  }
  const copies: RecoveryCopy[] = [];
  for (const record of selected) {
    const plaintext = await crypto.subtle.decrypt(
      { name: "AES-GCM", iv: record.iv, additionalData: aad(record.id, record.workflow_id) },
      key,
      record.ciphertext,
    );
    copies.push({
      recovery_copy_id: record.id,
      workflow_id: record.workflow_id,
      editor_session_id: record.editor_session_id,
      expires_at: record.expires_at,
      command: JSON.parse(decoder.decode(plaintext)) as RecoveryCommand,
    });
  }
  return copies.sort((left, right) => left.expires_at - right.expires_at);
}

export async function deleteRecoveryCopy(id: string): Promise<void> {
  const database = await openDatabase();
  await transactionDone(database, "copies", "readwrite", (store) => store.delete(id));
  database.close();
}

export async function purgeExpired(now: number): Promise<void> {
  const database = await openDatabase();
  const transaction = database.transaction("copies", "readwrite");
  const store = transaction.objectStore("copies");
  const records = await request<StoredCopy[]>(store.getAll());
  for (const record of records) if (record.expires_at <= now) store.delete(record.id);
  await complete(transaction);
  database.close();
}

export async function clearRecoveryCopies(): Promise<void> {
  const database = await openDatabase();
  const transaction = database.transaction(["copies", "keys"], "readwrite");
  transaction.objectStore("copies").clear();
  transaction.objectStore("keys").clear();
  await complete(transaction);
  database.close();
}

async function recoveryKey(): Promise<CryptoKey> {
  const existing = await existingRecoveryKey();
  if (existing) return existing;
  const key = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  const database = await openDatabase();
  await transactionDone(database, "keys", "readwrite", (store) => store.put({ id: KEY_ID, key }));
  database.close();
  return key;
}

async function existingRecoveryKey(): Promise<CryptoKey | undefined> {
  const database = await openDatabase();
  const row = await request<{ id: string; key: CryptoKey } | undefined>(
    database.transaction("keys", "readonly").objectStore("keys").get(KEY_ID),
  );
  database.close();
  return row?.key;
}

function aad(copyId: string, workflowId: string): ArrayBuffer {
  const bytes = encoder.encode(`canopy:recovery-copy:v1\0${workflowId}\0${copyId}`);
  return bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer;
}
function openDatabase(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const opening = indexedDB.open(DATABASE, 1);
    opening.onupgradeneeded = () => {
      const database = opening.result;
      database.createObjectStore("keys", { keyPath: "id" });
      database.createObjectStore("copies", { keyPath: "id" });
    };
    opening.onsuccess = () => resolve(opening.result);
    opening.onerror = () => reject(opening.error ?? new Error("Recovery storage could not open"));
  });
}
function request<T>(value: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    value.onsuccess = () => resolve(value.result);
    value.onerror = () => reject(value.error ?? new Error("Recovery storage request failed"));
  });
}
function complete(transaction: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error ?? new Error("Recovery storage transaction failed"));
    transaction.onabort = () => reject(transaction.error ?? new Error("Recovery storage transaction aborted"));
  });
}
async function transactionDone(
  database: IDBDatabase,
  storeName: string,
  mode: IDBTransactionMode,
  action: (store: IDBObjectStore) => IDBRequest,
): Promise<void> {
  const transaction = database.transaction(storeName, mode);
  action(transaction.objectStore(storeName));
  await complete(transaction);
}
