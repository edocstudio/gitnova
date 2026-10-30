//! Reviewed native amend, preserving original parents/authorship and unstaged work.
use super::*;
use crate::merge::scratch::Scratch;
fn original(git: &GitCli, root: &Path, oid: &str) -> Result<(Commit, String, String, bool)> {
    let raw = git
        .read(
            root,
            &["cat-file".as_ref(), "commit".as_ref(), oid.as_ref()],
            128 * 1024,
        )?
        .bytes;
    let text = String::from_utf8(raw).map_err(|_| {
        AppError::new(
            "amend_blocked",
            "The original commit encoding needs inspection with Git.",
        )
    })?;
    if text.contains('\0') {
        return Err(AppError::new(
            "amend_blocked",
            "The original commit contains unsupported NUL bytes.",
        ));
    }
    let (headers, message) = text
        .split_once("\n\n")
        .ok_or_else(|| AppError::new("amend_blocked", "Invalid original commit object."))?;
    let author = headers
        .lines()
        .find(|s| s.starts_with("author "))
        .ok_or_else(|| AppError::new("amend_blocked", "Missing original author."))?
        .to_owned();
    let formatted = git.text(
        root,
        &[
            "show",
            "-s",
            "--no-show-signature",
            "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
            oid,
            "--",
        ],
    )?;
    let fields = formatted
        .trim_end_matches('\n')
        .as_bytes()
        .split(|v| *v == 0)
        .collect::<Vec<_>>();
    if fields.len() != 6 {
        return Err(AppError::new(
            "amend_blocked",
            "Invalid original commit metadata.",
        ));
    }
    Ok((
        crate::parse_commit(&fields)?,
        message.into(),
        author,
        headers
            .lines()
            .any(|s| s.starts_with("gpgsig ") || s.starts_with("gpgsig-sha256 ")),
    ))
}
impl GitCli {
    pub fn amend_original(&self, root: &Path, oid: &str) -> Result<AmendOriginal> {
        crate::validate_oid(oid)?;
        let (_, before) = self.commit_state(root)?;
        if self
            .optional_text(root, &["rev-parse", "--verify", "--quiet", "HEAD"])?
            .as_deref()
            != Some(oid)
        {
            return Err(AppError::new(
                "stale_preview",
                "HEAD changed. Select Amend again for the current commit.",
            ));
        }
        let (commit, message, _, signed) = original(self, root, oid)?;
        if self.commit_state(root)?.1 != before {
            return Err(AppError::new(
                "stale_preview",
                "HEAD or index changed while reading the amend.",
            ));
        }
        Ok(AmendOriginal {
            commit,
            message,
            signed,
        })
    }
    pub fn preview_amend(&self, root: &Path, request: &AmendRequest) -> Result<AmendPreview> {
        if request.message.trim().is_empty()
            || request.message.len() > 65536
            || request.message.contains('\0')
        {
            return Err(AppError::new(
                "amend_blocked",
                "The amend message must be nonempty and at most 64 KiB.",
            ));
        }
        crate::validate_oid(&request.oid)?;
        let (d, state) = self.commit_state(root)?;
        let status = self.status(root)?;
        if status.head.as_ref() != Some(&request.oid) {
            return Err(AppError::new(
                "stale_preview",
                "HEAD changed. Select Amend again for the current commit.",
            ));
        }
        let branch = status.branch.as_ref().ok_or_else(|| {
            AppError::new("amend_blocked", "Check out a local branch before amending.")
        })?;
        let (original, original_message, _, original_signed) = original(self, root, &request.oid)?;
        let config = self.config_snapshot(root)?;
        self.text(root, &["var", "GIT_COMMITTER_IDENT"])?;
        let changes = self.working_changes(root)?;
        let mut blocked = None;
        if self.multiple_branch_worktrees(root, branch)? {
            blocked=Some("This branch is checked out in multiple worktrees. Inspect them before rewriting it.".into());
        }
        if self
            .optional_text(root, &["rev-parse", "--shared-index-path"])?
            .is_some_and(|p| !p.is_empty())
        {
            return Err(AppError::new(
                "amend_blocked",
                "Split indexes require inspection with Git before amending.",
            ));
        }
        let files = changes
            .files
            .into_iter()
            .filter(|f| f.staged)
            .collect::<Vec<_>>();
        if files.iter().any(|f| {
            f.blocked_reason.is_some()
                && !(f.index_status == "M"
                    && f.blocked_reason.as_deref()
                        == Some("Submodule pointer writes need a separate reviewed workflow"))
        }) {
            blocked = Some("Resolve staged conflicts or unsupported paths before amending.".into());
        }
        let scratch = Scratch::new(self, root)?;
        fs::write(scratch.dir.path().join("index"), index_bytes(&d.git_dir)?)?;
        let expected_tree = scratch.text(&["write-tree"])?.trim().to_owned();
        crate::validate_oid(&expected_tree)?;
        let base_tree = if let Some(parent) = original.parents.first() {
            self.text(root, &["rev-parse", &format!("{parent}^{{tree}}")])?
                .trim()
                .to_owned()
        } else {
            scratch
                .text(&["hash-object", "-t", "tree", "--stdin"])?
                .trim()
                .to_owned()
        };
        let empty_replacement = expected_tree == base_tree;
        let out = self.execute(
            root,
            &[
                "diff".as_ref(),
                "--cached".as_ref(),
                "--no-ext-diff".as_ref(),
                "--no-textconv".as_ref(),
                "--no-renames".as_ref(),
                "--no-color".as_ref(),
                "--submodule=short".as_ref(),
                "--ignore-submodules=none".as_ref(),
                "--src-prefix=a/".as_ref(),
                "--dst-prefix=b/".as_ref(),
                "--".as_ref(),
            ],
            PATCH_LIMIT,
            true,
        )?;
        if expected_tree
            == self
                .text(root, &["rev-parse", &format!("{}^{{tree}}", request.oid)])?
                .trim()
            && request.message.trim_end_matches('\n') == original_message.trim_end_matches('\n')
        {
            blocked = Some("No staged or message changes to amend.".into());
        }
        let referenced_by = self
            .text(
                root,
                &[
                    "for-each-ref",
                    &format!("--contains={}", request.oid),
                    "--format=%(refname)",
                    "refs/remotes/",
                    "refs/tags/",
                ],
            )?
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let recovery_ref = format!("refs/gitnova/recovery/amend-{}", request.oid);
        if self
            .optional_text(root, &["symbolic-ref", "--quiet", &recovery_ref])?
            .is_some()
        {
            return Err(AppError::new(
                "amend_blocked",
                "The amend recovery ref is symbolic. Inspect it with Git.",
            ));
        }
        let recovery_oid =
            self.optional_text(root, &["rev-parse", "--verify", "--quiet", &recovery_ref])?;
        if recovery_oid.as_ref().is_some_and(|oid| oid != &request.oid) {
            blocked = Some(
                "The amend recovery ref contains a different commit. Inspect it with Git.".into(),
            );
        }
        if self.commit_state(root)?.1 != state {
            return Err(AppError::new(
                "stale_preview",
                "HEAD or index changed while reading the amend.",
            ));
        }
        let token = serde_json::to_vec(&(
            request,
            &state,
            &config,
            &expected_tree,
            &referenced_by,
            &recovery_oid,
        ))
        .map_err(|e| AppError::new("snapshot", e.to_string()))?;
        Ok(AmendPreview {
            request: request.clone(),
            commit: CommitPreview {
                // The bounded status scan supplies the complete staged manifest.
                // A truncated aggregate patch is inspected per file, as for normal commits.
                complete_manifest: true,
                token: state,
                files,
                status,
                diff: String::from_utf8_lossy(&out.bytes).into(),
                truncated: out.truncated,
            },
            original,
            original_message,
            original_signed,
            expected_tree,
            empty_replacement,
            referenced_by,
            recovery_ref,
            token: digest(&token),
            blocked_reason: blocked,
        })
    }
    pub fn apply_amend(
        &self,
        root: &Path,
        request: &AmendRequest,
        token: &str,
        acknowledge_rewrite: bool,
    ) -> Result<()> {
        let p = self.preview_amend(root, request)?;
        if p.token != token {
            return Err(AppError::new(
                "stale_preview",
                "The amend scope or referenced history changed. Review again.",
            ));
        }
        if let Some(reason) = p.blocked_reason {
            return Err(AppError::new("amend_blocked", reason));
        }
        if !p.referenced_by.is_empty() && !acknowledge_rewrite {
            return Err(AppError::new(
                "amend_blocked",
                "Acknowledge the referenced-history rewrite before amending.",
            ));
        }
        let (_, _, author, _) = original(self, root, &request.oid)?;
        let exists = self
            .optional_text(root, &["rev-parse", "--verify", "--quiet", &p.recovery_ref])?
            .is_some();
        let save = if exists {
            format!("verify {} {}\n", p.recovery_ref, request.oid)
        } else {
            format!("create {} {}\n", p.recovery_ref, request.oid)
        };
        run_ref_transaction(root, &format!("start\n{save}prepare\ncommit\n"))?;
        self.commit_with_index(root, &request.message, &p.commit.token, true)?;
        let status = self.status(root)?;
        let head = status.head.as_ref().ok_or_else(|| {
            AppError::new(
                "operation_unknown",
                "The amended HEAD could not be verified.",
            )
        })?;
        let (after, message, after_author, _) = original(self, root, head)?;
        if head == &request.oid
            || status.branch != p.commit.status.branch
            || after.parents != p.original.parents
            || after_author != author
            || message != request.message
            || self
                .text(
                    root,
                    &[
                        "diff",
                        "--cached",
                        "--quiet",
                        "--no-ext-diff",
                        "--no-textconv",
                        "--ignore-submodules=none",
                        "HEAD",
                        "--",
                    ],
                )
                .is_err()
            || self
                .text(root, &["rev-parse", &format!("{head}^{{tree}}")])?
                .trim()
                != p.expected_tree
            || self
                .optional_text(root, &["rev-parse", "--verify", "--quiet", &p.recovery_ref])?
                .as_ref()
                != Some(&request.oid)
        {
            return Err(AppError::new("operation_unknown","The amend outcome differs from its review. Inspect HEAD, index and the original recovery ref; nothing was rolled back."));
        }
        Ok(())
    }
}
