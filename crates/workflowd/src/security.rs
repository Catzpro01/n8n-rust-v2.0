// SPDX-License-Identifier: AGPL-3.0-or-later
use crate::{config::ServeConfig, error::AppError};
use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Algorithm, Argon2, Params, Version,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    XChaCha20Poly1305, XNonce,
};
use rand_core::{OsRng, RngCore};
use rusqlite::{limits::Limit, params, Connection};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const VAULT_PLAINTEXT: &[u8] = b"canopy-vault-root-v1";
const VAULT_AAD: &[u8] = b"canopy:vault-metadata:v1";

#[derive(Debug)]
pub enum SecurityError {
    SetupClosed,
    InvalidInput(&'static str),
    InvalidCredentials,
    LoginLimited,
    Unauthorized,
    Csrf,
    RecoveryChecksum,
    Internal(String),
}
#[derive(Clone, Serialize)]
pub struct RecoveryHealth {
    pub state: &'static str,
    pub disaster_recovery_ready: bool,
}
#[derive(Serialize)]
pub struct SetupResult {
    pub owner: OwnerView,
    pub recovery_kit: RecoveryKit,
}
#[derive(Serialize)]
pub struct OwnerView {
    pub id: &'static str,
    pub email: String,
    pub password_kdf: PasswordKdf,
}
#[derive(Serialize)]
pub struct PasswordKdf {
    pub algorithm: &'static str,
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    pub measured_millis: u128,
}
#[derive(Serialize)]
pub struct RecoveryKit {
    pub document: String,
    pub checksum: String,
    pub acknowledgement: &'static str,
}
#[derive(Serialize)]
pub struct SessionGrant {
    #[serde(skip)]
    pub token: String,
    pub csrf_token: String,
    pub expires_at: i64,
}
#[derive(Clone)]
pub struct OwnerActor {
    pub id: i64,
}
#[derive(Serialize)]
pub struct AuditView {
    pub occurred_at: i64,
    pub actor_id: String,
    pub action: String,
    pub outcome: String,
}

pub struct WrappedPublicationSeed {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

pub struct WrappedSecret {
    pub nonce: [u8; 24],
    pub ciphertext: Vec<u8>,
}

pub struct SecurityService {
    database: PathBuf,
    master_key: Option<Zeroizing<[u8; 32]>>,
    origin: String,
    ttl: i64,
    max_failures: i64,
    argon_memory: u32,
    argon_iterations: u32,
    recovery: AtomicU8,
}

impl SecurityService {
    #[cfg(test)]
    pub(crate) fn initialize_for_test(state: &Path) -> Self {
        Self {
            database: state.join("workflow.sqlite3"),
            master_key: Some(Zeroizing::new([0x5a; 32])),
            origin: "http://127.0.0.1:8787".into(),
            ttl: 3600,
            max_failures: 3,
            argon_memory: 8192,
            argon_iterations: 1,
            recovery: AtomicU8::new(1),
        }
    }

    pub fn initialize(state: &Path, config: &ServeConfig) -> Result<Self, AppError> {
        let database = state.join("workflow.sqlite3");
        let master_key = read_master_key(config.master_key_file.as_deref())?;
        let service = Self {
            database,
            master_key,
            origin: config.control_origin.clone(),
            ttl: config.session_ttl_seconds,
            max_failures: config.login_max_failures,
            argon_memory: config.argon_memory_kib,
            argon_iterations: config.argon_iterations,
            recovery: AtomicU8::new(0),
        };
        let connection = service
            .connect()
            .map_err(|e| AppError::Security(format!("security schema: {e:?}")))?;
        connection
            .execute_batch(
                r#"
                CREATE TABLE IF NOT EXISTS owners (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    email TEXT NOT NULL UNIQUE,
                    password_phc TEXT NOT NULL,
                    failed_logins INTEGER NOT NULL DEFAULT 0,
                    locked_until INTEGER NOT NULL DEFAULT 0,
                    created_at INTEGER NOT NULL
                ) STRICT;
                CREATE TABLE IF NOT EXISTS owner_sessions (
                    token_hash BLOB PRIMARY KEY,
                    csrf_hash BLOB NOT NULL,
                    expires_at INTEGER NOT NULL,
                    created_at INTEGER NOT NULL,
                    owner_id INTEGER NOT NULL REFERENCES owners(id)
                ) STRICT;
                CREATE TABLE IF NOT EXISTS vault_metadata (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    key_fingerprint BLOB NOT NULL,
                    nonce BLOB NOT NULL,
                    ciphertext BLOB NOT NULL
                ) STRICT;
                CREATE TABLE IF NOT EXISTS recovery_status (
                    id INTEGER PRIMARY KEY CHECK(id = 1),
                    kit_checksum TEXT NOT NULL,
                    acknowledged_at INTEGER
                ) STRICT;
                CREATE TABLE IF NOT EXISTS owner_audit (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    occurred_at INTEGER NOT NULL,
                    actor_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    outcome TEXT NOT NULL
                ) STRICT;
                "#,
            )
            .map_err(internal)?;
        let owner_count: i64 = connection
            .query_row("SELECT count(*) FROM owners", [], |r| r.get(0))
            .map_err(internal)?;
        if owner_count == 0 {
            service.recovery.store(0, Ordering::Relaxed);
        } else {
            let key = service.master_key.as_ref().ok_or_else(|| {
                AppError::Security("master key is required for initialized state".into())
            })?;
            validate_vault(&connection, key)
                .map_err(|e| AppError::Security(format!("vault key rejected: {e:?}")))?;
            let acknowledged: Option<i64> = connection
                .query_row(
                    "SELECT acknowledged_at FROM recovery_status WHERE id=1",
                    [],
                    |r| r.get(0),
                )
                .map_err(internal)?;
            service.recovery.store(
                if acknowledged.is_some() { 2 } else { 1 },
                Ordering::Relaxed,
            );
        }
        Ok(service)
    }
    pub fn origin(&self) -> &str {
        &self.origin
    }
    pub fn recovery_health(&self) -> RecoveryHealth {
        match self.recovery.load(Ordering::Relaxed) {
            0 => RecoveryHealth {
                state: "setup-required",
                disaster_recovery_ready: false,
            },
            1 => RecoveryHealth {
                state: "recovery-kit-unacknowledged",
                disaster_recovery_ready: false,
            },
            _ => RecoveryHealth {
                state: "local-recovery-only",
                disaster_recovery_ready: false,
            },
        }
    }
    pub fn setup(
        &self,
        email: &str,
        password: &str,
        recovery_passphrase: &str,
    ) -> Result<SetupResult, SecurityError> {
        let email = email.trim().to_ascii_lowercase();
        if !email.contains('@') {
            return Err(SecurityError::InvalidInput("email"));
        }
        if password.len() < 12 {
            return Err(SecurityError::InvalidInput("password"));
        }
        if recovery_passphrase.len() < 16 {
            return Err(SecurityError::InvalidInput("recovery_passphrase"));
        }
        let master = self
            .master_key
            .as_ref()
            .ok_or(SecurityError::Internal("master key is unavailable".into()))?;
        let started = std::time::Instant::now();
        let password_phc = self.hash_password(password)?;
        let measured = started.elapsed().as_millis();
        let (document, checksum) = make_recovery_kit(
            master,
            recovery_passphrase,
            self.argon_memory,
            self.argon_iterations,
        )?;
        let fingerprint = key_fingerprint(master);
        let mut nonce = [0u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new_from_slice(master.as_ref())
            .map_err(|_| SecurityError::Internal("cipher initialization".into()))?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: VAULT_PLAINTEXT,
                    aad: VAULT_AAD,
                },
            )
            .map_err(|_| SecurityError::Internal("vault encryption".into()))?;
        let mut c = self.connect()?;
        let tx = c.transaction().map_err(sec_internal)?;
        let count: i64 = tx
            .query_row("SELECT count(*) FROM owners", [], |r| r.get(0))
            .map_err(sec_internal)?;
        if count != 0 {
            return Err(SecurityError::SetupClosed);
        }
        let now = now();
        tx.execute(
            "INSERT INTO owners(id,email,password_phc,created_at) VALUES(1,?1,?2,?3)",
            params![email, password_phc, now],
        )
        .map_err(sec_internal)?;
        tx.execute(
            "INSERT INTO vault_metadata(id,key_fingerprint,nonce,ciphertext) VALUES(1,?1,?2,?3)",
            params![fingerprint.as_slice(), nonce.as_slice(), ciphertext],
        )
        .map_err(sec_internal)?;
        tx.execute(
            "INSERT INTO recovery_status(id,kit_checksum) VALUES(1,?1)",
            params![checksum],
        )
        .map_err(sec_internal)?;
        audit(&tx, "owner:1", "owner.setup", "succeeded")?;
        tx.commit().map_err(sec_internal)?;
        self.recovery.store(1, Ordering::Relaxed);
        Ok(SetupResult{owner:OwnerView{id:"owner:1",email,password_kdf:PasswordKdf{algorithm:"argon2id",memory_kib:self.argon_memory,iterations:self.argon_iterations,parallelism:1,measured_millis:measured}},recovery_kit:RecoveryKit{document,checksum,acknowledgement:"Store this encrypted document outside the server, then acknowledge its checksum."}})
    }
    pub fn login(&self, email: &str, password: &str) -> Result<SessionGrant, SecurityError> {
        let c = self.connect()?;
        let row = c
            .query_row(
                "SELECT id,password_phc,failed_logins,locked_until FROM owners WHERE email=?1",
                params![email.trim().to_ascii_lowercase()],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .map_err(|e| {
                if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
                    SecurityError::InvalidCredentials
                } else {
                    sec_internal(e)
                }
            })?;
        if row.3 > now() {
            return Err(SecurityError::LoginLimited);
        }
        let parsed = PasswordHash::new(&row.1)
            .map_err(|_| SecurityError::Internal("stored password hash is invalid".into()))?;
        if self
            .argon()
            .verify_password(password.as_bytes(), &parsed)
            .is_err()
        {
            let failures = row.2 + 1;
            let locked = if failures >= self.max_failures {
                now() + 60
            } else {
                0
            };
            c.execute(
                "UPDATE owners SET failed_logins=?1,locked_until=?2 WHERE id=?3",
                params![failures, locked, row.0],
            )
            .map_err(sec_internal)?;
            audit(&c, "owner:1", "session.login", "rejected")?;
            return Err(if locked > 0 {
                SecurityError::LoginLimited
            } else {
                SecurityError::InvalidCredentials
            });
        }
        c.execute(
            "UPDATE owners SET failed_logins=0,locked_until=0 WHERE id=?1",
            params![row.0],
        )
        .map_err(sec_internal)?;
        let marker = format!("m={},t={},p=1", self.argon_memory, self.argon_iterations);
        if !row.1.contains(&marker) {
            let upgraded = self.hash_password(password)?;
            c.execute(
                "UPDATE owners SET password_phc=?1 WHERE id=?2",
                params![upgraded, row.0],
            )
            .map_err(sec_internal)?;
            audit(&c, "owner:1", "owner.password_hash_upgraded", "succeeded")?;
        }
        let grant = create_session(&c, row.0, self.ttl)?;
        audit(&c, "owner:1", "session.login", "succeeded")?;
        Ok(grant)
    }
    pub fn authenticate(&self, token: &str) -> Result<OwnerActor, SecurityError> {
        let c = self.connect()?;
        authenticate(&c, token)
    }
    pub fn require_csrf(&self, token: &str, csrf: &str) -> Result<OwnerActor, SecurityError> {
        let c = self.connect()?;
        let actor = authenticate(&c, token)?;
        let expected: Vec<u8> = c
            .query_row(
                "SELECT csrf_hash FROM owner_sessions WHERE token_hash=?1",
                params![token_hash(token).as_slice()],
                |r| r.get(0),
            )
            .map_err(sec_internal)?;
        let actual = token_hash(csrf);
        if expected.as_slice().ct_eq(actual.as_slice()).unwrap_u8() != 1 {
            return Err(SecurityError::Csrf);
        }
        Ok(actor)
    }
    pub fn renew(&self, token: &str, csrf: &str) -> Result<SessionGrant, SecurityError> {
        let actor = self.require_csrf(token, csrf)?;
        let c = self.connect()?;
        c.execute(
            "DELETE FROM owner_sessions WHERE token_hash=?1",
            params![token_hash(token).as_slice()],
        )
        .map_err(sec_internal)?;
        let grant = create_session(&c, actor.id, self.ttl)?;
        audit(&c, "owner:1", "session.renew", "succeeded")?;
        Ok(grant)
    }
    pub fn logout(&self, token: &str, csrf: &str) -> Result<(), SecurityError> {
        self.require_csrf(token, csrf)?;
        let c = self.connect()?;
        c.execute(
            "DELETE FROM owner_sessions WHERE token_hash=?1",
            params![token_hash(token).as_slice()],
        )
        .map_err(sec_internal)?;
        audit(&c, "owner:1", "session.logout", "succeeded")
    }
    pub fn acknowledge(
        &self,
        token: &str,
        csrf: &str,
        checksum: &str,
    ) -> Result<(), SecurityError> {
        self.require_csrf(token, csrf)?;
        let c = self.connect()?;
        let changed = c
            .execute(
                "UPDATE recovery_status SET acknowledged_at=?1 WHERE id=1 AND kit_checksum=?2",
                params![now(), checksum],
            )
            .map_err(sec_internal)?;
        if changed != 1 {
            return Err(SecurityError::RecoveryChecksum);
        }
        audit(&c, "owner:1", "recovery_kit.acknowledge", "succeeded")?;
        self.recovery.store(2, Ordering::Relaxed);
        Ok(())
    }
    pub fn audit(&self, token: &str) -> Result<Vec<AuditView>, SecurityError> {
        self.authenticate(token)?;
        let c = self.connect()?;
        let mut s = c
            .prepare("SELECT occurred_at,actor_id,action,outcome FROM owner_audit ORDER BY id")
            .map_err(sec_internal)?;
        let events = s
            .query_map([], |r| {
                Ok(AuditView {
                    occurred_at: r.get(0)?,
                    actor_id: r.get(1)?,
                    action: r.get(2)?,
                    outcome: r.get(3)?,
                })
            })
            .map_err(sec_internal)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(sec_internal)?;
        Ok(events)
    }
    pub fn wrap_secret(
        &self,
        context: &str,
        identity: &str,
        plaintext: &[u8],
    ) -> Result<WrappedSecret, SecurityError> {
        if self.recovery.load(Ordering::Relaxed) == 0 {
            return Err(SecurityError::Unauthorized);
        }
        let master = self
            .master_key
            .as_ref()
            .ok_or_else(|| SecurityError::Internal("master key is unavailable".into()))?;
        let mut nonce = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new_from_slice(master.as_ref())
            .map_err(|_| SecurityError::Internal("secret wrapping cipher".into()))?;
        let aad = format!("canopy:wrapped-secret:v1\0{context}\0{identity}");
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: plaintext,
                    aad: aad.as_bytes(),
                },
            )
            .map_err(|_| SecurityError::Internal("secret wrapping failed".into()))?;
        Ok(WrappedSecret { nonce, ciphertext })
    }

    pub fn unwrap_secret(
        &self,
        context: &str,
        identity: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<Vec<u8>>, SecurityError> {
        if nonce.len() != 24 {
            return Err(SecurityError::Internal(
                "secret wrapping nonce has invalid length".into(),
            ));
        }
        let master = self
            .master_key
            .as_ref()
            .ok_or_else(|| SecurityError::Internal("master key is unavailable".into()))?;
        let cipher = XChaCha20Poly1305::new_from_slice(master.as_ref())
            .map_err(|_| SecurityError::Internal("secret wrapping cipher".into()))?;
        let aad = format!("canopy:wrapped-secret:v1\0{context}\0{identity}");
        cipher
            .decrypt(
                XNonce::from_slice(nonce),
                chacha20poly1305::aead::Payload {
                    msg: ciphertext,
                    aad: aad.as_bytes(),
                },
            )
            .map(Zeroizing::new)
            .map_err(|_| SecurityError::Internal("secret unwrapping failed".into()))
    }

    pub fn wrap_publication_seed(
        &self,
        key_id: &str,
        seed: &[u8; 32],
    ) -> Result<WrappedPublicationSeed, SecurityError> {
        if self.recovery.load(Ordering::Relaxed) == 0 {
            return Err(SecurityError::Unauthorized);
        }
        let master = self
            .master_key
            .as_ref()
            .ok_or_else(|| SecurityError::Internal("master key is unavailable".into()))?;
        let mut nonce = [0_u8; 24];
        OsRng.fill_bytes(&mut nonce);
        let cipher = XChaCha20Poly1305::new_from_slice(master.as_ref())
            .map_err(|_| SecurityError::Internal("publication wrapping cipher".into()))?;
        let aad = publication_key_aad(key_id);
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                chacha20poly1305::aead::Payload {
                    msg: seed,
                    aad: &aad,
                },
            )
            .map_err(|_| SecurityError::Internal("publication key wrapping failed".into()))?;
        Ok(WrappedPublicationSeed { nonce, ciphertext })
    }
    pub fn unwrap_publication_seed(
        &self,
        key_id: &str,
        nonce: &[u8],
        ciphertext: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, SecurityError> {
        if nonce.len() != 24 {
            return Err(SecurityError::Internal(
                "publication key wrapping nonce has invalid length".into(),
            ));
        }
        let master = self
            .master_key
            .as_ref()
            .ok_or_else(|| SecurityError::Internal("master key is unavailable".into()))?;
        let cipher = XChaCha20Poly1305::new_from_slice(master.as_ref())
            .map_err(|_| SecurityError::Internal("publication wrapping cipher".into()))?;
        let aad = publication_key_aad(key_id);
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(nonce),
                    chacha20poly1305::aead::Payload {
                        msg: ciphertext,
                        aad: &aad,
                    },
                )
                .map_err(|_| SecurityError::Internal("publication key unwrapping failed".into()))?,
        );
        if plaintext.len() != 32 {
            return Err(SecurityError::Internal(
                "publication signing seed has invalid length".into(),
            ));
        }
        let mut seed = Zeroizing::new([0_u8; 32]);
        seed.copy_from_slice(&plaintext);
        Ok(seed)
    }
    fn argon(&self) -> Argon2<'static> {
        Argon2::new(
            Algorithm::Argon2id,
            Version::V0x13,
            Params::new(self.argon_memory, self.argon_iterations, 1, Some(32))
                .expect("validated argon parameters"),
        )
    }
    fn hash_password(&self, password: &str) -> Result<String, SecurityError> {
        let salt = SaltString::generate(&mut OsRng);
        self.argon()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(|_| SecurityError::Internal("password hashing failed".into()))
    }
    fn connect(&self) -> Result<Connection, SecurityError> {
        let c = Connection::open(&self.database).map_err(sec_internal)?;
        c.busy_timeout(std::time::Duration::from_secs(5))
            .map_err(sec_internal)?;
        c.pragma_update(None, "journal_mode", "WAL")
            .map_err(sec_internal)?;
        c.pragma_update(None, "synchronous", "FULL")
            .map_err(sec_internal)?;
        c.pragma_update(None, "foreign_keys", true)
            .map_err(sec_internal)?;
        c.set_limit(Limit::SQLITE_LIMIT_LENGTH, 16 * 1024 * 1024)
            .map_err(sec_internal)?;
        c.set_limit(Limit::SQLITE_LIMIT_SQL_LENGTH, 1024 * 1024)
            .map_err(sec_internal)?;
        c.set_limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER, 999)
            .map_err(sec_internal)?;
        let synchronous: i64 = c
            .query_row("PRAGMA synchronous", [], |row| row.get(0))
            .map_err(sec_internal)?;
        let foreign_keys: i64 = c
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .map_err(sec_internal)?;
        if synchronous != 2
            || foreign_keys != 1
            || c.limit(Limit::SQLITE_LIMIT_LENGTH).map_err(sec_internal)? != 16 * 1024 * 1024
            || c.limit(Limit::SQLITE_LIMIT_SQL_LENGTH)
                .map_err(sec_internal)?
                != 1024 * 1024
            || c.limit(Limit::SQLITE_LIMIT_VARIABLE_NUMBER)
                .map_err(sec_internal)?
                != 999
        {
            return Err(SecurityError::Internal(
                "SQLite policy read-back failed".into(),
            ));
        }
        Ok(c)
    }
}
fn read_master_key(path: Option<&Path>) -> Result<Option<Zeroizing<[u8; 32]>>, AppError> {
    let Some(path) = path else { return Ok(None) };
    let bytes = Zeroizing::new(
        fs::read(path).map_err(|e| AppError::Security(format!("cannot read master key: {e}")))?,
    );
    if bytes.len() != 32 {
        return Err(AppError::Security(
            "master key must contain exactly 32 bytes".into(),
        ));
    }
    let mut key = [0u8; 32];
    key.copy_from_slice(&bytes);
    Ok(Some(Zeroizing::new(key)))
}
fn validate_vault(c: &Connection, key: &[u8; 32]) -> Result<(), SecurityError> {
    let (fingerprint, nonce, ciphertext): (Vec<u8>, Vec<u8>, Vec<u8>) = c
        .query_row(
            "SELECT key_fingerprint,nonce,ciphertext FROM vault_metadata WHERE id=1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(sec_internal)?;
    if fingerprint
        .as_slice()
        .ct_eq(key_fingerprint(key).as_slice())
        .unwrap_u8()
        != 1
    {
        return Err(SecurityError::Unauthorized);
    }
    let cipher = XChaCha20Poly1305::new_from_slice(key).map_err(|_| SecurityError::Unauthorized)?;
    let plain = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            chacha20poly1305::aead::Payload {
                msg: &ciphertext,
                aad: VAULT_AAD,
            },
        )
        .map_err(|_| SecurityError::Unauthorized)?;
    if plain != VAULT_PLAINTEXT {
        return Err(SecurityError::Unauthorized);
    }
    Ok(())
}
fn make_recovery_kit(
    master: &[u8; 32],
    pass: &str,
    memory: u32,
    iterations: u32,
) -> Result<(String, String), SecurityError> {
    let mut salt = [0u8; 16];
    let mut nonce = [0u8; 24];
    let mut instance = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);
    OsRng.fill_bytes(&mut instance);
    let params = Params::new(memory, iterations, 1, Some(32))
        .map_err(|_| SecurityError::Internal("recovery KDF parameters".into()))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut derived = Zeroizing::new([0u8; 32]);
    argon
        .hash_password_into(pass.as_bytes(), &salt, derived.as_mut())
        .map_err(|_| SecurityError::Internal("recovery KDF failed".into()))?;
    let payload=json!({"schema":1,"instance_id":URL_SAFE_NO_PAD.encode(instance),"master_key":URL_SAFE_NO_PAD.encode(master)}).to_string();
    let cipher = XChaCha20Poly1305::new_from_slice(derived.as_ref())
        .map_err(|_| SecurityError::Internal("recovery cipher".into()))?;
    let encrypted = cipher
        .encrypt(XNonce::from_slice(&nonce), payload.as_bytes())
        .map_err(|_| SecurityError::Internal("recovery encryption".into()))?;
    let document=json!({"schema":1,"kdf":{"algorithm":"argon2id","memory_kib":memory,"iterations":iterations,"parallelism":1,"salt":URL_SAFE_NO_PAD.encode(salt)},"cipher":{"algorithm":"xchacha20poly1305","nonce":URL_SAFE_NO_PAD.encode(nonce),"ciphertext":URL_SAFE_NO_PAD.encode(encrypted)}}).to_string();
    let checksum = format!("{:x}", Sha256::digest(document.as_bytes()));
    Ok((document, checksum))
}
fn create_session(c: &Connection, owner: i64, ttl: i64) -> Result<SessionGrant, SecurityError> {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let token = URL_SAFE_NO_PAD.encode(raw);
    OsRng.fill_bytes(&mut raw);
    let csrf = URL_SAFE_NO_PAD.encode(raw);
    let expires = now() + ttl;
    c.execute("INSERT INTO owner_sessions(token_hash,csrf_hash,expires_at,created_at,owner_id) VALUES(?1,?2,?3,?4,?5)",params![token_hash(&token).as_slice(),token_hash(&csrf).as_slice(),expires,now(),owner]).map_err(sec_internal)?;
    Ok(SessionGrant {
        token,
        csrf_token: csrf,
        expires_at: expires,
    })
}
fn authenticate(c: &Connection, token: &str) -> Result<OwnerActor, SecurityError> {
    let result = c.query_row(
        "SELECT owner_id,expires_at FROM owner_sessions WHERE token_hash=?1",
        params![token_hash(token).as_slice()],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    );
    let (id, expires) = result.map_err(|e| {
        if matches!(e, rusqlite::Error::QueryReturnedNoRows) {
            SecurityError::Unauthorized
        } else {
            sec_internal(e)
        }
    })?;
    if expires <= now() {
        let _ = c.execute(
            "DELETE FROM owner_sessions WHERE token_hash=?1",
            params![token_hash(token).as_slice()],
        );
        return Err(SecurityError::Unauthorized);
    }
    Ok(OwnerActor { id })
}
fn token_hash(value: &str) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"canopy-session-v1\0");
    h.update(value.as_bytes());
    h.finalize().into()
}
fn key_fingerprint(key: &[u8; 32]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(b"canopy-master-key-v1\0");
    h.update(key);
    h.finalize().into()
}
fn publication_key_aad(key_id: &str) -> Vec<u8> {
    let mut aad = b"canopy:publication-signing-key-wrap:v1\0".to_vec();
    aad.extend_from_slice(key_id.as_bytes());
    aad
}
fn audit(c: &Connection, actor: &str, action: &str, outcome: &str) -> Result<(), SecurityError> {
    c.execute(
        "INSERT INTO owner_audit(occurred_at,actor_id,action,outcome) VALUES(?1,?2,?3,?4)",
        params![now(), actor, action, outcome],
    )
    .map_err(sec_internal)?;
    Ok(())
}
fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn sec_internal(e: rusqlite::Error) -> SecurityError {
    SecurityError::Internal(e.to_string())
}
fn internal(e: rusqlite::Error) -> AppError {
    AppError::Security(e.to_string())
}
use serde_json::json;
