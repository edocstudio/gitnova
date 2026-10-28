//! Offline, repository-shared commit metadata with worktree-specific reachability.
use gitnova_domain::*;
use gitnova_git::{GitCli, RefReader, RepositoryReader};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    ffi::OsStr,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
pub const SCAN_LIMIT: usize = 20_000;
const RAW_LIMIT: usize = 32 * 1024 * 1024;
const CACHE_LIMIT: usize = 64 * 1024 * 1024;
#[derive(Clone, Serialize, Deserialize)]
struct IndexedCommit {
    commit: Commit,
    message: String,
    raw_size: usize,
    lossy: bool,
    #[serde(skip)]
    encoded_size: usize,
}
#[derive(Default, Serialize, Deserialize)]
struct Cache {
    version: u8,
    checksum: String,
    entries: BTreeMap<String, IndexedCommit>,
}
pub struct SearchIndex {
    requests: Mutex<()>,
    caches: Mutex<BTreeMap<String, Arc<Mutex<Cache>>>>,
    traversals: moka::sync::Cache<TraversalKey, SharedTraversal>,
}
type TraversalKey = (PathBuf, String);
type SharedTraversal = Arc<(Vec<String>, bool)>;
impl Default for SearchIndex {
    fn default() -> Self {
        Self {
            requests: Mutex::new(()),
            caches: Mutex::new(BTreeMap::new()),
            traversals: moka::sync::Cache::builder()
                .max_capacity(4 * 1024 * 1024)
                .weigher(
                    |key: &(PathBuf, String), value: &Arc<(Vec<String>, bool)>| {
                        let bytes = 512
                            + key.0.as_os_str().len() * 4
                            + key.1.capacity()
                            + value.0.capacity() * std::mem::size_of::<String>()
                            + value.0.iter().map(|s| s.capacity() + 32).sum::<usize>();
                        u32::try_from(bytes).unwrap_or(u32::MAX)
                    },
                )
                .time_to_live(std::time::Duration::from_secs(60))
                .time_to_idle(std::time::Duration::from_secs(30))
                .build(),
        }
    }
}
fn rank_window<T>(
    rows: &mut [T],
    offset: usize,
    count: usize,
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
) {
    let end = offset.saturating_add(count).min(rows.len());
    if offset >= end {
        return;
    }
    if end < rows.len() {
        rows.select_nth_unstable_by(end, &compare);
    }
    if offset > 0 {
        rows[..end].select_nth_unstable_by(offset, &compare);
    }
    rows[offset..end].sort_unstable_by(compare);
}

pub struct Source {
    pub location: SearchLocation,
    pub path: PathBuf,
    pub common_dir: PathBuf,
    pub git_dir: Option<PathBuf>,
}
struct GroupResult {
    matches: Vec<SearchMatch>,
    coverage: SearchCoverage,
    snapshots: Vec<(String, String)>,
}
// Rank with small descriptors, then materialize only the requested 100 hits.
// Copying messages and repository paths for every match inflated broad searches.
struct SearchMatch {
    oid: String,
    timestamp: i64,
    locations: u32,
}
fn hash(value: &[u8]) -> String {
    format!("{:x}", Sha256::digest(value))
}
fn parse_commit(oid: &str, bytes: &[u8]) -> Result<IndexedCommit> {
    let end = bytes
        .windows(2)
        .position(|v| v == b"\n\n")
        .ok_or_else(|| AppError::new("search_parse", "Invalid commit header."))?;
    let mut parents = vec![];
    let mut author = None;
    for field in bytes[..end].split(|b| *b == b'\n') {
        if let Some(parent) = field.strip_prefix(b"parent ") {
            let parent = std::str::from_utf8(parent)
                .map_err(|_| AppError::new("search_parse", "Invalid parent."))?;
            gitnova_git::validate_oid(parent)?;
            parents.push(parent.to_owned());
        } else if let Some(value) = field.strip_prefix(b"author ") {
            let value = String::from_utf8_lossy(value);
            let (rest, _zone) = value
                .rsplit_once(' ')
                .ok_or_else(|| AppError::new("search_parse", "Invalid author date."))?;
            let (identity, time) = rest
                .rsplit_once(' ')
                .ok_or_else(|| AppError::new("search_parse", "Invalid author date."))?;
            let timestamp = time
                .parse::<i64>()
                .map_err(|_| AppError::new("search_parse", "Invalid author timestamp."))?;
            let (name, email) = identity
                .rsplit_once(" <")
                .ok_or_else(|| AppError::new("search_parse", "Invalid author identity."))?;
            author = Some((
                name.to_owned(),
                email
                    .strip_suffix('>')
                    .ok_or_else(|| AppError::new("search_parse", "Invalid author email."))?
                    .to_owned(),
                timestamp,
            ));
        }
    }
    let (author, email, timestamp) =
        author.ok_or_else(|| AppError::new("search_parse", "Missing author."))?;
    let message = String::from_utf8_lossy(&bytes[end + 2..]).into_owned();
    let subject = message
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .chars()
        .take(4096)
        .collect();
    let mut entry = IndexedCommit {
        commit: Commit {
            oid: oid.into(),
            parents,
            author,
            email,
            timestamp,
            subject,
        },
        message,
        raw_size: bytes.len(),
        lossy: std::str::from_utf8(bytes).is_err(),
        encoded_size: 0,
    };
    entry.encoded_size = serde_json::to_vec(&entry)
        .map_err(|e| AppError::new("search_cache", e.to_string()))?
        .len();
    Ok(entry)
}
fn prune_unreachable(
    cache: &mut Cache,
    reachable: &BTreeSet<String>,
) -> Result<(usize, usize, bool)> {
    let removed = cache
        .entries
        .keys()
        .filter(|oid| !reachable.contains(*oid))
        .cloned()
        .collect::<Vec<_>>();
    let mut raw = 0;
    let mut encoded = 0;
    let changed = !removed.is_empty();
    for oid in removed {
        if let Some(entry) = cache.entries.remove(&oid) {
            raw += entry.raw_size + 512;
            encoded += entry.encoded_size;
        }
    }
    Ok((raw, encoded, changed))
}
fn batch_records<'a>(bytes: &'a [u8], expected: &[String]) -> Result<Vec<(&'a [u8], String)>> {
    let mut position = 0;
    let mut result = vec![];
    for oid in expected {
        let line_end = bytes[position..]
            .iter()
            .position(|b| *b == b'\n')
            .map(|n| position + n)
            .ok_or_else(|| AppError::new("search_parse", "Missing batch header."))?;
        let fields = std::str::from_utf8(&bytes[position..line_end])
            .map_err(|_| AppError::new("search_parse", "Invalid batch header."))?
            .split_whitespace()
            .collect::<Vec<_>>();
        if fields.len() != 3 || fields[0] != oid || fields[1] != "commit" {
            return Err(AppError::new(
                "search_parse",
                "Unexpected commit object in search batch.",
            ));
        }
        let size = fields[2]
            .parse::<usize>()
            .map_err(|_| AppError::new("search_parse", "Invalid object size."))?;
        let start = line_end + 1;
        let end = start
            .checked_add(size)
            .ok_or_else(|| AppError::new("search_parse", "Object size overflow."))?;
        if end >= bytes.len() || bytes[end] != b'\n' {
            return Err(AppError::new("search_parse", "Incomplete commit batch."));
        }
        result.push((&bytes[start..end], oid.clone()));
        position = end + 1;
    }
    if position != bytes.len() {
        return Err(AppError::new("search_parse", "Unexpected batch suffix."));
    }
    Ok(result)
}
fn input(oids: &[String]) -> Vec<u8> {
    format!("{}\n", oids.join("\n")).into_bytes()
}
fn traversal(git: &GitCli, path: &Path) -> Result<(Vec<String>, bool)> {
    let mut oids = git.history_oids(path, SCAN_LIMIT + 1)?;
    let limited = oids.len() > SCAN_LIMIT;
    oids.truncate(SCAN_LIMIT);
    Ok((oids, limited))
}
impl SearchIndex {
    fn cache(&self, directory: &Path, repository_id: &str) -> Result<Arc<Mutex<Cache>>> {
        // IDs are validated by the registered catalog, and checked again before use as filenames.
        if repository_id.is_empty()
            || !repository_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        {
            return Err(AppError::new("identity", "Invalid search cache identity."));
        }
        let mut caches = self
            .caches
            .lock()
            .map_err(|_| AppError::new("search", "Search cache is unavailable."))?;
        if let Some(cache) = caches.get(repository_id) {
            return Ok(cache.clone());
        }
        let path = directory.join(format!("{repository_id}.json"));
        let mut cache = Cache {
            version: 1,
            ..Default::default()
        };
        if let Ok(file) = std::fs::File::open(path) {
            let mut bytes = vec![];
            file.take(CACHE_LIMIT as u64 + 1).read_to_end(&mut bytes)?;
            if bytes.len() <= CACHE_LIMIT {
                if let Ok(mut saved) = serde_json::from_slice::<Cache>(&bytes) {
                    if saved.version == 1
                        && saved.entries.len() <= 40_000
                        && saved.entries.iter().all(|(oid, entry)| {
                            gitnova_git::validate_oid(oid).is_ok()
                                && &entry.commit.oid == oid
                                && entry.raw_size <= 65536
                        })
                        && serde_json::to_vec(&saved.entries)
                            .is_ok_and(|bytes| hash(&bytes) == saved.checksum)
                    {
                        for entry in saved.entries.values_mut() {
                            entry.encoded_size = serde_json::to_vec(entry)
                                .map_err(|e| AppError::new("search_cache", e.to_string()))?
                                .len();
                        }
                        let raw_size = saved
                            .entries
                            .values()
                            .map(|entry| entry.raw_size as u64 + 512)
                            .sum::<u64>();
                        let encoded_size = saved
                            .entries
                            .values()
                            .map(|entry| entry.encoded_size as u64)
                            .sum::<u64>();
                        if raw_size <= RAW_LIMIT as u64 && encoded_size <= RAW_LIMIT as u64 {
                            cache = saved;
                        }
                    }
                }
            }
        }
        // Cache files are disposable metadata; malformed/oversized files are rebuilt, never trusted as Git state.
        let cache = Arc::new(Mutex::new(cache));
        // At most two resident repository indexes. Up to three current search
        // workers may hold temporary indexes; never retain an additional one
        // just because all resident entries are busy.
        if caches.len() >= 2 {
            if let Some(key) = caches
                .iter()
                .find(|(_, cache)| Arc::strong_count(cache) == 1)
                .map(|(id, _)| id.clone())
            {
                caches.remove(&key);
            }
        }
        if caches.len() < 2 {
            caches.insert(repository_id.into(), cache.clone());
        }
        Ok(cache)
    }
    fn reachable(
        &self,
        git: &GitCli,
        path: &Path,
        fingerprint: &str,
    ) -> Result<Arc<(Vec<String>, bool)>> {
        let key = (std::fs::canonicalize(path)?, fingerprint.to_owned());
        let result = self
            .traversals
            .try_get_with(key, || {
                let result = traversal(git, path)?;
                if git.fingerprint(path)? != fingerprint {
                    return Err(AppError::new(
                        "stale_search",
                        "Repository refs or HEAD changed while searching. Search again.",
                    ));
                }
                Ok(Arc::new(result))
            })
            .map_err(|e: Arc<AppError>| (*e).clone())?;
        self.traversals.run_pending_tasks();
        Ok(result)
    }
    fn group(
        &self,
        git: &GitCli,
        directory: &Path,
        sources: &[&Source],
        terms: &[String],
    ) -> Result<GroupResult> {
        let repository_id = sources[0].location.repository_id.clone();
        let cache = self.cache(directory, &repository_id)?;
        let mut cache = cache
            .lock()
            .map_err(|_| AppError::new("search", "Repository index is unavailable."))?;
        let mut snapshots = vec![];
        let mut reachable = vec![];
        let mut all = BTreeSet::new();
        let mut limited = false;
        for source in sources {
            let discovered = git.discover(&source.path)?;
            if discovered.common_dir != source.common_dir
                || source
                    .git_dir
                    .as_ref()
                    .is_some_and(|expected| expected != &discovered.git_dir)
            {
                return Err(AppError::new(
                    "repository_changed",
                    "Registered repository/worktree identity changed; reopen it before searching.",
                ));
            }
            if git
                .refs(&source.path)?
                .iter()
                .any(|r| r.name.starts_with("refs/replace/"))
                || discovered.common_dir.join("info/grafts").exists()
            {
                return Err(AppError::new(
                    "search_history",
                    "Replacement/grafted histories require a separately validated index.",
                ));
            }
            limited |= discovered.common_dir.join("shallow").exists();
            let before = git.fingerprint(&source.path)?;
            let traversal = self.reachable(git, &source.path, &before)?;
            let (oids, cut) = &*traversal;
            limited |= *cut;
            all.extend(oids.iter().cloned());
            reachable.push(oids.iter().cloned().collect::<BTreeSet<_>>());
            snapshots.push((source.location.membership_id.clone(), before));
        }
        if all.len() > 40_000 {
            return Err(AppError::new(
                "search_budget",
                "Selected worktrees exceed the shared 40,000-commit metadata budget.",
            ));
        }
        // Metadata is immutable and shared across scopes; evict only under actual budget pressure.
        let mut changed = false;
        let mut can_prune = cache.entries.keys().any(|oid| !all.contains(oid));
        let missing = all
            .iter()
            .filter(|oid| !cache.entries.contains_key(*oid))
            .cloned()
            .collect::<Vec<_>>();
        if cache.entries.len().saturating_add(missing.len()) > 40_000 {
            changed |= prune_unreachable(&mut cache, &all)?.2;
            can_prune = false;
        }
        let mut raw_total = cache
            .entries
            .values()
            .map(|c| c.raw_size + 512)
            .sum::<usize>();
        let mut encoded_total = cache
            .entries
            .values()
            .map(|c| c.encoded_size)
            .sum::<usize>();
        for batch in missing.chunks(64) {
            let check = git
                .read_input(
                    &sources[0].path,
                    &[OsStr::new("cat-file"), OsStr::new("--batch-check")],
                    &input(batch),
                    64 * 1024,
                )?
                .bytes;
            let rows = String::from_utf8(check)
                .map_err(|_| AppError::new("search_parse", "Invalid batch-check output."))?;
            let rows = rows.lines().collect::<Vec<_>>();
            if rows.len() != batch.len() {
                return Err(AppError::new("search_parse", "Incomplete object headers."));
            }
            let mut eligible = vec![];
            for (row, oid) in rows.iter().zip(batch) {
                let f = row.split_whitespace().collect::<Vec<_>>();
                if f.len() != 3 || f[0] != oid || f[1] != "commit" {
                    return Err(AppError::new(
                        "search_parse",
                        "Missing or invalid commit object.",
                    ));
                }
                let size = f[2]
                    .parse::<usize>()
                    .map_err(|_| AppError::new("search_parse", "Invalid commit size."))?;
                if raw_total.saturating_add(size + 512) > RAW_LIMIT && can_prune {
                    let (raw, encoded, did_prune) = prune_unreachable(&mut cache, &all)?;
                    raw_total = raw_total.saturating_sub(raw);
                    encoded_total = encoded_total.saturating_sub(encoded);
                    changed |= did_prune;
                    can_prune = false;
                }
                if size > 65536 || raw_total.saturating_add(size + 512) > RAW_LIMIT {
                    limited = true;
                    continue;
                }
                raw_total += size + 512;
                eligible.push(oid.clone());
            }
            if !eligible.is_empty() {
                let bytes = git
                    .read_input(
                        &sources[0].path,
                        &[OsStr::new("cat-file"), OsStr::new("--batch")],
                        &input(&eligible),
                        5 * 1024 * 1024,
                    )?
                    .bytes;
                for (raw, oid) in batch_records(&bytes, &eligible)? {
                    let entry = parse_commit(&oid, raw)?;
                    let size = entry.encoded_size;
                    if encoded_total.saturating_add(size) > RAW_LIMIT && can_prune {
                        let (raw, encoded, did_prune) = prune_unreachable(&mut cache, &all)?;
                        raw_total = raw_total.saturating_sub(raw);
                        encoded_total = encoded_total.saturating_sub(encoded);
                        changed |= did_prune;
                        can_prune = false;
                    }
                    if encoded_total.saturating_add(size) > RAW_LIMIT {
                        limited = true;
                        continue;
                    }
                    encoded_total += size;
                    cache.entries.insert(oid.clone(), entry);
                    changed = true;
                }
            }
        }
        for (source, (_, fingerprint)) in sources.iter().zip(&snapshots) {
            if git.fingerprint(&source.path)? != *fingerprint {
                return Err(AppError::new(
                    "stale_search",
                    "Repository refs or HEAD changed while searching. Search again.",
                ));
            }
        }
        if changed {
            std::fs::create_dir_all(directory)?;
            let mut file = tempfile::NamedTempFile::new_in(directory)?;
            cache.checksum = hash(
                &serde_json::to_vec(&cache.entries)
                    .map_err(|e| AppError::new("search_cache", e.to_string()))?,
            );
            let bytes = serde_json::to_vec(&*cache)
                .map_err(|e| AppError::new("search_cache", e.to_string()))?;
            if bytes.len() > CACHE_LIMIT {
                return Err(AppError::new(
                    "search_budget",
                    "Encoded index exceeds 64 MiB.",
                ));
            }
            file.write_all(&bytes)?;
            file.flush()?;
            file.as_file().sync_all()?;
            file.persist(directory.join(format!("{repository_id}.json")))
                .map_err(|e| AppError::new("search_cache", e.to_string()))?;
        }
        let indexed = all
            .iter()
            .filter(|oid| cache.entries.contains_key(*oid))
            .count();
        limited |= indexed < all.len()
            || all
                .iter()
                .any(|oid| cache.entries.get(oid).is_some_and(|entry| entry.lossy));
        let mut matches = vec![];
        for oid in &all {
            let Some(entry) = cache.entries.get(oid) else {
                continue;
            };
            let haystack = format!(
                "{}\n{}\n{}\n{}",
                entry.commit.oid, entry.commit.author, entry.commit.email, entry.message
            )
            .to_lowercase();
            if terms.iter().all(|term| haystack.contains(term)) {
                let locations = reachable
                    .iter()
                    .enumerate()
                    .filter(|(_, set)| set.contains(oid))
                    .fold(0u32, |mask, (i, _)| mask | (1u32 << i));
                matches.push(SearchMatch {
                    oid: oid.clone(),
                    timestamp: entry.commit.timestamp,
                    locations,
                });
            }
        }
        Ok(GroupResult {
            matches,
            coverage: SearchCoverage {
                repository_id,
                membership_ids: sources
                    .iter()
                    .map(|s| s.location.membership_id.clone())
                    .collect(),
                scanned: all.len(),
                indexed,
                limited,
                issue: limited.then(||"Some history is outside the available local/indexed range. Each worktree is limited to 20,000 commits and bounded metadata. Inspect complete history with Git when needed.".into()),
            },
            snapshots,
        })
    }
    pub fn query(
        &self,
        git: &GitCli,
        directory: &Path,
        request: &CommitSearchRequest,
        sources: &[Source],
    ) -> Result<CommitSearchPage> {
        let _request = self.requests.try_lock().map_err(|_| {
            AppError::new(
                "search_busy",
                "Search index is busy; wait for the current read to finish.",
            )
        })?;
        let query = request.query.trim();
        if query.is_empty() || query.len() > 512 || query.contains('\0') {
            return Err(AppError::new(
                "search_query",
                "Enter 1–512 UTF-8 bytes without NUL.",
            ));
        }
        if sources.is_empty() || sources.len() > 32 {
            return Err(AppError::new(
                "search_scope",
                "Choose 1–32 registered repositories/worktrees.",
            ));
        }
        let terms = query
            .split_whitespace()
            .map(str::to_lowercase)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if terms.len() > 16 {
            return Err(AppError::new(
                "search_query",
                "Use at most 16 search words.",
            ));
        }
        let mut groups = BTreeMap::<String, Vec<&Source>>::new();
        for s in sources {
            groups
                .entry(s.location.repository_id.clone())
                .or_default()
                .push(s);
        }
        let groups = groups.into_values().collect::<Vec<_>>();
        let mut results = vec![];
        for chunk in groups.chunks(3) {
            std::thread::scope(|scope| {
                let workers = chunk
                    .iter()
                    .map(|g| scope.spawn(|| self.group(git, directory, g, &terms)))
                    .collect::<Vec<_>>();
                for (g, worker) in chunk.iter().zip(workers) {
                    let result = worker
                        .join()
                        .unwrap_or_else(|_| Err(AppError::new("search", "Search worker failed.")));
                    results.push(match result {
                        Ok(r) => r,
                        Err(e) => GroupResult {
                            matches: vec![],
                            coverage: SearchCoverage {
                                repository_id: g[0].location.repository_id.clone(),
                                membership_ids: g
                                    .iter()
                                    .map(|s| s.location.membership_id.clone())
                                    .collect(),
                                scanned: 0,
                                indexed: 0,
                                limited: true,
                                issue: Some(e.message.clone()),
                            },
                            snapshots: vec![(
                                g[0].location.repository_id.clone(),
                                format!("error:{}", e.code),
                            )],
                        },
                    });
                }
            });
        }
        let mut matches = vec![];
        let mut coverage = vec![];
        let mut snapshots = vec![];
        for (group, r) in results.into_iter().enumerate() {
            matches.extend(r.matches.into_iter().map(|m| (group, m)));
            coverage.push(r.coverage);
            snapshots.extend(r.snapshots);
        }
        snapshots.sort();
        let token = hash(
            &serde_json::to_vec(&(
                &request.workspace_id,
                &request.scope,
                &request.current_membership_id,
                &request.selected_ids,
                query,
                &snapshots,
                &coverage,
            ))
            .map_err(|e| AppError::new("search", e.to_string()))?,
        );
        let offset = if let Some(cursor) = &request.cursor {
            if cursor.token != token {
                return Err(AppError::new(
                    "stale_search",
                    "Search scope or repository history changed. Search again.",
                ));
            }
            if cursor.offset > 640_000 || !cursor.offset.is_multiple_of(100) {
                return Err(AppError::new("search_cursor", "Invalid search page."));
            }
            cursor.offset
        } else {
            0
        };
        let total_matches = matches.len();
        if offset > total_matches {
            return Err(AppError::new(
                "search_cursor",
                "Search page is outside the result range.",
            ));
        }
        // Partition in place, then sort only the requested page. Broad searches
        // keep exact totals/order without sorting every match or stable-sort's
        // additional whole-result allocation.
        rank_window(&mut matches, offset, 100, |(ag, a), (bg, b)| {
            b.timestamp
                .cmp(&a.timestamp)
                .then_with(|| {
                    coverage[*ag]
                        .repository_id
                        .cmp(&coverage[*bg].repository_id)
                })
                .then_with(|| a.oid.cmp(&b.oid))
        });
        let selected = matches
            .into_iter()
            .skip(offset)
            .take(100)
            .collect::<Vec<_>>();
        let mut page = vec![None; selected.len()];
        for (group, sources) in groups.iter().enumerate() {
            if !selected.iter().any(|(g, _)| *g == group) {
                continue;
            }
            let cache = self.cache(directory, &coverage[group].repository_id)?;
            let cache = cache
                .lock()
                .map_err(|_| AppError::new("search", "Repository index is unavailable."))?;
            for (position, (_, candidate)) in selected
                .iter()
                .enumerate()
                .filter(|(_, (g, _))| *g == group)
            {
                let entry = cache.entries.get(&candidate.oid).ok_or_else(|| {
                    AppError::new("stale_search", "Search metadata changed. Search again.")
                })?;
                let excerpt = entry
                    .message
                    .lines()
                    .filter(|s| !s.trim().is_empty())
                    .find(|line| terms.iter().any(|t| line.to_lowercase().contains(t)))
                    .unwrap_or(&entry.commit.subject)
                    .chars()
                    .take(220)
                    .collect();
                let locations = sources
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| candidate.locations & (1u32 << i) != 0)
                    .map(|(_, source)| source.location.clone())
                    .collect();
                page[position] = Some(CommitSearchHit {
                    commit: entry.commit.clone(),
                    excerpt,
                    locations,
                });
            }
        }
        let page = page
            .into_iter()
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| AppError::new("search", "Incomplete search page."))?;
        Ok(CommitSearchPage {
            hits: page,
            partial: coverage.iter().any(|c| c.limited),
            coverage,
            total_matches,
            next: (offset + 100 < total_matches).then_some(SearchCursor {
                offset: offset + 100,
                token: token.clone(),
            }),
            token,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn busy_repository_indexes_never_grow_the_resident_map() {
        let directory = tempfile::tempdir().unwrap();
        let index = SearchIndex::default();
        let mut held = vec![];
        for i in 0..32 {
            held.push(index.cache(directory.path(), &format!("repo-{i}")).unwrap());
            assert!(index.caches.lock().unwrap().len() <= 2);
        }
        drop(held);
        index.cache(directory.path(), "next").unwrap();
        assert!(index.caches.lock().unwrap().contains_key("next"));
    }
    #[test]
    fn traversal_cache_evicts_under_weight_pressure() {
        let index = SearchIndex::default();
        for i in 0..24 {
            let oids = (0..20_000).map(|n| format!("{n:040x}")).collect();
            index.traversals.insert(
                (PathBuf::from(format!("repo-{i}")), "snapshot".into()),
                Arc::new((oids, false)),
            );
            index.traversals.run_pending_tasks();
        }
        index.traversals.run_pending_tasks();
        assert!(index.traversals.weighted_size() <= 4 * 1024 * 1024);
        assert!(index.traversals.entry_count() < 24);
    }
    #[test]
    fn real_ref_changes_invalidate_traversals_while_dirty_files_reuse_them() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path();
        let git = |args: &[&str]| {
            let out = std::process::Command::new("git")
                .arg("-C")
                .arg(path)
                .args([
                    "-c",
                    "user.name=Cache Test",
                    "-c",
                    "user.email=cache@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                    "-c",
                    "core.hooksPath=/dev/null",
                ])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        git(&["init", "-b", "main"]);
        git(&["commit", "--allow-empty", "-m", "first"]);
        let index = SearchIndex::default();
        let fingerprint = GitCli.fingerprint(path).unwrap();
        let first = index.reachable(&GitCli, path, &fingerprint).unwrap();
        std::fs::write(path.join("untracked.txt"), "unsaved").unwrap();
        assert_eq!(GitCli.fingerprint(path).unwrap(), fingerprint);
        let warm = index.reachable(&GitCli, path, &fingerprint).unwrap();
        assert!(Arc::ptr_eq(&first, &warm));
        git(&["commit", "--allow-empty", "-m", "second"]);
        let next = index
            .reachable(&GitCli, path, &GitCli.fingerprint(path).unwrap())
            .unwrap();
        assert_eq!(first.0.len(), 1);
        assert_eq!(next.0.len(), 2);
        assert!(!Arc::ptr_eq(&first, &next));
    }
    #[test]
    fn byte_lengths_keep_nul_and_forged_headers_inside_their_commit_body() {
        let oid = "a".repeat(40);
        let other = "b".repeat(40);
        let body=format!("tree {}\nauthor A <a@example.invalid> 1 +0000\ncommitter A <a@example.invalid> 1 +0000\n\nTitle\n\nNUL\0{other} commit 12\nbody\n","c".repeat(40));
        let raw = format!(
            "{oid} commit {}\n{body}\n{other} commit {}\n{body}\n",
            body.len(),
            body.len()
        )
        .into_bytes();
        let records = batch_records(&raw, &[oid, other]).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].0, body.as_bytes());
        assert!(parse_commit(&records[0].1, records[0].0)
            .unwrap()
            .message
            .contains('\0'));
    }
    #[test]
    fn incomplete_or_mismatched_batches_are_rejected_instead_of_returning_other_commits() {
        let oid = "a".repeat(40);
        assert!(batch_records(
            format!("{oid} commit 999\nshort\n").as_bytes(),
            std::slice::from_ref(&oid)
        )
        .is_err());
        assert!(batch_records(
            format!("{} commit 0\n\n", "b".repeat(40)).as_bytes(),
            &[oid]
        )
        .is_err());
    }
}

#[cfg(test)]
mod ranked_window_tests {
    use super::rank_window;
    #[test]
    fn broad_pages_match_full_total_order_with_timestamp_and_repository_ties() {
        let input = (0..503)
            .rev()
            .map(|i| (i % 11, i % 3, i))
            .collect::<Vec<_>>();
        let mut expected = input.clone();
        expected.sort();
        for offset in [0, 100, 300, 500, 503] {
            let mut rows = input.clone();
            rank_window(&mut rows, offset, 100, Ord::cmp);
            let end = (offset + 100).min(rows.len());
            assert_eq!(rows[offset..end], expected[offset..end]);
        }
    }
}
