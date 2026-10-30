//! Bounded read-only summaries of durable receipts; never an execution/recovery API.
use super::*;
use std::{fs, io::Read, time::UNIX_EPOCH};
fn short(text: &str) -> String {
    text.chars().take(4096).collect()
}
impl Application {
    pub fn recorded_activity(&self, membership_id: &str) -> Result<RecordedActivity> {
        let catalog = self.catalog()?;
        let membership = catalog
            .workspaces
            .iter()
            .flat_map(|w| &w.memberships)
            .find(|m| m.id == membership_id)
            .ok_or_else(|| AppError::new("membership", "Unknown membership"))?;
        let mut result = RecordedActivity {
            entries: vec![],
            incomplete: false,
            directory: NativePath::new(&self.receipt_dir),
        };
        let dir = match fs::symlink_metadata(&self.receipt_dir) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(e) => return Err(e.into()),
        };
        if !dir.is_dir() || dir.file_type().is_symlink() {
            return Err(AppError::new(
                "activity",
                "Recorded activity directory needs inspection.",
            ));
        }
        let mut candidates = vec![];
        for (count, item) in fs::read_dir(&self.receipt_dir)?.enumerate() {
            if count >= 2048 || candidates.len() >= 512 {
                result.incomplete = true;
                break;
            }
            let entry = match item {
                Ok(e) => e,
                Err(_) => {
                    result.incomplete = true;
                    continue;
                }
            };
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let Some(id) = name.strip_suffix(".json") else {
                continue;
            };
            if uuid_valid(id).is_none() {
                continue;
            }
            let metadata = match fs::symlink_metadata(entry.path()) {
                Ok(m) => m,
                Err(_) => {
                    result.incomplete = true;
                    continue;
                }
            };
            if !metadata.is_file() || metadata.file_type().is_symlink() {
                result.incomplete = true;
                continue;
            }
            candidates.push((
                metadata.modified().unwrap_or(UNIX_EPOCH),
                entry.path(),
                id.to_owned(),
                metadata.len(),
            ));
        }
        candidates.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));
        if candidates.len() > 128 {
            result.incomplete = true;
        }
        let mut total = 0;
        for (_, path, id, length) in candidates.into_iter().take(128) {
            if length > 2 * 1024 * 1024 {
                result.incomplete = true;
                continue;
            }
            if total + length > 16 * 1024 * 1024 {
                result.incomplete = true;
                break;
            }
            let mut bytes = vec![];
            let read_limit = (16 * 1024 * 1024 - total).min(2 * 1024 * 1024);
            let read =
                fs::File::open(path).and_then(|f| f.take(read_limit + 1).read_to_end(&mut bytes));
            if read.is_err() || bytes.len() as u64 > read_limit {
                result.incomplete = true;
                continue;
            }
            total += bytes.len() as u64;
            let receipt: OperationReceipt = match serde_json::from_slice(&bytes) {
                Ok(r) => r,
                Err(_) => {
                    result.incomplete = true;
                    continue;
                }
            };
            if receipt.id != id {
                result.incomplete = true;
                continue;
            }
            if receipt.repository_id != membership.repository_id {
                continue;
            }
            let (location, reference) = match &receipt.planned {
                Some(PlannedOperation::WorktreeMaintenance(p)) => (p.recovery_dir.clone(), None),
                Some(PlannedOperation::SubmoduleLifecycle(p)) => {
                    (Some(p.recovery_dir.clone()), None)
                }
                Some(PlannedOperation::SubmoduleSync(p)) => (Some(p.recovery_dir.clone()), None),
                Some(PlannedOperation::AddRemote(p)) => (Some(p.recovery_path.clone()), None),
                Some(PlannedOperation::Discard(p)) => (Some(p.recovery_dir.clone()), None),
                Some(PlannedOperation::Merge(p)) => {
                    (p.recovery_dir.clone(), p.recovery_ref.clone())
                }
                Some(PlannedOperation::Stash(p)) => {
                    (Some(p.recovery_dir.clone()), Some(p.recovery_ref.clone()))
                }
                Some(PlannedOperation::Amend(p)) => (None, Some(p.recovery_ref.clone())),
                Some(PlannedOperation::RemoteDelete(p)) => (None, Some(p.recovery_ref.clone())),
                Some(PlannedOperation::Ref(p)) => (None, p.recovery_ref.clone()),
                Some(PlannedOperation::WorktreeRemove(p)) => (None, Some(p.recovery_ref.clone())),
                _ => (None, None),
            };
            let source = match &receipt.planned {
                Some(PlannedOperation::Discard(p)) => Some(p.source.clone()),
                _ => receipt
                    .worktree_id
                    .as_ref()
                    .and_then(|id| catalog.worktrees.iter().find(|w| &w.id == id))
                    .map(|w| w.root.clone()),
            };
            result.entries.push(RecordedOperation {
                id: receipt.id,
                kind: match &receipt.planned {
                    Some(PlannedOperation::Merge(p)) if p.operation == "rebase" => {
                        short(&receipt.kind.replacen("merge_", "rebase_", 1))
                    }
                    _ => short(&receipt.kind),
                },
                status: short(&receipt.status),
                summary: short(&receipt.summary),
                started_at: receipt.started_at,
                finished_at: receipt.finished_at,
                current_checkout: receipt.worktree_id == membership.worktree_id,
                source,
                recovery_dir: location,
                recovery_ref: reference
                    .or_else(|| receipt.merge.as_ref().and_then(|m| m.recovery_ref.clone())),
                recovery: receipt.recovery.map(|s| short(&s)),
                head_after: receipt.head_after,
            });
        }
        result.entries.sort_by(|a, b| {
            b.finished_at
                .unwrap_or(b.started_at)
                .cmp(&a.finished_at.unwrap_or(a.started_at))
                .then_with(|| b.id.cmp(&a.id))
        });
        result.entries.truncate(10);
        Ok(result)
    }
}
fn uuid_valid(value: &str) -> Option<()> {
    // UUID receipt names only; do not interpret arbitrary filenames as records.
    let raw = value.as_bytes();
    if raw.len() != 36
        || ![8, 13, 18, 23].iter().all(|i| raw[*i] == b'-')
        || !raw
            .iter()
            .enumerate()
            .all(|(i, b)| [8, 13, 18, 23].contains(&i) || b.is_ascii_hexdigit())
    {
        None
    } else {
        Some(())
    }
}
