// SPDX-License-Identifier: AGPL-3.0-or-later
//! Secrets at rest (PLAN.md §5).
use std::path::PathBuf;

use cupful::crypto::{self, Cipher, Secret};

const KEY_A: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
const KEY_B: &str = "BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBA=";

fn temp_dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("cupful-test-{}", uuid::Uuid::now_v7()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[tokio::test]
async fn round_trip_uses_a_fresh_nonce_each_time() {
    let c = Cipher::random();
    let a = c.encrypt("sk-secret");
    let b = c.encrypt("sk-secret");
    assert!(a.starts_with("v1:"));
    assert_ne!(a, b);
    assert_eq!(c.decrypt(&a).unwrap(), "sk-secret");
    assert!(Cipher::random().decrypt(&a).is_err());
}

#[tokio::test]
async fn fresh_install_generates_a_private_key_file_and_reuses_it() {
    let (db, dir) = (cupful::db::memory().await, temp_dir());
    let c = crypto::load(&db, &dir, None).await.unwrap();
    let path = dir.join("secret.key");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
    let sealed = c.seal(&db, "k").await.unwrap();
    let again = crypto::load(&db, &dir, None).await.unwrap();
    assert_eq!(again.decrypt(&sealed).unwrap(), "k");
}

#[tokio::test]
async fn wrong_key_fails_the_check() {
    let (db, dir) = (cupful::db::memory().await, temp_dir());
    let c = crypto::load(&db, &dir, Some(KEY_A)).await.unwrap();
    c.seal(&db, "k").await.unwrap();
    let err = crypto::load(&db, &dir, Some(KEY_B)).await.err().unwrap();
    assert!(
        err.contains("CUPFUL_SECRET_KEY") && err.contains("secret.key"),
        "{err}"
    );
}

#[tokio::test]
async fn removing_the_env_key_after_saving_secrets_refuses_to_start_without_generating_a_key() {
    let (db, dir) = (cupful::db::memory().await, temp_dir());
    let c = crypto::load(&db, &dir, Some(KEY_A)).await.unwrap();
    c.seal(&db, "k").await.unwrap();
    let err = crypto::load(&db, &dir, None).await.err().unwrap();
    assert!(
        err.contains("CUPFUL_SECRET_KEY") && err.contains("secret.key"),
        "{err}"
    );
    assert!(
        !dir.join("secret.key").exists(),
        "no key generated over existing data"
    );
}

#[tokio::test]
async fn env_key_must_be_32_bytes_of_base64() {
    let (db, dir) = (cupful::db::memory().await, temp_dir());
    assert!(crypto::load(&db, &dir, Some("short")).await.is_err());
}

#[test]
fn secret_prints_stars() {
    let s = Secret::new("sk-live-123".into());
    assert_eq!(format!("{s:?}"), "***");
    assert_eq!(format!("{s}"), "***");
    assert_eq!(s.expose(), "sk-live-123");
}
