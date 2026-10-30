//! One reviewed, pinned, non-force branch push. Destination inspection never runs push hooks.
use crate::{
    working::{run_fetch, run_ref_transaction},
    BranchReader, GitCli, RemoteReader, RepositoryReader,
};
use gitnova_domain::*;
use sha2::{Digest, Sha256};
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};
pub fn display_destination(url: &str) -> String {
    let url = url.split(['?', '#']).next().unwrap_or(url);
    if let Some((scheme, rest)) = url.split_once("://") {
        let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
        if let Some((_, host)) = host.rsplit_once('@') {
            return format!("{scheme}://[credentials]@{host}/{path}");
        }
    }
    url.to_owned()
}
impl GitCli {
    /// Git may update matching tracking refs even when push uses a URL. Accept
    /// the verified result or the reviewed old value, never a third-party value.
    /// Keep the final check/write atomic, including an already absent deletion.
    pub(crate) fn reconcile_tracking_ref(
        &self,
        path: &Path,
        reference: &str,
        reviewed: Option<&str>,
        desired: Option<&str>,
        oid_len: usize,
    ) -> Result<()> {
        if self
            .optional_text(path, &["symbolic-ref", "--quiet", reference])?
            .is_some()
        {
            return Err(AppError::new("operation_unknown", "Remote operation succeeded, but the tracking ref became symbolic. Inspect it before retrying."));
        }
        let current = self.optional_text(path, &["rev-parse", "--verify", "--quiet", reference])?;
        if current.as_deref() != reviewed && current.as_deref() != desired {
            return Err(AppError::new("operation_unknown", "Remote operation succeeded, but the tracking ref changed concurrently. Inspect both sides before retrying."));
        }
        let zero = "0".repeat(oid_len);
        let expected = current.as_deref().unwrap_or(&zero);
        let command = match desired {
            Some(oid) => format!("update {reference} {oid} {expected}\n"),
            None if current.is_some() => format!("delete {reference} {expected}\n"),
            None => format!("verify {reference} {zero}\n"),
        };
        run_ref_transaction(path, &format!("start\n{command}prepare\ncommit\n"))
    }

    pub(crate) fn push_destination(&self, path: &Path, remote: &str) -> Result<String> {
        if !self
            .remote_branches(path)?
            .remotes
            .iter()
            .any(|name| name == remote)
        {
            return Err(AppError::new("push", "Choose a configured remote."));
        }
        let urls = self.text(
            path,
            &["remote", "get-url", "--push", "--all", "--", remote],
        )?;
        let urls = urls.lines().collect::<Vec<_>>();
        if urls.len() != 1 || urls[0].is_empty() || urls[0].starts_with('-') {
            return Err(AppError::new(
                "push",
                "Choose a remote with exactly one push destination.",
            ));
        }
        if let Some((scheme, rest)) = urls[0].split_once("://") {
            let authority = rest.split('/').next().unwrap_or(rest);
            let embedded = authority.rsplit_once('@').is_some_and(|(userinfo, _)| {
                (scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https"))
                    || userinfo.contains(':')
            });
            if embedded || urls[0].contains('?') || urls[0].contains('#') {
                return Err(AppError::new("push","Use a credential helper or SSH agent instead of credentials/query parameters embedded in a push URL."));
            }
        }
        let resolved = self.text(path, &["ls-remote", "--get-url", "--", urls[0]])?;
        if resolved.trim_end_matches('\n') != urls[0] {
            return Err(AppError::new(
                "push",
                "Chained URL rewrites require manual inspection before pushing here.",
            ));
        }
        Ok(urls[0].into())
    }
    pub(crate) fn destination_head(
        &self,
        path: &Path,
        url: &str,
        branch: &str,
    ) -> Result<Option<String>> {
        let full = format!("refs/heads/{branch}");
        let args = ["ls-remote", "--refs", "--heads", "--", url, &full];
        let args = args.iter().map(OsStr::new).collect::<Vec<_>>();
        let bytes=self.execute_transport(path,&args,1024*1024,false,"file:http:https:ssh:git").map_err(|error|if error.code=="git_auth" {error} else {AppError::new("push_inspection",format!("Cannot inspect the selected push destination ({}). Check repository access and credentials, then review again.",error.code))})?.bytes;
        let text = String::from_utf8(bytes)
            .map_err(|_| AppError::new("push", "Invalid remote ref response."))?;
        let mut result = None;
        for line in text.lines() {
            let (oid, name) = line
                .split_once('\t')
                .ok_or_else(|| AppError::new("push", "Invalid remote ref response."))?;
            if name != full || result.is_some() {
                return Err(AppError::new("push", "Ambiguous remote ref response."));
            }
            crate::validate_oid(oid)?;
            result = Some(oid.to_owned());
        }
        Ok(result)
    }
    pub fn preview_push(&self, path: &Path, request: &PushRequest) -> Result<PushPreview> {
        self.valid_name(path, &request.branch, "heads")?;
        self.valid_name(path, &request.remote_branch, "heads")?;
        crate::validate_oid(&request.oid)?;
        if self
            .optional_text(
                path,
                &[
                    "symbolic-ref",
                    "--quiet",
                    &format!("refs/heads/{}", request.branch),
                ],
            )?
            .is_some()
        {
            return Err(AppError::new(
                "push",
                "Choose a direct local branch rather than a symbolic alias.",
            ));
        }
        let branches = self.local_branches(path)?;
        if !branches
            .iter()
            .any(|b| b.name == request.branch && b.oid == request.oid)
        {
            return Err(AppError::new(
                "stale_preview",
                "The selected local branch moved or disappeared. Refresh and review it again.",
            ));
        }
        let (d, state) = self.write_state(path)?;
        let config = self.config_snapshot(path)?;
        let url = self.push_destination(path, &request.remote)?;
        if request.buffered_https && !url.starts_with("https://") {
            return Err(AppError::new(
                "push",
                "Buffered upload is available only for HTTPS destinations.",
            ));
        }
        let upstream_before = self.branch_upstream(path, &request.branch)?;
        let mut blocked = None;
        let local_path = if let Some(file) = url.strip_prefix("file://") {
            let file = file
                .strip_prefix("localhost/")
                .map_or_else(|| file.to_owned(), |rest| format!("/{rest}"));
            if !file.starts_with('/') {
                return Err(AppError::new(
                    "push",
                    "File URLs with a non-local host require separate inspection.",
                ));
            }
            if file.starts_with('/') {
                let raw = file.as_bytes();
                let mut decoded = Vec::new();
                let mut i = 0;
                while i < raw.len() {
                    if raw[i] == b'%' && i + 2 < raw.len() {
                        if let Some(byte) = std::str::from_utf8(&raw[i + 1..i + 3])
                            .ok()
                            .and_then(|part| u8::from_str_radix(part, 16).ok())
                        {
                            decoded.push(byte);
                            i += 3;
                            continue;
                        }
                    }
                    decoded.push(raw[i]);
                    i += 1;
                }
                #[cfg(windows)]
                if decoded.len() > 2 && decoded[0] == b'/' && decoded[2] == b':' {
                    decoded.remove(0);
                }
                crate::path_from_bytes(decoded).ok()
            } else {
                None
            }
        } else {
            let candidate = path.join(&url);
            candidate.exists().then_some(candidate)
        };
        if let Some(destination) = local_path {
            if let Ok(destination) = self.discover(&destination) {
                if destination.common_dir == d.common_dir {
                    blocked=Some("The push destination is this same repository. Choose an independent remote repository.".into());
                }
            }
        }
        if request.set_upstream && self.multiple_branch_worktrees(path, &request.branch)? {
            blocked=Some("This branch is checked out in multiple worktrees. Push without changing tracking, or review the shared worktrees first.".into());
        }
        if request.set_upstream && self.inherited_tracking(path, &request.branch)? {
            blocked = Some(
                "Tracking settings from included/global configuration need separate review.".into(),
            );
        }
        if request.set_upstream
            && self
                .optional_text(
                    path,
                    &["config", "--bool", "--get", "extensions.worktreeConfig"],
                )?
                .as_deref()
                == Some("true")
        {
            blocked =
                Some("Worktree-specific upstream configuration requires separate review.".into());
        }
        if request.set_upstream
            && branches
                .iter()
                .find(|b| b.name == request.branch)
                .is_some_and(|b| !b.current && b.checked_out_at.is_some())
        {
            blocked=Some("Set upstream from the worktree that has this branch checked out, or push without changing tracking.".into());
        }
        if self
            .optional_text(
                path,
                &[
                    "config",
                    "--bool",
                    "--get",
                    &format!("remote.{}.mirror", request.remote),
                ],
            )?
            .as_deref()
            == Some("true")
        {
            blocked = Some(
                "Mirror remotes are not supported by this single-branch push workflow.".into(),
            );
        }
        if self
            .optional_text(path, &["config", "--get-all", "push.pushOption"])?
            .is_some_and(|v| !v.is_empty())
        {
            blocked = Some(
                "Configured server push options require a separately reviewed workflow.".into(),
            );
        }
        if d.git_dir.join("HEAD.lock").exists()
            || d.git_dir.join("index.lock").exists()
            || d.common_dir.join("config.lock").exists()
        {
            blocked =
                Some("Another Git operation holds a native lock. Wait and review again.".into());
        }
        let tracking = format!("refs/remotes/{}/{}", request.remote, request.remote_branch);
        let fetch = self.text(path, &["remote", "get-url", "--", &request.remote])?;
        let mapping = self
            .optional_text(
                path,
                &[
                    "config",
                    "--get-all",
                    &format!("remote.{}.fetch", request.remote),
                ],
            )?
            .unwrap_or_default();
        let normal_mapping = mapping.lines().count() == 1
            && mapping.lines().any(|line| {
                line.trim_start_matches('+')
                    == format!("refs/heads/*:refs/remotes/{}/*", request.remote)
                    || line.trim_start_matches('+')
                        == format!("refs/heads/{}:{tracking}", request.remote_branch)
            });
        let tracking_ref =
            (fetch.trim_end_matches('\n') == url && normal_mapping).then_some(tracking);
        let tracking_oid = if let Some(ref tracking) = tracking_ref {
            if self
                .optional_text(path, &["symbolic-ref", "--quiet", tracking])?
                .is_some()
            {
                return Err(AppError::new(
                    "push",
                    "Symbolic tracking refs need separate inspection.",
                ));
            }
            self.optional_text(path, &["rev-parse", "--verify", "--quiet", tracking])?
        } else {
            None
        };
        if request.set_upstream && tracking_ref.is_none() {
            blocked = Some(
                "Setting upstream requires a matching fetch/push URL and a standard fetch mapping."
                    .into(),
            );
        }
        let remote_oid = self.destination_head(path, &url, &request.remote_branch)?;
        let mut outgoing = vec![];
        let mut outgoing_count = 0;
        if let Some(ref oid) = remote_oid {
            if self
                .text(path, &["cat-file", "-e", &format!("{oid}^{{commit}}")])
                .is_err()
            {
                blocked=Some("The destination has commits unavailable locally. Fetch that destination before reviewing a push.".into());
            } else if self
                .text(path, &["merge-base", "--is-ancestor", oid, &request.oid])
                .is_err()
            {
                blocked=Some("The destination is ahead or diverged. Fetch and investigate; this workflow never forces a push.".into());
            }
        }
        if blocked.is_none() {
            let range = remote_oid.as_ref().map_or_else(
                || request.oid.clone(),
                |oid| format!("{oid}..{}", request.oid),
            );
            outgoing_count = self
                .text(path, &["rev-list", "--count", &range])?
                .trim()
                .parse()
                .map_err(|_| AppError::new("git_parse", "Invalid outgoing commit count"))?;
            // The pinned source OID defines the complete branch scope. Bound the
            // subject list, not publication: a new destination may include years
            // of already-shared history without representing that many new edits.
            {
                let args = [
                    "log",
                    "--no-show-signature",
                    "--no-decorate",
                    "--date-order",
                    "-z",
                    "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
                    "--max-count=500",
                    &range,
                    "--",
                ];
                let raw = self
                    .read(
                        path,
                        &args.iter().map(OsStr::new).collect::<Vec<_>>(),
                        4 * 1024 * 1024,
                    )?
                    .bytes;
                let mut fields = raw.split(|b| *b == 0).collect::<Vec<_>>();
                if fields.last() == Some(&&b""[..]) {
                    fields.pop();
                }
                if !fields.len().is_multiple_of(6) {
                    return Err(AppError::new(
                        "git_parse",
                        "Invalid outgoing commit records.",
                    ));
                }
                outgoing = fields
                    .as_chunks::<6>()
                    .0
                    .iter()
                    .map(|f| crate::parse_commit(f))
                    .collect::<Result<_>>()?;
            }
        }
        if blocked.is_none() && outgoing.len() != outgoing_count.min(500) {
            return Err(AppError::new(
                "output_limit",
                "Outgoing commit review is incomplete.",
            ));
        }
        let data = serde_json::to_vec(&(
            "push-history-summary-v2",
            outgoing_count,
            request,
            &state,
            &config,
            &url,
            &remote_oid,
            &tracking_ref,
            &tracking_oid,
            &upstream_before,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(PushPreview {
            request: request.clone(),
            destination: display_destination(&url),
            remote_oid,
            outgoing,
            outgoing_count,
            tracking_ref,
            tracking_oid,
            upstream_before,
            token: format!("{:x}", Sha256::digest(data)),
            blocked_reason: blocked,
        })
    }
    pub fn apply_push(&self, path: &Path, request: &PushRequest, token: &str) -> Result<()> {
        let review = self.preview_push(path, request)?;
        if review.token != token {
            return Err(AppError::new(
                "stale_preview",
                "Source, destination or configuration changed. Review the push again.",
            ));
        }
        if let Some(reason) = review.blocked_reason {
            return Err(AppError::new("push_blocked", reason));
        }
        let before = self.head_index_snapshot(path)?;
        let url = self.push_destination(path, &request.remote)?;
        let spec = format!("{}:refs/heads/{}", request.oid, request.remote_branch);
        let mut args: Vec<OsString> = [
            "push",
            "--porcelain",
            "--no-force",
            "--no-force-with-lease",
            "--no-mirror",
            "--no-follow-tags",
            "--recurse-submodules=check",
            "--",
            &url,
            &spec,
        ]
        .iter()
        .map(OsString::from)
        .collect();
        if request.buffered_https {
            // Opt-in, process-local and URL-scoped. Avoid the unbounded memory
            // workarounds often suggested for HTTP 400; larger packs still stream.
            args.splice(
                0..0,
                [
                    OsString::from("-c"),
                    OsString::from(format!("http.{url}.postBuffer=16777216")),
                ],
            );
        }
        run_fetch(path, &args).map_err(|e| {
            AppError::new(&e.code, e.message.replace(&url, &display_destination(&url)))
        })?;
        if self
            .destination_head(path, &url, &request.remote_branch)?
            .as_ref()
            != Some(&request.oid)
        {
            return Err(AppError::new("operation_unknown","Push returned, but the destination differs from the reviewed commit. Inspect the remote before retrying."));
        }
        if let Some(ref tracking) = review.tracking_ref {
            self.reconcile_tracking_ref(
                path,
                tracking,
                review.tracking_oid.as_deref(),
                Some(&request.oid),
                request.oid.len(),
            )?;
            if request.set_upstream {
                self.set_exact_upstream(
                    path,
                    &request.branch,
                    &request.remote,
                    &request.remote_branch,
                )?;
            }
        }
        if self.head_index_snapshot(path)? != before
            || !self
                .local_branches(path)?
                .iter()
                .any(|b| b.name == request.branch && b.oid == request.oid)
            || request.set_upstream
                && self.branch_upstream(path, &request.branch)? != review.tracking_ref
        {
            return Err(AppError::new("operation_unknown","The remote accepted the reviewed commit, but local branch/index/tracking state changed. Refresh and inspect both sides before retrying; publication was not rolled back."));
        }
        Ok(())
    }
}

impl GitCli {
    pub fn validate_hosting_branch(&self, path: &Path, name: &str) -> Result<()> {
        self.valid_name(path, name, "heads")
    }
    pub fn hosting_source(
        &self,
        path: &Path,
        branch: &str,
        oid: &str,
        remote: &str,
    ) -> Result<(String, String)> {
        self.valid_name(path, branch, "heads")?;
        crate::validate_oid(oid)?;
        if !self
            .local_branches(path)?
            .iter()
            .any(|b| b.name == branch && b.oid == oid)
        {
            return Err(AppError::new(
                "stale_preview",
                "The selected source branch moved. Refresh and select it again.",
            ));
        }
        let url = self.push_destination(path, remote)?;
        let bytes = serde_json::to_vec(&(
            branch,
            oid,
            &url,
            self.config_snapshot(path)?,
            self.head_index_snapshot(path)?,
        ))
        .map_err(|_| AppError::new("hosting", "Could not inspect source state."))?;
        Ok((url, format!("{:x}", Sha256::digest(bytes))))
    }
}

#[cfg(test)]
mod tracking_tests {
    use super::*;
    use std::process::Command;

    fn git(root: &Path, args: &[&str]) -> String {
        let out = Command::new(crate::executable::git_executable())
            .arg("-C")
            .arg(root)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(args)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            )
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().into()
    }
    #[test]
    fn tracking_reconciliation_accepts_native_results_and_preserves_concurrent_changes() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        git(root, &["init", "-b", "main"]);
        let mut oids = Vec::new();
        for message in ["Base", "Published", "Concurrent"] {
            git(root, &["commit", "--allow-empty", "-m", message]);
            oids.push(git(root, &["rev-parse", "HEAD"]));
        }
        let cli = GitCli;
        let reference = "refs/remotes/origin/feature";
        let (old, tip, foreign) = (&oids[0], &oids[1], &oids[2]);
        // Older Git leaves the reviewed cache alone; newer Git may have already
        // created/advanced it. Both paths must reach the same verified result.
        for initial in [None, Some(old), Some(tip)] {
            git(root, &["update-ref", "-d", reference]);
            if let Some(oid) = initial {
                git(root, &["update-ref", reference, oid]);
            }
            cli.reconcile_tracking_ref(
                root,
                reference,
                initial.filter(|v| *v != tip).map(String::as_str),
                Some(tip),
                tip.len(),
            )
            .unwrap();
            assert_eq!(git(root, &["rev-parse", reference]), *tip);
        }
        cli.reconcile_tracking_ref(root, reference, Some(old), Some(tip), tip.len())
            .unwrap();
        // Already absent after native deletion is valid; a retained old cache
        // still needs deleting. Never recreate a deleted cache.
        cli.reconcile_tracking_ref(root, reference, Some(tip), None, tip.len())
            .unwrap();
        cli.reconcile_tracking_ref(root, reference, Some(tip), None, tip.len())
            .unwrap();
        assert!(cli
            .optional_text(root, &["rev-parse", "--verify", "--quiet", reference])
            .unwrap()
            .is_none());
        git(root, &["update-ref", reference, foreign]);
        for desired in [Some(tip.as_str()), None] {
            assert!(cli
                .reconcile_tracking_ref(root, reference, Some(old), desired, tip.len())
                .is_err());
            assert_eq!(git(root, &["rev-parse", reference]), *foreign);
        }
        git(root, &["update-ref", "-d", reference]);
        git(root, &["symbolic-ref", reference, "refs/heads/main"]);
        assert!(cli
            .reconcile_tracking_ref(root, reference, Some(foreign), Some(tip), tip.len())
            .is_err());
        assert_eq!(git(root, &["symbolic-ref", reference]), "refs/heads/main");
        git(root, &["symbolic-ref", "--delete", reference]);
        let lock = root.join(".git/refs/remotes/origin/feature.lock");
        std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
        std::fs::write(&lock, "other writer").unwrap();
        assert!(cli
            .reconcile_tracking_ref(root, reference, None, Some(tip), tip.len())
            .is_err());
        assert_eq!(std::fs::read_to_string(lock).unwrap(), "other writer");
        assert_eq!(git(root, &["rev-parse", "HEAD"]), *foreign);
        assert_eq!(git(root, &["status", "--porcelain"]), "");
    }
}
