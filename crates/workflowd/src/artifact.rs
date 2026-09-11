// SPDX-License-Identifier: AGPL-3.0-or-later
//! Owner-scoped encrypted content-addressed Artifact storage.

use crate::security::SecurityService;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use zeroize::Zeroizing;

pub const ARTIFACT_FORMAT: &str = "canopy.artifact+xchacha20poly1305/v1alpha1";
pub const CONTENT_DIGEST_ALGORITHM: &str = "blake3-256";
pub const DEDUPE_ALGORITHM: &str = "blake3-keyed-owner-v1";
pub const CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_PREVIEW_BYTES: usize = 64 * 1024;
pub const STAGING_LEASE_MILLIS: i64 = 5 * 60 * 1000;
pub const QUARANTINE_MILLIS: i64 = 24 * 60 * 60 * 1000;
const MAGIC: &[u8; 8] = b"CNPART01";
const NAMESPACE_ID: &str = "owner-1-artifacts-v1";
const NAMESPACE_WRAP_CONTEXT: &str = "canopy:artifact-namespace-seed:v1";
const DEDUPE_CONTEXT: &str = "Canopy Artifact owner deduplication v1alpha1 2026-09-11";
const KEY_WRAP_CONTEXT: &str = "Canopy Artifact data-key wrapping v1alpha1 2026-09-11";

#[derive(Debug, thiserror::Error)]
pub enum ArtifactError {
    #[error("Artifact reference denied")]
    NotAuthorized,
    #[error("Invalid Artifact field: {0}")]
    Invalid(&'static str),
    #[error("Artifact size limit exceeded")]
    TooLarge,
    #[error("Artifact integrity failed: {0}")]
    Integrity(String),
    #[error("Artifact storage unavailable: {0}")]
    Storage(String),
}

#[derive(Clone, Debug)]
pub struct UploadLease {
    lease_id: String,
    staging_name: String,
    logical_bytes: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ArtifactReference {
    pub artifact_id: String,
    pub format: String,
    pub media_type: String,
    pub logical_bytes: u64,
    pub content_digest_algorithm: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ArtifactView {
    pub reference: ArtifactReference,
    pub deduplicated: bool,
    pub chunk_bytes: usize,
    pub chunk_count: u32,
    pub integrity_verified: bool,
}

pub struct ArtifactContentStream {
    file: File,
    cipher: XChaCha20Poly1305,
    artifact_id: String,
    logical_bytes: u64,
    chunk_count: u32,
    nonce_prefix: [u8; 16],
    next_ordinal: u32,
}

impl ArtifactContentStream {
    pub fn next_chunk(&mut self) -> Result<Option<Vec<u8>>, ArtifactError> {
        if self.next_ordinal >= self.chunk_count {
            return Ok(None);
        }
        let ordinal = self.next_ordinal;
        let ciphertext_len = read_u32(&mut self.file)? as usize;
        if ciphertext_len > CHUNK_BYTES + 16 {
            return Err(ArtifactError::Integrity(
                "Artifact stream chunk length failed".into(),
            ));
        }
        let mut ciphertext = vec![0_u8; ciphertext_len];
        self.file.read_exact(&mut ciphertext).map_err(storage)?;
        let nonce = chunk_nonce(&self.nonce_prefix, u64::from(ordinal));
        let aad = chunk_aad(&self.artifact_id, ordinal, self.logical_bytes);
        let plaintext = self
            .cipher
            .decrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &ciphertext,
                    aad: &aad,
                },
            )
            .map_err(|_| {
                ArtifactError::Integrity("Artifact stream authentication failed".into())
            })?;
        let offset = u64::from(ordinal) * CHUNK_BYTES as u64;
        let expected = self
            .logical_bytes
            .saturating_sub(offset)
            .min(CHUNK_BYTES as u64) as usize;
        if plaintext.len() != expected {
            return Err(ArtifactError::Integrity(
                "Artifact stream plaintext length failed".into(),
            ));
        }
        self.next_ordinal += 1;
        Ok(Some(plaintext))
    }
}

#[derive(Serialize, Deserialize)]
struct EncryptedMetadata {
    format: String,
    content_digest_algorithm: String,
    plaintext_digest: String,
    media_type: String,
    logical_bytes: u64,
}

#[derive(Clone)]
struct ArtifactRow {
    artifact_id: String,
    object_name: String,
    logical_bytes: u64,
    chunk_count: u32,
    nonce_prefix: Vec<u8>,
    wrapped_dek_nonce: Vec<u8>,
    wrapped_dek: Vec<u8>,
    metadata_nonce: Vec<u8>,
    metadata_ciphertext: Vec<u8>,
    ciphertext_digest: String,
}

pub struct ArtifactService {
    database: PathBuf,
    objects: PathBuf,
    staging: PathBuf,
    quarantine: PathBuf,
    security: Arc<SecurityService>,
    mutation: Mutex<()>,
    #[cfg(test)]
    read_output_peak: AtomicUsize,
}

impl ArtifactService {
    pub fn initialize(state_dir: &Path, security: Arc<SecurityService>) -> Result<Self, String> {
        let root = state_dir.join("artifacts");
        let service = Self {
            database: state_dir.join("workflow.sqlite3"),
            objects: root.join("objects"),
            staging: root.join("staging"),
            quarantine: root.join("quarantine"),
            security,
            mutation: Mutex::new(()),
            #[cfg(test)]
            read_output_peak: AtomicUsize::new(0),
        };
        for directory in [&service.objects, &service.staging, &service.quarantine] {
            fs::create_dir_all(directory).map_err(|error| error.to_string())?;
        }
        let connection = service.connect()?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS artifact_namespaces(
                    owner_id INTEGER PRIMARY KEY CHECK(owner_id=1),
                    namespace_id TEXT NOT NULL UNIQUE,
                    wrap_algorithm TEXT NOT NULL,
                    wrap_nonce BLOB NOT NULL,
                    wrapped_seed BLOB NOT NULL,
                    created_at INTEGER NOT NULL
                ) STRICT;
                CREATE TABLE IF NOT EXISTS artifacts(
                    artifact_id TEXT PRIMARY KEY,
                    owner_id INTEGER NOT NULL CHECK(owner_id=1),
                    dedupe_algorithm TEXT NOT NULL,
                    dedupe_identity TEXT NOT NULL,
                    object_name TEXT NOT NULL UNIQUE,
                    logical_bytes INTEGER NOT NULL,
                    chunk_count INTEGER NOT NULL,
                    nonce_prefix BLOB NOT NULL,
                    wrapped_dek_nonce BLOB NOT NULL,
                    wrapped_dek BLOB NOT NULL,
                    metadata_nonce BLOB NOT NULL,
                    metadata_ciphertext BLOB NOT NULL,
                    ciphertext_digest_algorithm TEXT NOT NULL,
                    ciphertext_digest TEXT NOT NULL,
                    state TEXT NOT NULL CHECK(state IN ('ready','quarantined')),
                    created_at INTEGER NOT NULL,
                    UNIQUE(owner_id,dedupe_algorithm,dedupe_identity)
                ) STRICT;
                CREATE TABLE IF NOT EXISTS artifact_references(
                    owner_id INTEGER NOT NULL CHECK(owner_id=1),
                    reference_id TEXT NOT NULL,
                    reference_kind TEXT NOT NULL,
                    artifact_id TEXT NOT NULL REFERENCES artifacts(artifact_id),
                    logical_ref_count INTEGER NOT NULL CHECK(logical_ref_count>0),
                    created_at INTEGER NOT NULL,
                    PRIMARY KEY(owner_id,reference_id,artifact_id)
                ) STRICT;
                CREATE INDEX IF NOT EXISTS artifact_references_artifact
                    ON artifact_references(owner_id,artifact_id);
                CREATE TABLE IF NOT EXISTS artifact_leases(
                    lease_id TEXT PRIMARY KEY,
                    staging_name TEXT NOT NULL UNIQUE,
                    logical_bytes INTEGER NOT NULL CHECK(logical_bytes>=0),
                    state TEXT NOT NULL CHECK(state IN ('uploading','finalizing')),
                    expires_at INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    updated_at INTEGER NOT NULL
                ) STRICT;
                CREATE TABLE IF NOT EXISTS artifact_orphans(
                    object_name TEXT PRIMARY KEY,
                    reason TEXT NOT NULL,
                    quarantined_at INTEGER NOT NULL,
                    eligible_after INTEGER NOT NULL
                ) STRICT;
                "#,
            )
            .map_err(|error| error.to_string())?;
        drop(connection);
        service.reconcile_startup()?;
        Ok(service)
    }

    pub fn begin_upload(&self) -> Result<UploadLease, ArtifactError> {
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact upload lock is poisoned".into()))?;
        let lease_id = random_id("artifact-upload");
        let staging_name = format!("{}.upload", random_id("plaintext"));
        let path = self.staging.join(&staging_name);
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(storage)?;
        sync_directory(&self.staging)?;
        let connection = self.connect().map_err(ArtifactError::Storage)?;
        let now = now_millis();
        if let Err(error) = connection.execute(
            "INSERT INTO artifact_leases(lease_id,staging_name,logical_bytes,state,expires_at,created_at,updated_at) VALUES(?1,?2,0,'uploading',?3,?4,?4)",
            params![lease_id, staging_name, now + STAGING_LEASE_MILLIS, now],
        ) {
            let _ = fs::remove_file(path);
            return Err(storage(error));
        }
        Ok(UploadLease {
            lease_id,
            staging_name,
            logical_bytes: 0,
        })
    }

    pub fn append_upload(
        &self,
        lease: &mut UploadLease,
        chunk: &[u8],
    ) -> Result<(), ArtifactError> {
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact upload lock is poisoned".into()))?;
        let next = lease.logical_bytes.saturating_add(chunk.len() as u64);
        if next > MAX_ARTIFACT_BYTES as u64 {
            return Err(ArtifactError::TooLarge);
        }
        let connection = self.connect().map_err(ArtifactError::Storage)?;
        let state: Option<(String, i64)> = connection
            .query_row(
                "SELECT staging_name,expires_at FROM artifact_leases WHERE lease_id=?1 AND state='uploading'",
                params![lease.lease_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(storage)?;
        let now = now_millis();
        if state
            .as_ref()
            .is_none_or(|(name, expires)| name != &lease.staging_name || *expires < now)
        {
            return Err(ArtifactError::Storage(
                "Artifact upload lease expired".into(),
            ));
        }
        let path = self.staging.join(&lease.staging_name);
        let mut file = OpenOptions::new()
            .append(true)
            .open(path)
            .map_err(storage)?;
        file.write_all(chunk).map_err(storage)?;
        file.flush().map_err(storage)?;
        connection
            .execute(
                "UPDATE artifact_leases SET logical_bytes=?2,expires_at=?3,updated_at=?4 WHERE lease_id=?1 AND state='uploading'",
                params![lease.lease_id, next as i64, now + STAGING_LEASE_MILLIS, now],
            )
            .map_err(storage)?;
        lease.logical_bytes = next;
        Ok(())
    }

    pub fn finalize_upload(
        &self,
        lease: UploadLease,
        media_type: &str,
        reference_id: &str,
        reference_kind: &str,
        logical_ref_count: u64,
    ) -> Result<ArtifactView, ArtifactError> {
        let connection = self.connect().map_err(ArtifactError::Storage)?;
        let now = now_millis();
        let changed = connection
            .execute(
                "UPDATE artifact_leases SET state='finalizing',expires_at=?2,updated_at=?3 WHERE lease_id=?1 AND staging_name=?4 AND logical_bytes=?5 AND state='uploading' AND expires_at>=?3",
                params![lease.lease_id, now + STAGING_LEASE_MILLIS, now, lease.staging_name, lease.logical_bytes as i64],
            )
            .map_err(storage)?;
        if changed != 1 {
            return Err(ArtifactError::Storage(
                "Artifact upload lease expired".into(),
            ));
        }
        let path = self.staging.join(&lease.staging_name);
        let result = (|| {
            let mut plaintext = File::open(&path).map_err(storage)?;
            self.put_seekable(
                &mut plaintext,
                lease.logical_bytes,
                media_type,
                reference_id,
                reference_kind,
                logical_ref_count,
            )
        })();
        if result.is_ok() {
            let _guard = self
                .mutation
                .lock()
                .map_err(|_| ArtifactError::Storage("Artifact upload lock is poisoned".into()))?;
            fs::remove_file(&path).map_err(storage)?;
            connection
                .execute(
                    "DELETE FROM artifact_leases WHERE lease_id=?1",
                    params![lease.lease_id],
                )
                .map_err(storage)?;
            sync_directory(&self.staging)?;
        }
        result
    }

    pub fn abandon_upload(&self, lease: UploadLease) {
        let Ok(_guard) = self.mutation.lock() else {
            return;
        };
        if let Ok(connection) = self.connect() {
            let _ = connection.execute(
                "DELETE FROM artifact_leases WHERE lease_id=?1",
                params![lease.lease_id],
            );
        }
        let _ = fs::remove_file(self.staging.join(lease.staging_name));
    }

    pub fn put(
        &self,
        plaintext: &[u8],
        media_type: &str,
        reference_id: &str,
        reference_kind: &str,
        logical_ref_count: u64,
    ) -> Result<ArtifactView, ArtifactError> {
        let logical_bytes = plaintext.len() as u64;
        let mut plaintext = std::io::Cursor::new(plaintext);
        self.put_seekable(
            &mut plaintext,
            logical_bytes,
            media_type,
            reference_id,
            reference_kind,
            logical_ref_count,
        )
    }

    fn put_seekable<R: Read + Seek>(
        &self,
        plaintext: &mut R,
        logical_bytes: u64,
        media_type: &str,
        reference_id: &str,
        reference_kind: &str,
        logical_ref_count: u64,
    ) -> Result<ArtifactView, ArtifactError> {
        if logical_bytes > MAX_ARTIFACT_BYTES as u64 {
            return Err(ArtifactError::TooLarge);
        }
        validate_token(reference_id, "reference_id")?;
        validate_token(reference_kind, "reference_kind")?;
        validate_media_type(media_type)?;
        if logical_ref_count == 0 || logical_ref_count > 50_000 {
            return Err(ArtifactError::Invalid("logical_ref_count"));
        }
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact writer lock is poisoned".into()))?;
        let mut connection = self.connect().map_err(ArtifactError::Storage)?;
        let namespace_seed = self.namespace_seed(&mut connection)?;
        let dedupe_key = blake3::derive_key(DEDUPE_CONTEXT, &namespace_seed);
        let mut dedupe_hasher = blake3::Hasher::new_keyed(&dedupe_key);
        let mut plaintext_hasher = blake3::Hasher::new();
        let mut observed = 0_u64;
        let mut buffer = [0_u8; CHUNK_BYTES];
        loop {
            let read = plaintext.read(&mut buffer).map_err(storage)?;
            if read == 0 {
                break;
            }
            observed = observed.saturating_add(read as u64);
            if observed > logical_bytes || observed > MAX_ARTIFACT_BYTES as u64 {
                return Err(ArtifactError::TooLarge);
            }
            dedupe_hasher.update(&buffer[..read]);
            plaintext_hasher.update(&buffer[..read]);
        }
        if observed != logical_bytes {
            return Err(ArtifactError::Integrity(
                "Artifact upload length changed during finalization".into(),
            ));
        }
        plaintext.seek(SeekFrom::Start(0)).map_err(storage)?;
        let dedupe_identity = dedupe_hasher.finalize().to_hex().to_string();
        let plaintext_digest = plaintext_hasher.finalize().to_hex().to_string();
        if let Some(row) = find_by_dedupe(&connection, &dedupe_identity)? {
            self.commit_reference(
                &mut connection,
                &row.artifact_id,
                reference_id,
                reference_kind,
                logical_ref_count,
            )?;
            return self.view_from_row(&namespace_seed, row, true);
        }

        let artifact_id = random_id("artifact");
        let staging_name = format!("{}.stage", random_id("write"));
        let staging_path = self.staging.join(&staging_name);
        let object_name = format!("{artifact_id}.car");
        let object_path = self.objects.join(&object_name);
        let mut dek = Zeroizing::new([0_u8; 32]);
        OsRng.fill_bytes(dek.as_mut());
        let mut nonce_prefix = [0_u8; 16];
        OsRng.fill_bytes(&mut nonce_prefix);
        let chunk_count = (logical_bytes as usize).div_ceil(CHUNK_BYTES) as u32;
        let ciphertext_digest = write_object_reader(
            &staging_path,
            &artifact_id,
            plaintext,
            logical_bytes,
            &dek,
            &nonce_prefix,
        )?;
        fs::hard_link(&staging_path, &object_path).map_err(|error| {
            ArtifactError::Storage(format!("atomic Artifact placement failed: {error}"))
        })?;
        fs::remove_file(&staging_path).map_err(storage)?;
        sync_directory(&self.objects)?;
        sync_directory(&self.staging)?;

        let wrapping_key = blake3::derive_key(KEY_WRAP_CONTEXT, &namespace_seed);
        let (wrapped_dek_nonce, wrapped_dek) =
            encrypt_secret(&wrapping_key, artifact_id.as_bytes(), dek.as_ref())?;
        let metadata = EncryptedMetadata {
            format: ARTIFACT_FORMAT.into(),
            content_digest_algorithm: CONTENT_DIGEST_ALGORITHM.into(),
            plaintext_digest,
            media_type: media_type.into(),
            logical_bytes,
        };
        let metadata_plaintext = serde_json::to_vec(&metadata)
            .map_err(|error| ArtifactError::Integrity(error.to_string()))?;
        let (metadata_nonce, metadata_ciphertext) = encrypt_secret(
            &wrapping_key,
            format!("metadata:{artifact_id}").as_bytes(),
            &metadata_plaintext,
        )?;
        let transaction = connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(storage)?;
        let insert = transaction.execute(
            "INSERT INTO artifacts(artifact_id,owner_id,dedupe_algorithm,dedupe_identity,object_name,logical_bytes,chunk_count,nonce_prefix,wrapped_dek_nonce,wrapped_dek,metadata_nonce,metadata_ciphertext,ciphertext_digest_algorithm,ciphertext_digest,state,created_at)
             VALUES(?1,1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,'blake3-256',?12,'ready',?13)",
            params![
                artifact_id,
                DEDUPE_ALGORITHM,
                dedupe_identity,
                object_name,
                logical_bytes as i64,
                chunk_count as i64,
                nonce_prefix.as_slice(),
                wrapped_dek_nonce.as_slice(),
                wrapped_dek,
                metadata_nonce.as_slice(),
                metadata_ciphertext,
                ciphertext_digest,
                now_millis()
            ],
        );
        if let Err(error) = insert {
            drop(transaction);
            self.quarantine_file(&object_name, "deduplication_race")?;
            if let Some(row) = find_by_dedupe(&connection, &dedupe_identity)? {
                self.commit_reference(
                    &mut connection,
                    &row.artifact_id,
                    reference_id,
                    reference_kind,
                    logical_ref_count,
                )?;
                return self.view_from_row(&namespace_seed, row, true);
            }
            return Err(storage(error));
        }
        transaction
            .execute(
                "INSERT INTO artifact_references(owner_id,reference_id,reference_kind,artifact_id,logical_ref_count,created_at) VALUES(1,?1,?2,?3,?4,?5)",
                params![reference_id, reference_kind, artifact_id, logical_ref_count as i64, now_millis()],
            )
            .map_err(storage)?;
        transaction.commit().map_err(storage)?;
        let row = load_row(&connection, &artifact_id)?;
        self.view_from_row(&namespace_seed, row, false)
    }

    pub fn metadata(&self, artifact_id: &str) -> Result<ArtifactView, ArtifactError> {
        validate_token(artifact_id, "artifact_id")?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact read lock is poisoned".into()))?;
        let mut connection = self.connect().map_err(ArtifactError::Storage)?;
        require_reference(&connection, artifact_id)?;
        let seed = self.namespace_seed(&mut connection)?;
        let row = load_row(&connection, artifact_id)?;
        let object_name = row.object_name.clone();
        match self.view_from_row(&seed, row, false) {
            Ok(view) => Ok(view),
            Err(error @ ArtifactError::Integrity(_)) => {
                let _ = self.quarantine_file(&object_name, "metadata_integrity_failure");
                Err(error)
            }
            Err(error) => Err(error),
        }
    }

    pub fn preview(
        &self,
        artifact_id: &str,
        maximum: usize,
    ) -> Result<(ArtifactView, Vec<u8>), ArtifactError> {
        if maximum == 0 || maximum > MAX_PREVIEW_BYTES {
            return Err(ArtifactError::Invalid("preview_bytes"));
        }
        let metadata = self.metadata(artifact_id)?;
        if metadata.reference.logical_bytes == 0 {
            return Ok((metadata, Vec::new()));
        }
        let end = (maximum as u64)
            .min(metadata.reference.logical_bytes)
            .saturating_sub(1);
        let (view, bytes, _, _, _) = self.content(artifact_id, Some((0, end)))?;
        Ok((view, bytes))
    }

    pub fn stream_content(
        &self,
        artifact_id: &str,
    ) -> Result<(ArtifactView, ArtifactContentStream), ArtifactError> {
        validate_token(artifact_id, "artifact_id")?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact read lock is poisoned".into()))?;
        let mut connection = self.connect().map_err(ArtifactError::Storage)?;
        require_reference(&connection, artifact_id)?;
        let seed = self.namespace_seed(&mut connection)?;
        let row = load_row(&connection, artifact_id)?;
        let object_name = row.object_name.clone();
        let result = (|| {
            let view = self.view_from_row(&seed, row.clone(), false)?;
            self.read_verified(&seed, &row, None)?;
            let stream = self.open_content_stream(&seed, &row)?;
            Ok((view, stream))
        })();
        match result {
            Err(error @ ArtifactError::Integrity(_)) => {
                let _ = self.quarantine_file(&object_name, "content_integrity_failure");
                Err(error)
            }
            outcome => outcome,
        }
    }

    pub fn content(
        &self,
        artifact_id: &str,
        range: Option<(u64, u64)>,
    ) -> Result<(ArtifactView, Vec<u8>, u64, u64, u64), ArtifactError> {
        validate_token(artifact_id, "artifact_id")?;
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact read lock is poisoned".into()))?;
        let mut connection = self.connect().map_err(ArtifactError::Storage)?;
        require_reference(&connection, artifact_id)?;
        let seed = self.namespace_seed(&mut connection)?;
        let row = load_row(&connection, artifact_id)?;
        let object_name = row.object_name.clone();
        let result = (|| {
            let view = self.view_from_row(&seed, row.clone(), false)?;
            let total = view.reference.logical_bytes;
            let (start, end) = match range {
                Some((start, end)) if total > 0 && start <= end && end < total => (start, end),
                Some(_) => return Err(ArtifactError::Invalid("range")),
                None if total == 0 => (0, 0),
                None => (0, total - 1),
            };
            let read_range = (total > 0).then_some((start, end));
            let bytes = self.read_verified(&seed, &row, read_range)?;
            Ok((view, bytes, start, end, total))
        })();
        match result {
            Err(error @ ArtifactError::Integrity(_)) => {
                let _ = self.quarantine_file(&object_name, "content_integrity_failure");
                Err(error)
            }
            outcome => outcome,
        }
    }

    fn view_from_row(
        &self,
        namespace_seed: &[u8],
        row: ArtifactRow,
        deduplicated: bool,
    ) -> Result<ArtifactView, ArtifactError> {
        let wrapping_key = blake3::derive_key(KEY_WRAP_CONTEXT, namespace_seed);
        let metadata_plaintext = decrypt_secret(
            &wrapping_key,
            format!("metadata:{}", row.artifact_id).as_bytes(),
            &row.metadata_nonce,
            &row.metadata_ciphertext,
        )?;
        let metadata: EncryptedMetadata = serde_json::from_slice(&metadata_plaintext)
            .map_err(|error| ArtifactError::Integrity(error.to_string()))?;
        if metadata.logical_bytes != row.logical_bytes || metadata.format != ARTIFACT_FORMAT {
            return Err(ArtifactError::Integrity(
                "Artifact metadata does not match its durable index".into(),
            ));
        }
        Ok(ArtifactView {
            reference: ArtifactReference {
                artifact_id: row.artifact_id,
                format: metadata.format,
                media_type: metadata.media_type,
                logical_bytes: metadata.logical_bytes,
                content_digest_algorithm: metadata.content_digest_algorithm,
            },
            deduplicated,
            chunk_bytes: CHUNK_BYTES,
            chunk_count: row.chunk_count,
            integrity_verified: true,
        })
    }

    fn read_verified(
        &self,
        namespace_seed: &[u8],
        row: &ArtifactRow,
        range: Option<(u64, u64)>,
    ) -> Result<Vec<u8>, ArtifactError> {
        let path = self.objects.join(&row.object_name);
        let physical = hash_file(&path)?;
        if physical != row.ciphertext_digest {
            return Err(ArtifactError::Integrity(
                "Artifact ciphertext digest failed".into(),
            ));
        }
        let wrapping_key = blake3::derive_key(KEY_WRAP_CONTEXT, namespace_seed);
        let dek = decrypt_secret(
            &wrapping_key,
            row.artifact_id.as_bytes(),
            &row.wrapped_dek_nonce,
            &row.wrapped_dek,
        )?;
        if dek.len() != 32 || row.nonce_prefix.len() != 16 {
            return Err(ArtifactError::Integrity(
                "Artifact key format failed".into(),
            ));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(&dek)
            .map_err(|_| ArtifactError::Integrity("Artifact data key failed".into()))?;
        let mut file = File::open(&path).map_err(storage)?;
        let (logical_bytes, chunk_count, nonce_prefix) = read_header(&mut file)?;
        if logical_bytes != row.logical_bytes
            || chunk_count != row.chunk_count
            || nonce_prefix.as_slice() != row.nonce_prefix
        {
            return Err(ArtifactError::Integrity("Artifact header failed".into()));
        }
        let requested_bytes = range
            .map(|(start, end)| end.saturating_sub(start).saturating_add(1))
            .unwrap_or(0)
            .min(logical_bytes) as usize;
        let mut output = Vec::with_capacity(requested_bytes);
        let mut plaintext_hasher = blake3::Hasher::new();
        for ordinal in 0..chunk_count {
            let ciphertext_len = read_u32(&mut file)? as usize;
            if ciphertext_len > CHUNK_BYTES + 16 {
                return Err(ArtifactError::Integrity(
                    "Artifact chunk length failed".into(),
                ));
            }
            let mut ciphertext = vec![0_u8; ciphertext_len];
            file.read_exact(&mut ciphertext).map_err(storage)?;
            let nonce = chunk_nonce(&nonce_prefix, ordinal as u64);
            let aad = chunk_aad(&row.artifact_id, ordinal, logical_bytes);
            let plaintext = cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    chacha20poly1305::aead::Payload {
                        msg: &ciphertext,
                        aad: &aad,
                    },
                )
                .map_err(|_| ArtifactError::Integrity("Artifact authentication failed".into()))?;
            plaintext_hasher.update(&plaintext);
            let chunk_start = u64::from(ordinal) * CHUNK_BYTES as u64;
            let chunk_end = chunk_start.saturating_add(plaintext.len() as u64);
            if let Some((requested_start, requested_end)) = range {
                let overlap_start = requested_start.max(chunk_start);
                let overlap_end = requested_end.saturating_add(1).min(chunk_end);
                if overlap_start < overlap_end {
                    let local_start = (overlap_start - chunk_start) as usize;
                    let local_end = (overlap_end - chunk_start) as usize;
                    output.extend_from_slice(&plaintext[local_start..local_end]);
                }
            }
            #[cfg(test)]
            self.read_output_peak
                .fetch_max(output.len(), Ordering::Relaxed);
        }
        let metadata_plaintext = decrypt_secret(
            &wrapping_key,
            format!("metadata:{}", row.artifact_id).as_bytes(),
            &row.metadata_nonce,
            &row.metadata_ciphertext,
        )?;
        let metadata: EncryptedMetadata = serde_json::from_slice(&metadata_plaintext)
            .map_err(|error| ArtifactError::Integrity(error.to_string()))?;
        if plaintext_hasher.finalize().to_hex().as_str() != metadata.plaintext_digest {
            return Err(ArtifactError::Integrity(
                "Artifact plaintext digest failed".into(),
            ));
        }
        if output.len() != requested_bytes {
            return Err(ArtifactError::Integrity(
                "Artifact requested range length failed".into(),
            ));
        }
        Ok(output)
    }

    fn open_content_stream(
        &self,
        namespace_seed: &[u8],
        row: &ArtifactRow,
    ) -> Result<ArtifactContentStream, ArtifactError> {
        let wrapping_key = blake3::derive_key(KEY_WRAP_CONTEXT, namespace_seed);
        let dek = decrypt_secret(
            &wrapping_key,
            row.artifact_id.as_bytes(),
            &row.wrapped_dek_nonce,
            &row.wrapped_dek,
        )?;
        if dek.len() != 32 || row.nonce_prefix.len() != 16 {
            return Err(ArtifactError::Integrity(
                "Artifact stream key format failed".into(),
            ));
        }
        let cipher = XChaCha20Poly1305::new_from_slice(&dek)
            .map_err(|_| ArtifactError::Integrity("Artifact stream data key failed".into()))?;
        let mut file = File::open(self.objects.join(&row.object_name)).map_err(storage)?;
        let (logical_bytes, chunk_count, nonce_prefix) = read_header(&mut file)?;
        if logical_bytes != row.logical_bytes
            || chunk_count != row.chunk_count
            || nonce_prefix.as_slice() != row.nonce_prefix
        {
            return Err(ArtifactError::Integrity(
                "Artifact stream header failed".into(),
            ));
        }
        Ok(ArtifactContentStream {
            file,
            cipher,
            artifact_id: row.artifact_id.clone(),
            logical_bytes,
            chunk_count,
            nonce_prefix,
            next_ordinal: 0,
        })
    }

    fn namespace_seed(
        &self,
        connection: &mut Connection,
    ) -> Result<Zeroizing<Vec<u8>>, ArtifactError> {
        let existing = connection
            .query_row(
                "SELECT wrap_nonce,wrapped_seed FROM artifact_namespaces WHERE owner_id=1",
                [],
                |row| Ok((row.get::<_, Vec<u8>>(0)?, row.get::<_, Vec<u8>>(1)?)),
            )
            .optional()
            .map_err(storage)?;
        if let Some((nonce, ciphertext)) = existing {
            return self
                .security
                .unwrap_secret(NAMESPACE_WRAP_CONTEXT, NAMESPACE_ID, &nonce, &ciphertext)
                .map_err(|error| ArtifactError::Storage(format!("namespace key: {error:?}")));
        }
        let mut seed = Zeroizing::new(vec![0_u8; 32]);
        OsRng.fill_bytes(&mut seed);
        let wrapped = self
            .security
            .wrap_secret(NAMESPACE_WRAP_CONTEXT, NAMESPACE_ID, &seed)
            .map_err(|error| ArtifactError::Storage(format!("namespace key: {error:?}")))?;
        connection
            .execute(
                "INSERT INTO artifact_namespaces(owner_id,namespace_id,wrap_algorithm,wrap_nonce,wrapped_seed,created_at) VALUES(1,?1,'xchacha20poly1305',?2,?3,?4)",
                params![NAMESPACE_ID, wrapped.nonce.as_slice(), wrapped.ciphertext, now_millis()],
            )
            .map_err(storage)?;
        Ok(seed)
    }

    fn commit_reference(
        &self,
        connection: &mut Connection,
        artifact_id: &str,
        reference_id: &str,
        reference_kind: &str,
        logical_ref_count: u64,
    ) -> Result<(), ArtifactError> {
        connection
            .execute(
                "INSERT INTO artifact_references(owner_id,reference_id,reference_kind,artifact_id,logical_ref_count,created_at)
                 VALUES(1,?1,?2,?3,?4,?5)
                 ON CONFLICT(owner_id,reference_id,artifact_id) DO UPDATE SET logical_ref_count=MAX(logical_ref_count,excluded.logical_ref_count)",
                params![reference_id, reference_kind, artifact_id, logical_ref_count as i64, now_millis()],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn reconcile_startup(&self) -> Result<(), String> {
        let now = now_millis();
        let connection = self.connect()?;
        // Startup runs before admission and before any Artifact writer can exist. Every
        // surviving staging file therefore belongs to an interrupted old operation.
        let mut moved_staging = false;
        for entry in fs::read_dir(&self.staging).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
            {
                let name = entry.file_name().to_string_lossy().to_string();
                let target = self.quarantine.join(&name);
                fs::rename(entry.path(), &target).map_err(|error| error.to_string())?;
                moved_staging = true;
                connection
                    .execute(
                        "INSERT OR REPLACE INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,'startup_interrupted_staging',?2,?3)",
                        params![name, now, now + QUARANTINE_MILLIS],
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        if moved_staging {
            sync_directory(&self.staging).map_err(|error| format!("{error:?}"))?;
            sync_directory(&self.quarantine).map_err(|error| format!("{error:?}"))?;
        }
        connection
            .execute("DELETE FROM artifact_leases", [])
            .map_err(|error| error.to_string())?;
        let mut known = std::collections::HashSet::new();
        let mut statement = connection
            .prepare("SELECT object_name FROM artifacts")
            .map_err(|error| error.to_string())?;
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| error.to_string())?;
        for row in rows {
            known.insert(row.map_err(|error| error.to_string())?);
        }
        drop(statement);
        let mut moved_objects = false;
        for entry in fs::read_dir(&self.objects).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
                && !known.contains(&name)
            {
                let target = self.quarantine.join(&name);
                fs::rename(entry.path(), target).map_err(|error| error.to_string())?;
                moved_objects = true;
                connection
                    .execute(
                        "INSERT OR REPLACE INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,'startup_unreferenced',?2,?3)",
                        params![name, now, now + QUARANTINE_MILLIS],
                    )
                    .map_err(|error| error.to_string())?;
            }
        }
        if moved_objects {
            sync_directory(&self.objects).map_err(|error| format!("{error:?}"))?;
            sync_directory(&self.quarantine).map_err(|error| format!("{error:?}"))?;
        }
        for entry in fs::read_dir(&self.quarantine).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            if !entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_file()
            {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let indexed: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM artifact_orphans WHERE object_name=?1)",
                    params![name],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            if indexed {
                continue;
            }
            let suspect: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM artifacts WHERE object_name=?1 AND state='quarantined')",
                    params![name],
                    |row| row.get(0),
                )
                .map_err(|error| error.to_string())?;
            connection
                .execute(
                    "INSERT INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,?2,?3,?4)",
                    params![
                        name,
                        if suspect {
                            "startup_uncertain_integrity"
                        } else {
                            "startup_unindexed_quarantine"
                        },
                        now,
                        if suspect { i64::MAX } else { now + QUARANTINE_MILLIS }
                    ],
                )
                .map_err(|error| error.to_string())?;
        }
        self.cleanup_eligible().map_err(|error| error.to_string())?;
        Ok(())
    }

    pub fn cleanup_eligible(&self) -> Result<u64, ArtifactError> {
        let _guard = self
            .mutation
            .lock()
            .map_err(|_| ArtifactError::Storage("Artifact maintenance lock is poisoned".into()))?;
        let mut connection = self.connect().map_err(ArtifactError::Storage)?;
        let now = now_millis();
        let expired_leases = {
            let mut statement = connection
                .prepare(
                    "SELECT lease_id,staging_name FROM artifact_leases WHERE expires_at<?1 ORDER BY lease_id",
                )
                .map_err(storage)?;
            let leases = statement
                .query_map(params![now], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(storage)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(storage)?;
            drop(statement);
            leases
        };
        for (lease_id, name) in expired_leases {
            let source = self.staging.join(&name);
            let target = self.quarantine.join(&name);
            let moved = source.exists();
            if moved {
                fs::rename(&source, &target).map_err(storage)?;
                sync_directory(&self.staging)?;
                sync_directory(&self.quarantine)?;
            }
            let transaction = connection
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .map_err(storage)?;
            if moved {
                transaction
                    .execute(
                        "INSERT OR REPLACE INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,'expired_staging_lease',?2,?3)",
                        params![name, now, now + QUARANTINE_MILLIS],
                    )
                    .map_err(storage)?;
            }
            transaction
                .execute(
                    "DELETE FROM artifact_leases WHERE lease_id=?1 AND expires_at<?2",
                    params![lease_id, now],
                )
                .map_err(storage)?;
            transaction.commit().map_err(storage)?;
        }
        let mut statement = connection
            .prepare(
                "SELECT o.object_name FROM artifact_orphans o
                 LEFT JOIN artifacts a ON a.object_name=o.object_name
                 LEFT JOIN artifact_references r ON r.artifact_id=a.artifact_id
                 WHERE o.eligible_after<=?1 AND o.reason NOT LIKE '%integrity%'
                 GROUP BY o.object_name HAVING COUNT(r.artifact_id)=0",
            )
            .map_err(storage)?;
        let names = statement
            .query_map(params![now], |row| row.get::<_, String>(0))
            .map_err(storage)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(storage)?;
        drop(statement);
        let mut removed = 0_u64;
        for name in names {
            let path = self.quarantine.join(&name);
            if path.exists() {
                fs::remove_file(path).map_err(storage)?;
            }
            connection
                .execute(
                    "DELETE FROM artifact_orphans WHERE object_name=?1",
                    params![name],
                )
                .map_err(storage)?;
            removed += 1;
        }
        if removed > 0 {
            sync_directory(&self.quarantine)?;
        }
        Ok(removed)
    }

    fn quarantine_file(&self, object_name: &str, reason: &str) -> Result<(), ArtifactError> {
        let source = self.objects.join(object_name);
        let target = self.quarantine.join(object_name);
        if source.exists() {
            fs::rename(source, target).map_err(storage)?;
            sync_directory(&self.objects)?;
            sync_directory(&self.quarantine)?;
        }
        let connection = self.connect().map_err(ArtifactError::Storage)?;
        let now = now_millis();
        let corruption_evidence = reason.contains("integrity");
        connection
            .execute(
                "UPDATE artifacts SET state='quarantined' WHERE owner_id=1 AND object_name=?1",
                params![object_name],
            )
            .map_err(storage)?;
        connection
            .execute(
                "INSERT OR REPLACE INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,?2,?3,?4)",
                params![object_name, reason, now, if corruption_evidence { i64::MAX } else { now + 24 * 60 * 60 * 1000_i64 }],
            )
            .map_err(storage)?;
        Ok(())
    }

    fn connect(&self) -> Result<Connection, String> {
        let connection = Connection::open(&self.database).map_err(|error| error.to_string())?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "journal_mode", "WAL")
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "synchronous", "FULL")
            .map_err(|error| error.to_string())?;
        connection
            .pragma_update(None, "foreign_keys", true)
            .map_err(|error| error.to_string())?;
        Ok(connection)
    }
}

fn write_object_reader<R: Read>(
    path: &Path,
    artifact_id: &str,
    plaintext: &mut R,
    logical_bytes: u64,
    dek: &[u8; 32],
    nonce_prefix: &[u8; 16],
) -> Result<String, ArtifactError> {
    let cipher = XChaCha20Poly1305::new_from_slice(dek)
        .map_err(|_| ArtifactError::Integrity("Artifact data key failed".into()))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(storage)?;
    let chunk_count = (logical_bytes as usize).div_ceil(CHUNK_BYTES) as u32;
    file.write_all(MAGIC).map_err(storage)?;
    file.write_all(&(CHUNK_BYTES as u32).to_be_bytes())
        .map_err(storage)?;
    file.write_all(&logical_bytes.to_be_bytes())
        .map_err(storage)?;
    file.write_all(&chunk_count.to_be_bytes())
        .map_err(storage)?;
    file.write_all(nonce_prefix).map_err(storage)?;
    let mut remaining = logical_bytes as usize;
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    for ordinal in 0..chunk_count {
        let chunk_len = remaining.min(CHUNK_BYTES);
        plaintext
            .read_exact(&mut chunk[..chunk_len])
            .map_err(storage)?;
        remaining -= chunk_len;
        let nonce = chunk_nonce(nonce_prefix, ordinal as u64);
        let aad = chunk_aad(artifact_id, ordinal, logical_bytes);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: &chunk[..chunk_len],
                    aad: &aad,
                },
            )
            .map_err(|_| ArtifactError::Integrity("Artifact encryption failed".into()))?;
        file.write_all(&(ciphertext.len() as u32).to_be_bytes())
            .map_err(storage)?;
        file.write_all(&ciphertext).map_err(storage)?;
    }
    if remaining != 0 {
        return Err(ArtifactError::Integrity(
            "Artifact plaintext was truncated during encryption".into(),
        ));
    }
    file.sync_all().map_err(storage)?;
    drop(file);
    hash_file(path)
}

fn read_header(file: &mut File) -> Result<(u64, u32, [u8; 16]), ArtifactError> {
    file.seek(SeekFrom::Start(0)).map_err(storage)?;
    let mut magic = [0_u8; 8];
    file.read_exact(&mut magic).map_err(storage)?;
    if &magic != MAGIC || read_u32(file)? as usize != CHUNK_BYTES {
        return Err(ArtifactError::Integrity(
            "Artifact format header failed".into(),
        ));
    }
    let mut length = [0_u8; 8];
    file.read_exact(&mut length).map_err(storage)?;
    let logical_bytes = u64::from_be_bytes(length);
    let chunk_count = read_u32(file)?;
    let mut nonce_prefix = [0_u8; 16];
    file.read_exact(&mut nonce_prefix).map_err(storage)?;
    if logical_bytes > MAX_ARTIFACT_BYTES as u64
        || chunk_count != (logical_bytes as usize).div_ceil(CHUNK_BYTES) as u32
    {
        return Err(ArtifactError::Integrity(
            "Artifact bounds header failed".into(),
        ));
    }
    Ok((logical_bytes, chunk_count, nonce_prefix))
}

fn read_u32(file: &mut File) -> Result<u32, ArtifactError> {
    let mut bytes = [0_u8; 4];
    file.read_exact(&mut bytes).map_err(storage)?;
    Ok(u32::from_be_bytes(bytes))
}

fn chunk_nonce(prefix: &[u8; 16], ordinal: u64) -> [u8; 24] {
    let mut nonce = [0_u8; 24];
    nonce[..16].copy_from_slice(prefix);
    nonce[16..].copy_from_slice(&ordinal.to_be_bytes());
    nonce
}

fn chunk_aad(artifact_id: &str, ordinal: u32, logical_bytes: u64) -> Vec<u8> {
    format!("canopy:artifact-chunk:v1\0owner:1\0{artifact_id}\0{ordinal}\0{logical_bytes}")
        .into_bytes()
}

fn encrypt_secret(
    key: &[u8; 32],
    aad: &[u8],
    plaintext: &[u8],
) -> Result<([u8; 24], Vec<u8>), ArtifactError> {
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| ArtifactError::Integrity("Artifact wrapping key failed".into()))?;
    let mut nonce = [0_u8; 24];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| ArtifactError::Integrity("Artifact secret encryption failed".into()))?;
    Ok((nonce, ciphertext))
}

fn decrypt_secret(
    key: &[u8; 32],
    aad: &[u8],
    nonce: &[u8],
    ciphertext: &[u8],
) -> Result<Zeroizing<Vec<u8>>, ArtifactError> {
    if nonce.len() != 24 {
        return Err(ArtifactError::Integrity("Artifact nonce failed".into()));
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key)
        .map_err(|_| ArtifactError::Integrity("Artifact wrapping key failed".into()))?;
    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            chacha20poly1305::aead::Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map(Zeroizing::new)
        .map_err(|_| ArtifactError::Integrity("Artifact secret authentication failed".into()))
}

fn find_by_dedupe(
    connection: &Connection,
    dedupe_identity: &str,
) -> Result<Option<ArtifactRow>, ArtifactError> {
    connection
        .query_row(
            "SELECT artifact_id,object_name,logical_bytes,chunk_count,nonce_prefix,wrapped_dek_nonce,wrapped_dek,metadata_nonce,metadata_ciphertext,ciphertext_digest FROM artifacts WHERE owner_id=1 AND dedupe_algorithm=?1 AND dedupe_identity=?2 AND state='ready'",
            params![DEDUPE_ALGORITHM, dedupe_identity],
            row_from_sql,
        )
        .optional()
        .map_err(storage)
}

fn load_row(connection: &Connection, artifact_id: &str) -> Result<ArtifactRow, ArtifactError> {
    connection
        .query_row(
            "SELECT artifact_id,object_name,logical_bytes,chunk_count,nonce_prefix,wrapped_dek_nonce,wrapped_dek,metadata_nonce,metadata_ciphertext,ciphertext_digest FROM artifacts WHERE owner_id=1 AND artifact_id=?1 AND state='ready'",
            params![artifact_id],
            row_from_sql,
        )
        .map_err(|error| {
            if matches!(error, rusqlite::Error::QueryReturnedNoRows) {
                ArtifactError::NotAuthorized
            } else {
                storage(error)
            }
        })
}

fn row_from_sql(row: &rusqlite::Row<'_>) -> rusqlite::Result<ArtifactRow> {
    Ok(ArtifactRow {
        artifact_id: row.get(0)?,
        object_name: row.get(1)?,
        logical_bytes: row.get::<_, i64>(2)? as u64,
        chunk_count: row.get::<_, i64>(3)? as u32,
        nonce_prefix: row.get(4)?,
        wrapped_dek_nonce: row.get(5)?,
        wrapped_dek: row.get(6)?,
        metadata_nonce: row.get(7)?,
        metadata_ciphertext: row.get(8)?,
        ciphertext_digest: row.get(9)?,
    })
}

fn require_reference(connection: &Connection, artifact_id: &str) -> Result<(), ArtifactError> {
    let exists: i64 = connection
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM artifact_references WHERE owner_id=1 AND artifact_id=?1)",
            params![artifact_id],
            |row| row.get(0),
        )
        .map_err(storage)?;
    if exists == 1 {
        Ok(())
    } else {
        Err(ArtifactError::NotAuthorized)
    }
}

fn hash_file(path: &Path) -> Result<String, ArtifactError> {
    let mut file = File::open(path).map_err(storage)?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = [0_u8; CHUNK_BYTES];
    loop {
        let read = file.read(&mut buffer).map_err(storage)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hasher.finalize().to_hex().to_string())
}

fn sync_directory(path: &Path) -> Result<(), ArtifactError> {
    File::open(path)
        .map_err(storage)?
        .sync_all()
        .map_err(storage)
}

fn validate_token(value: &str, field: &'static str) -> Result<(), ArtifactError> {
    if !value.is_empty()
        && value.len() <= 160
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_.:".contains(&byte))
    {
        Ok(())
    } else {
        Err(ArtifactError::Invalid(field))
    }
}

fn validate_media_type(value: &str) -> Result<(), ArtifactError> {
    if !value.is_empty()
        && value.len() <= 128
        && value.is_ascii()
        && !value.bytes().any(|byte| byte.is_ascii_control())
    {
        Ok(())
    } else {
        Err(ArtifactError::Invalid("media_type"))
    }
}

fn random_id(prefix: &str) -> String {
    let mut raw = [0_u8; 16];
    OsRng.fill_bytes(&mut raw);
    format!("{prefix}-{}", URL_SAFE_NO_PAD.encode(raw))
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn storage(error: impl std::fmt::Display) -> ArtifactError {
    ArtifactError::Storage(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn service(label: &str) -> (PathBuf, ArtifactService) {
        let root =
            std::env::temp_dir().join(format!("canopy-artifact-{label}-{}", random_id("test")));
        fs::create_dir_all(&root).unwrap();
        let security = Arc::new(SecurityService::initialize_for_test(&root));
        let service = ArtifactService::initialize(&root, security).unwrap();
        (root, service)
    }

    #[test]
    fn blake3_owner_keying_is_distinct_and_measured() {
        let chunk = [0x5a_u8; CHUNK_BYTES];
        let key = blake3::derive_key(DEDUPE_CONTEXT, &[0x33; 32]);
        let mut keyed = blake3::Hasher::new_keyed(&key);
        let mut physical = blake3::Hasher::new();
        let started = std::time::Instant::now();
        for _ in 0..256 {
            keyed.update(&chunk);
            physical.update(&chunk);
        }
        let elapsed = started.elapsed();
        assert_ne!(keyed.finalize(), physical.finalize());
        assert!(elapsed < Duration::from_secs(5));
        let hashed_bytes = 2 * 256 * CHUNK_BYTES;
        let mib_per_second = hashed_bytes as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);
        println!(
            "blake3-measure=passed hashed_bytes={hashed_bytes} elapsed_micros={} mib_per_second={mib_per_second:.2}",
            elapsed.as_micros()
        );
    }

    #[test]
    fn owner_deduplicates_and_verified_ranges_are_bounded() {
        let (root, service) = service("dedupe");
        let payload = vec![b'x'; CHUNK_BYTES + 91];
        let first = service
            .put(&payload, "application/octet-stream", "ref-one", "test", 1)
            .unwrap();
        let second = service
            .put(&payload, "application/octet-stream", "ref-two", "test", 1)
            .unwrap();
        assert_eq!(first.reference.artifact_id, second.reference.artifact_id);
        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        let (_, preview) = service.preview(&first.reference.artifact_id, 64).unwrap();
        assert_eq!(preview, vec![b'x'; 64]);
        service.read_output_peak.store(0, Ordering::Relaxed);
        let (_, range, start, end, total) = service
            .content(&first.reference.artifact_id, Some((7, 23)))
            .unwrap();
        assert_eq!((start, end, total), (7, 23, payload.len() as u64));
        assert_eq!(range, vec![b'x'; 17]);
        assert!(service.read_output_peak.load(Ordering::Relaxed) <= range.len());
        service.read_output_peak.store(0, Ordering::Relaxed);
        let (_, mut stream) = service
            .stream_content(&first.reference.artifact_id)
            .unwrap();
        assert_eq!(service.read_output_peak.load(Ordering::Relaxed), 0);
        let mut streamed = Vec::new();
        while let Some(chunk) = stream.next_chunk().unwrap() {
            assert!(chunk.len() <= CHUNK_BYTES);
            streamed.extend_from_slice(&chunk);
        }
        assert_eq!(streamed, payload);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_reference_is_a_non_disclosing_denial() {
        let (root, service) = service("denial");
        let view = service
            .put(b"private", "text/plain", "ref-private", "test", 1)
            .unwrap();
        let connection = service.connect().unwrap();
        connection
            .execute(
                "DELETE FROM artifact_references WHERE artifact_id=?1",
                params![view.reference.artifact_id],
            )
            .unwrap();
        assert!(matches!(
            service.metadata(&view.reference.artifact_id),
            Err(ArtifactError::NotAuthorized)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn ciphertext_damage_is_quarantined_before_plaintext_release() {
        let (root, service) = service("integrity");
        let view = service
            .put(
                b"integrity evidence",
                "text/plain",
                "ref-integrity",
                "test",
                1,
            )
            .unwrap();
        let connection = service.connect().unwrap();
        let object_name: String = connection
            .query_row(
                "SELECT object_name FROM artifacts WHERE artifact_id=?1",
                params![view.reference.artifact_id],
                |row| row.get(0),
            )
            .unwrap();
        let path = service.objects.join(&object_name);
        let mut bytes = fs::read(&path).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x80;
        fs::write(&path, bytes).unwrap();
        assert!(matches!(
            service.content(&view.reference.artifact_id, None),
            Err(ArtifactError::Integrity(_))
        ));
        assert!(!path.exists());
        assert!(service.quarantine.join(&object_name).exists());
        let connection = service.connect().unwrap();
        connection
            .execute(
                "UPDATE artifact_orphans SET eligible_after=0 WHERE object_name=?1",
                params![object_name],
            )
            .unwrap();
        drop(connection);
        assert_eq!(service.cleanup_eligible().unwrap(), 0);
        assert!(service.quarantine.join(object_name).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interrupted_streaming_upload_is_quarantined_for_at_least_a_day() {
        let (root, service) = service("staging");
        let mut lease = service.begin_upload().unwrap();
        service.append_upload(&mut lease, b"unfinished").unwrap();
        let staging_name = lease.staging_name.clone();
        drop(service);
        let security = Arc::new(SecurityService::initialize_for_test(&root));
        let restarted = ArtifactService::initialize(&root, security).unwrap();
        assert!(!restarted.staging.join(&staging_name).exists());
        assert!(restarted.quarantine.join(&staging_name).exists());
        let connection = restarted.connect().unwrap();
        let (quarantined_at, eligible_after): (i64, i64) = connection
            .query_row(
                "SELECT quarantined_at,eligible_after FROM artifact_orphans WHERE object_name=?1",
                params![staging_name],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(eligible_after - quarantined_at >= QUARANTINE_MILLIS);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_cleanup_quarantines_only_expired_staging_and_removes_it_after_safety_age() {
        let (root, service) = service("runtime-cleanup");
        let mut expired = service.begin_upload().unwrap();
        service
            .append_upload(&mut expired, b"expired staging")
            .unwrap();
        let expired_name = expired.staging_name.clone();
        let mut active = service.begin_upload().unwrap();
        service
            .append_upload(&mut active, b"active staging")
            .unwrap();
        let active_name = active.staging_name.clone();
        let connection = service.connect().unwrap();
        connection
            .execute(
                "UPDATE artifact_leases SET expires_at=0 WHERE lease_id=?1",
                params![expired.lease_id],
            )
            .unwrap();
        drop(connection);

        assert_eq!(service.cleanup_eligible().unwrap(), 0);
        assert!(!service.staging.join(&expired_name).exists());
        assert!(service.quarantine.join(&expired_name).exists());
        assert!(service.staging.join(&active_name).exists());

        let connection = service.connect().unwrap();
        connection
            .execute(
                "UPDATE artifact_orphans SET eligible_after=0 WHERE object_name=?1",
                params![expired_name],
            )
            .unwrap();
        drop(connection);
        assert_eq!(service.cleanup_eligible().unwrap(), 1);
        assert!(!service.quarantine.join(expired_name).exists());
        assert!(service.staging.join(active_name).exists());
        service.abandon_upload(active);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_and_integrity_quarantine_share_the_mutation_barrier() {
        let (root, service) = service("read-barrier");
        let view = service
            .put(b"barrier", "text/plain", "barrier-ref", "test", 1)
            .unwrap();
        let artifact_id = view.reference.artifact_id;
        let service = Arc::new(service);
        let guard = service.mutation.lock().unwrap();
        let reader = Arc::clone(&service);
        let (sender, receiver) = std::sync::mpsc::channel();
        let handle = std::thread::spawn(move || {
            sender.send(reader.metadata(&artifact_id)).unwrap();
        });
        assert!(receiver.recv_timeout(Duration::from_millis(100)).is_err());
        drop(guard);
        assert!(receiver
            .recv_timeout(Duration::from_secs(2))
            .unwrap()
            .is_ok());
        handle.join().unwrap();
        drop(service);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cleanup_never_removes_a_live_referenced_object() {
        let (root, service) = service("live-cleanup");
        let view = service
            .put(b"live", "text/plain", "live-ref", "test", 1)
            .unwrap();
        let connection = service.connect().unwrap();
        let object_name: String = connection
            .query_row(
                "SELECT object_name FROM artifacts WHERE artifact_id=?1",
                params![view.reference.artifact_id],
                |row| row.get(0),
            )
            .unwrap();
        connection.execute(
            "INSERT INTO artifact_orphans(object_name,reason,quarantined_at,eligible_after) VALUES(?1,'injected_safe_orphan',0,0)",
            params![object_name],
        ).unwrap();
        drop(connection);

        assert_eq!(service.cleanup_eligible().unwrap(), 0);
        assert!(service.objects.join(object_name).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn streaming_finalize_never_needs_the_whole_request_body() {
        let (root, service) = service("stream");
        let mut lease = service.begin_upload().unwrap();
        let mut expected = Vec::new();
        for ordinal in 0..5_u8 {
            let chunk = vec![ordinal; CHUNK_BYTES];
            expected.extend_from_slice(&chunk);
            service.append_upload(&mut lease, &chunk).unwrap();
        }
        let view = service
            .finalize_upload(lease, "application/octet-stream", "stream-ref", "test", 1)
            .unwrap();
        let (_, actual, _, _, _) = service.content(&view.reference.artifact_id, None).unwrap();
        assert_eq!(actual, expected);
        fs::remove_dir_all(root).unwrap();
    }
}
