// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 Paranoid Zero.

//! Local discovery of retained counters under the same immutable rules.
//! The index is a cache of public openings, never evidence of an unspent balance.
//! No receipts or recursive proofs are read when browsing it.

use super::*;
use std::collections::BTreeSet;

const INDEX: &str = "related-v1";
const READY: &str = "ready";

fn family(opening: &ObjectOpening) -> [u8; 32] {
    opening.successor(noid_core::Block128(0)).root().0
}

fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    std::fs::File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|error| error.to_string())?;
    Ok(())
}

pub(super) fn remember(key_path: &Path, opening: &ObjectOpening) -> Result<(), String> {
    let objects = directory(key_path)?;
    let index = objects.join(INDEX);
    let bucket = index.join(hex::encode(family(opening)));
    let path = bucket.join(hex::encode(opening.root().0));
    if read_bounded(&path, 0).is_ok() {
        return Ok(());
    }
    let new_index = !index.exists();
    let new_bucket = !bucket.exists();
    write_atomic(&path, &[])?;
    if new_bucket {
        sync_directory(&index)?;
    }
    if new_index {
        sync_directory(&objects)?;
    }
    Ok(())
}

fn root_name(name: &std::ffi::OsStr) -> Option<[u8; 32]> {
    let text = name.to_str()?;
    if text.len() != 64
        || !text
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return None;
    }
    hex::decode(text).ok()?.try_into().ok()
}

/// Migrate existing watched openings once. The completion marker follows all
/// durable entries; an interruption resumes idempotently on the next request.
fn ensure_ready(key_path: &Path) -> Result<(), String> {
    let objects = directory(key_path)?;
    let marker = objects.join(INDEX).join(READY);
    if read_bounded(&marker, 0).is_ok() {
        return Ok(());
    }
    std::fs::create_dir_all(&objects).map_err(|error| error.to_string())?;
    for entry in std::fs::read_dir(&objects).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|extension| extension == "opening")
        {
            if let Some(root) = path.file_stem().and_then(root_name) {
                remember(key_path, &load_opening(key_path, root)?)?;
            }
        }
    }
    write_atomic(&marker, &[])?;
    sync_directory(&objects)
}

/// Exclusive root cursor, at most 64 decoded openings. The directory walk is
/// streamed with O(limit) memory; it never loads all openings into the wallet.
/// Callers serialize index updates and reads with the existing wallet mutex.
pub(super) fn page(
    key_path: &Path,
    opening: &ObjectOpening,
    after: Option<[u8; 32]>,
    limit: usize,
) -> Result<(Vec<ObjectOpening>, Option<[u8; 32]>), String> {
    if !(1..=64).contains(&limit) {
        return Err("related contract page must contain 1..64 terms".into());
    }
    opening.validate().map_err(|error| error.to_string())?;
    ensure_ready(key_path)?;
    let expected = family(opening);
    let path = directory(key_path)?.join(INDEX).join(hex::encode(expected));
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok((Vec::new(), None))
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut roots = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let Some(root) = root_name(&entry.file_name()) else {
            continue;
        };
        if after.is_some_and(|after| root <= after) {
            continue;
        }
        roots.insert(root);
        if roots.len() > limit + 1 {
            roots.pop_last();
        }
    }
    let more = roots.len() > limit;
    if more {
        roots.pop_last();
    }
    let next = more.then(|| *roots.last().unwrap());
    let openings = roots
        .into_iter()
        .map(|root| {
            let opening = load_opening(key_path, root)?;
            if family(&opening) != expected {
                return Err("saved contract index rules mismatch".into());
            }
            Ok(opening)
        })
        .collect::<Result<_, String>>()?;
    Ok((openings, next))
}

#[cfg(test)]
mod tests {
    use super::*;
    use noid_poseidon2b::primitives::Address;

    #[test]
    fn pages_restore_predecessors_and_keep_other_policies_separate() {
        let temporary = tempfile::tempdir().unwrap();
        let key = temporary.path().join("wallet.key");
        let opening =
            noid_tx::experimental_object::applications::timelocked_vault(Address([7; 32]), 100, 5);
        let mut expected = Vec::new();
        for value in 0..5 {
            let terms = opening.successor(noid_core::Block128(value));
            save_opening(&key, &terms).unwrap();
            expected.push(terms.root().0);
        }
        let mut other = opening.clone();
        other.deadline += 1;
        save_opening(&key, &other).unwrap();
        // Simulate the preceding binary's files, without an index.
        std::fs::remove_dir_all(directory(&key).unwrap().join(INDEX)).unwrap();
        let mut found = Vec::new();
        let mut cursor = None;
        loop {
            let (entries, next) = page(&key, &opening, cursor, 2).unwrap();
            assert!(entries.len() <= 2);
            found.extend(entries.iter().map(|entry| entry.root().0));
            if next.is_none() {
                break;
            }
            cursor = next;
        }
        expected.sort();
        assert_eq!(found, expected);
        assert_eq!(page(&key, &other, None, 2).unwrap().0, vec![other]);
        assert!(page(&key, &opening, None, 0).is_err());
        assert!(page(&key, &opening, None, 65).is_err());
        // Subsequent writes are indexed even after migration completed.
        let newest = opening.successor(noid_core::Block128(6));
        save_opening(&key, &newest).unwrap();
        assert_eq!(page(&key, &opening, None, 64).unwrap().0.len(), 6);
    }

    #[test]
    fn forged_index_entry_cannot_substitute_different_rules() {
        let temporary = tempfile::tempdir().unwrap();
        let key = temporary.path().join("wallet.key");
        let opening =
            noid_tx::experimental_object::applications::timelocked_vault(Address([7; 32]), 100, 5);
        let mut other = opening.clone();
        other.rules.max_fee += 1;
        save_opening(&key, &opening).unwrap();
        save_opening(&key, &other).unwrap();
        let bucket = directory(&key)
            .unwrap()
            .join(INDEX)
            .join(hex::encode(family(&opening)));
        std::fs::write(bucket.join(hex::encode(other.root().0)), []).unwrap();
        assert!(page(&key, &opening, None, 64)
            .unwrap_err()
            .contains("rules mismatch"));
        std::fs::remove_file(bucket.join(hex::encode(other.root().0))).unwrap();
        std::fs::write(bucket.join(hex::encode([9u8; 32])), []).unwrap();
        assert!(page(&key, &opening, None, 64).is_err());
    }
}
