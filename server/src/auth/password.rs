// SPDX-License-Identifier: AGPL-3.0-or-later
//! Password hashing and verifying, bounded by a global semaphore.

use std::sync::OnceLock;

use argon2::Argon2;
use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

use crate::error::{Error, Result};

/// Argon2 is memory-hard and these run before authentication: cap how many run at once so a
/// flood of logins can't exhaust memory or the blocking pool.
static HASHING: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(4);

pub(super) async fn hash_password(password: &str) -> Result<String> {
    let _permit = HASHING.acquire().await.expect("semaphore is never closed");
    let password = password.to_owned();
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|h| h.to_string())
            .map_err(|e| Error::Internal(e.to_string()))
    })
    .await
    .map_err(|e| Error::Internal(e.to_string()))?
}

/// Verifies against `hash`, or against a dummy hash when the user doesn't exist, so both cases take as long.
pub(super) async fn verify_password(password: &str, hash: Option<String>) -> bool {
    static DUMMY: OnceLock<String> = OnceLock::new();
    let found = hash.is_some();
    let hash = hash.unwrap_or_else(|| {
        DUMMY
            .get_or_init(|| {
                Argon2::default()
                    .hash_password(b"dummy password")
                    .unwrap()
                    .to_string()
            })
            .clone()
    });
    let _permit = HASHING.acquire().await.expect("semaphore is never closed");
    let password = password.to_owned();
    let ok = tokio::task::spawn_blocking(move || {
        PasswordHash::new(&hash).is_ok_and(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
    })
    .await
    .unwrap_or(false);
    ok && found
}
