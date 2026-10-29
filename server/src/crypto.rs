// SPDX-License-Identifier: AGPL-3.0-or-later
//! Secrets at rest (PLAN.md §5): XChaCha20-Poly1305, stored as `v1:` + base64(nonce ‖ ciphertext).

use std::fmt;
use std::io::Write;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as B64;
use base64::engine::general_purpose::URL_SAFE_NO_PAD as B64URL;
use chacha20poly1305::aead::common::Generate;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use sqlx::SqlitePool;

const CHECK: &str = "cupful key check";
const KEY_FILE: &str = "secret.key";

/// 32 random bytes, base64url without padding: tokens, OAuth state, passwords.
pub fn random_token() -> String {
    B64URL.encode(Key::generate().as_slice())
}

/// A secret in memory. `Debug` and `Display` print `***`.
#[derive(Clone, PartialEq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: String) -> Self {
        Secret(s)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

pub struct Cipher(XChaCha20Poly1305);

impl Cipher {
    fn new(key: &Key) -> Self {
        Cipher(XChaCha20Poly1305::new(key))
    }

    /// Throwaway key; `main` replaces it with `load`'s. Used by tests.
    pub fn random() -> Self {
        Self::new(&Key::generate())
    }

    pub fn encrypt(&self, plain: &str) -> String {
        let nonce = XNonce::generate();
        let ct = self
            .0
            .encrypt(&nonce, plain.as_bytes())
            .expect("encrypting a short value cannot fail");
        format!("v1:{}", B64.encode([nonce.as_slice(), &ct].concat()))
    }

    pub fn decrypt(&self, stored: &str) -> Result<String, String> {
        let bytes = stored
            .strip_prefix("v1:")
            .and_then(|b| B64.decode(b).ok())
            .filter(|b| b.len() > 24)
            .ok_or("not an encrypted value")?;
        let (nonce, ct) = bytes.split_at(24);
        let nonce = XNonce::try_from(nonce).map_err(|_| "bad nonce")?;
        let plain = self
            .0
            .decrypt(&nonce, ct)
            .map_err(|_| "wrong key or corrupted value")?;
        String::from_utf8(plain).map_err(|e| e.to_string())
    }

    /// Encrypts a value to store, recording the key check value on first use.
    pub async fn seal(&self, db: &SqlitePool, plain: &str) -> Result<String, sqlx::Error> {
        let check = self.encrypt(CHECK);
        sqlx::query!(
            "INSERT OR IGNORE INTO secret_check (id, value) VALUES (1, ?)",
            check
        )
        .execute(db)
        .await?;
        Ok(self.encrypt(plain))
    }
}

fn parse_key(b64: &str, source: &str) -> Result<Cipher, String> {
    let bytes = B64.decode(b64.trim()).unwrap_or_default();
    let key = Key::try_from(bytes.as_slice()).map_err(|_| {
        format!("{source} must be 32 random bytes in base64 (e.g. `openssl rand -base64 32`)")
    })?;
    Ok(Cipher::new(&key))
}

/// Picks the key: `env_key` (CUPFUL_SECRET_KEY), else `<data_dir>/secret.key`, else a new key file,
/// but only while nothing is encrypted yet. Then checks the key against the stored check value.
pub async fn load(
    db: &SqlitePool,
    data_dir: &Path,
    env_key: Option<&str>,
) -> Result<Cipher, String> {
    let path = data_dir.join(KEY_FILE);
    let check = sqlx::query_scalar!("SELECT value FROM secret_check WHERE id = 1")
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?;
    let sources = format!(
        "Set CUPFUL_SECRET_KEY or restore {} to the key the secrets were saved with; \
         if the key is lost, delete the AI providers' keys and enter them again",
        path.display()
    );
    let cipher = if let Some(k) = env_key {
        parse_key(k, "CUPFUL_SECRET_KEY")?
    } else if path.exists() {
        let k =
            std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        parse_key(&k, &path.display().to_string())?
    } else if check.is_none() {
        let key = Key::generate();
        let mut f = std::fs::OpenOptions::new();
        f.write(true).create_new(true);
        #[cfg(unix)]
        std::os::unix::fs::OpenOptionsExt::mode(&mut f, 0o600);
        f.open(&path)
            .and_then(|mut f| f.write_all(B64.encode(key).as_bytes()))
            .map_err(|e| format!("create {}: {e}", path.display()))?;
        tracing::info!(
            "generated a new secret key at {}; back it up with the database",
            path.display()
        );
        Cipher::new(&key)
    } else {
        return Err(format!(
            "the database holds encrypted secrets but no key was found. {sources}"
        ));
    };
    if let Some(check) = check
        && cipher.decrypt(&check).ok().as_deref() != Some(CHECK)
    {
        return Err(format!(
            "the secret key does not match the one the database's secrets were saved with. {sources}"
        ));
    }
    Ok(cipher)
}
