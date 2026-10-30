// SPDX-License-Identifier: AGPL-3.0-or-later
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};

pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!();

/// Filesystem types that are a network share in practice. SQLite's locking is unreliable on them.
const NETWORK_FILESYSTEMS: [&str; 4] = ["nfs", "nfs4", "cifs", "smb"];

/// Parses `/proc/mounts` into `(mount point, filesystem type)` pairs.
///
/// Each line is `device mountpoint fstype options ...`. Spaces in a mount point arrive escaped as
/// octal (`\040`), so splitting on whitespace is safe. Malformed lines are skipped.
pub fn parse_mounts(text: &str) -> Vec<(PathBuf, String)> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split_whitespace();
            fields.next()?; // device
            let mount = fields.next()?;
            let fstype = fields.next()?;
            Some((PathBuf::from(unescape_mount(mount)), fstype.to_string()))
        })
        .collect()
}

/// Decodes the octal escapes `/proc/mounts` uses for whitespace and backslashes.
fn unescape_mount(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(c) = rest.chars().next() {
        if c == '\\' && rest.len() >= 4 && rest.as_bytes()[1..4].iter().all(u8::is_ascii_digit) {
            let digits = &rest[1..4];
            if let Ok(byte) = u8::from_str_radix(digits, 8) {
                out.push(byte as char);
                rest = &rest[4..];
                continue;
            }
        }
        out.push(c);
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// The network mount that contains `path`, if any. The longest matching mount point wins, so a
/// local mount nested under a network root is reported as local. Matching is by path component, so
/// `/mnt/data` does not match `/mnt/database`.
pub fn network_filesystem(path: &Path, mounts: &[(PathBuf, String)]) -> Option<(PathBuf, String)> {
    mounts
        .iter()
        .filter(|(mount, _)| path.starts_with(mount))
        .max_by_key(|(mount, _)| mount.as_os_str().len())
        .filter(|(_, fstype)| {
            NETWORK_FILESYSTEMS
                .iter()
                .any(|known| fstype.eq_ignore_ascii_case(known))
        })
        .cloned()
}

/// Best effort: warns when the data directory sits on a network filesystem, where SQLite locking
/// can corrupt the database. Linux only (reads `/proc/mounts`). Never fails startup.
pub fn warn_if_network_filesystem(data_dir: &Path) {
    #[cfg(target_os = "linux")]
    {
        let path = std::fs::canonicalize(data_dir).unwrap_or_else(|_| data_dir.to_path_buf());
        let Ok(text) = std::fs::read_to_string("/proc/mounts") else {
            return;
        };
        if let Some((mount, fstype)) = network_filesystem(&path, &parse_mounts(&text)) {
            tracing::warn!(
                data_dir = %path.display(),
                mount = %mount.display(),
                filesystem = %fstype,
                "CUPFUL_DATA_DIR is on a {fstype} mount. SQLite locking over a network filesystem \
                 can corrupt the database. Use a local disk."
            );
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = data_dir;
}

fn options() -> SqliteConnectOptions {
    SqliteConnectOptions::new()
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        .synchronous(SqliteSynchronous::Normal)
}

pub async fn open(path: &Path) -> Result<SqlitePool, sqlx::Error> {
    let opts = options()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal);
    let pool = SqlitePoolOptions::new().connect_with(opts).await?;
    MIGRATOR.run(&pool).await?;
    crate::nutrition::foods::seed(&pool).await?;
    Ok(pool)
}

/// Fresh migrated, seeded in-memory database. One connection, since each in-memory connection is its own database.
pub async fn memory() -> SqlitePool {
    let opts = options().in_memory(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .expect("open in-memory db");
    MIGRATOR.run(&pool).await.expect("migrate");
    crate::nutrition::foods::seed(&pool)
        .await
        .expect("seed foods");
    pool
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mounts() -> Vec<(PathBuf, String)> {
        vec![
            ("/".into(), "ext4".into()),
            ("/mnt/data".into(), "nfs4".into()),
            ("/mnt/data/nested".into(), "cifs".into()),
            ("/srv/share".into(), "smb".into()),
            ("/media/usb".into(), "vfat".into()),
        ]
    }

    #[test]
    fn parses_mount_table_and_unescapes_spaces() {
        let parsed = parse_mounts(
            "tmpfs /tmp tmpfs rw 0 0\n\
             nas:/export /mnt/my\\040share nfs rw 0 0\n\
             malformed line\n",
        );
        assert_eq!(
            parsed,
            vec![
                (PathBuf::from("/tmp"), "tmpfs".into()),
                (PathBuf::from("/mnt/my share"), "nfs".into()),
            ]
        );
    }

    #[test]
    fn finds_network_mount_for_nested_path() {
        let found = network_filesystem(Path::new("/mnt/data/cupful/cupful.db"), &mounts());
        assert_eq!(found, Some((PathBuf::from("/mnt/data"), "nfs4".into())));
    }

    #[test]
    fn longest_matching_mount_wins() {
        let found = network_filesystem(Path::new("/mnt/data/nested/db"), &mounts());
        assert_eq!(
            found,
            Some((PathBuf::from("/mnt/data/nested"), "cifs".into()))
        );
        // A local mount nested under a network root is not reported as network.
        let local = [
            ("/".to_string(), "nfs".to_string()),
            ("/mnt/local".to_string(), "ext4".to_string()),
        ];
        let local: Vec<_> = local
            .into_iter()
            .map(|(p, f)| (PathBuf::from(p), f))
            .collect();
        assert_eq!(
            network_filesystem(Path::new("/mnt/local/cupful"), &local),
            None
        );
    }

    #[test]
    fn non_matching_paths_and_sibling_prefixes_return_none() {
        assert_eq!(
            network_filesystem(Path::new("/home/me/.cupful"), &mounts()),
            None
        );
        assert_eq!(
            network_filesystem(Path::new("/mnt/database/cupful"), &mounts()),
            None
        );
        assert_eq!(
            network_filesystem(Path::new("/media/usb/cupful"), &mounts()),
            None
        );
    }

    #[test]
    fn warning_never_panics_for_missing_directory() {
        warn_if_network_filesystem(Path::new("/nonexistent/cupful-test-data-dir"));
    }
}
