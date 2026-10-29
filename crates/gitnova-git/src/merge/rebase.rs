//! Owned native linear rebase. A prepared journal precedes every native step.
use super::*;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct Journal {
    plan: RebasePlan,
    #[serde(default)]
    skipped: Vec<String>,
    predicted_tree: Option<String>,
    authors: Vec<String>,
    refs: String,
    native: String,
    head: String,
}
fn err(message: &str) -> AppError {
    AppError::new("rebase", message)
}
fn refs(git: &GitCli, root: &Path, branch: &str) -> Result<String> {
    let out = git
        .read(
            root,
            &[
                "for-each-ref".as_ref(),
                "--format=%(refname) %(objectname)".as_ref(),
                "refs/heads".as_ref(),
                "refs/remotes".as_ref(),
                "refs/tags".as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    let text = String::from_utf8(out).map_err(|_| err("Ref metadata is not UTF-8."))?;
    Ok(text
        .lines()
        .filter(|line| !line.starts_with(&format!("refs/heads/{branch} ")))
        .collect::<Vec<_>>()
        .join("\n"))
}
fn native(git: &GitCli, root: &Path) -> Result<String> {
    let d = git.discover(root)?;
    let dir = d.git_dir.join("rebase-merge");
    let mut entries = BTreeMap::new();
    if dir.exists() {
        if fs::symlink_metadata(&dir)?.file_type().is_symlink() {
            return Err(err("Rebase metadata cannot be a symbolic link."));
        }
        let mut total = 0;
        for entry in fs::read_dir(&dir)? {
            let e = entry?;
            let m = fs::symlink_metadata(e.path())?;
            if !m.is_file() || m.file_type().is_symlink() {
                return Err(err("Unexpected rebase metadata. Inspect with Git."));
            }
            let bytes = metadata(&e.path(), 1024 * 1024)?;
            total += bytes.len();
            if total > 2 * 1024 * 1024 {
                return Err(err("Rebase metadata exceeds its review budget."));
            }
            entries.insert(e.file_name(), bytes);
        }
    }
    // OsString is not a portable JSON key; preserve exact encoded path bytes.
    let rows = entries
        .into_iter()
        .map(|(name, bytes)| (NativePath::new(Path::new(&name)), bytes))
        .collect::<Vec<_>>();
    Ok(digest(
        &serde_json::to_vec(&rows).map_err(|e| err(&e.to_string()))?,
    ))
}
fn author(git: &GitCli, root: &Path, oid: &str) -> Result<String> {
    let bytes = git
        .read(
            root,
            &[
                "show".as_ref(),
                "-s".as_ref(),
                "--no-show-signature".as_ref(),
                "--format=%an%x00%ae%x00%at%x00%aI".as_ref(),
                oid.as_ref(),
            ],
            65536,
        )?
        .bytes;
    Ok(digest(&bytes))
}
fn save(git: &GitCli, root: &Path, marker: &Marker) -> Result<()> {
    let d = git.discover(root)?;
    let temporary = tempfile::NamedTempFile::new_in(&d.git_dir)?;
    {
        let mut handle = temporary.reopen()?;
        handle.write_all(&marker_bytes(marker)?)?;
        handle.sync_all()?;
    }
    temporary
        .persist(d.git_dir.join(MARKER))
        .map_err(|e| err(&e.to_string()))?;
    Ok(())
}
fn active(git: &GitCli, root: &Path) -> Result<bool> {
    Ok(git.discover(root)?.git_dir.join("rebase-merge").is_dir())
}
fn complete(git: &GitCli, root: &Path, marker: &Marker) -> Result<()> {
    let journal = marker
        .rebase
        .as_ref()
        .ok_or_else(|| err("Missing rebase journal."))?;
    let status = git.status(root)?;
    if active(git, root)?
        || status.branch.as_deref() != Some(&marker.branch)
        || status.dirty
        || refs(git, root, &marker.branch)? != journal.refs
    {
        return Err(err("Rebase completion could not be verified. Inspect the journal and retained original tip."));
    }
    let tip = status.head.ok_or_else(|| err("Missing rewritten HEAD."))?;
    if let Some(tree) = &journal.predicted_tree {
        if git
            .text(root, &["rev-parse", &format!("{tip}^{{tree}}")])?
            .trim()
            != tree
        {
            return Err(err(
                "Final rebase tree differs from the clean preview. Inspect the retained journal.",
            ));
        }
    }
    let commits = git
        .text(
            root,
            &[
                "rev-list",
                "--reverse",
                "--ancestry-path",
                &format!("{}..{tip}", marker.request.source_oid),
            ],
        )?
        .split_whitespace()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let target_ancestor = git
        .optional_text(
            root,
            &[
                "merge-base",
                "--is-ancestor",
                &marker.request.source_oid,
                &tip,
            ],
        )?
        .is_some();
    if !target_ancestor || commits.len() != journal.plan.commits.len() {
        return Err(err("Rebase did not retain the reviewed commit sequence. Inspect the recovery ref before any further write."));
    }
    for (i, oid) in commits.iter().enumerate() {
        if author(git, root, oid)? != journal.authors[i] {
            return Err(err(
                "Rebased authorship did not match the reviewed sequence. Inspect with Git.",
            ));
        }
    }
    // Keep the complete journal beside the recovery ref before closing ownership.
    let archive = marker.session_dir.to_path()?.join("completed.json");
    if !archive.exists() {
        recovery::private_file(&archive, &marker_bytes(marker)?)?;
    }
    fs::remove_file(git.discover(root)?.git_dir.join(MARKER))?;
    Ok(())
}
pub(super) fn state(git: &GitCli, root: &Path, marker: &Marker) -> Result<MergeState> {
    let journal = marker
        .rebase
        .as_ref()
        .ok_or_else(|| err("Missing rebase journal."))?;
    let status = git.status(root)?;
    let d = git.discover(root)?;
    let head = status.head.clone().unwrap_or_default();
    let expected_branch = format!("refs/heads/{}", marker.branch);
    let branch_ref = git
        .optional_text(root, &["rev-parse", "--verify", &expected_branch])?
        .unwrap_or_default();
    let native_hash = native(git, root)?;
    let meta_branch = metadata(&d.git_dir.join("rebase-merge/head-name"), 4096)?;
    let meta_original = metadata(&d.git_dir.join("rebase-merge/orig-head"), 4096)?;
    let meta_target = metadata(&d.git_dir.join("rebase-merge/onto"), 4096)?;
    let valid = marker.prepared
        && active(git, root)?
        && status.branch.is_none()
        && head == journal.head
        && native_hash == journal.native
        && branch_ref.trim() == marker.original_oid
        && String::from_utf8_lossy(&meta_branch).trim() == expected_branch
        && String::from_utf8_lossy(&meta_original).trim() == marker.original_oid
        && String::from_utf8_lossy(&meta_target).trim() == marker.request.source_oid
        && refs(git, root, &marker.branch)? == journal.refs;
    let token = digest(
        &serde_json::to_vec(&(
            marker_bytes(marker)?,
            &head,
            &native_hash,
            metadata(&d.git_dir.join("index"), 64 * 1024 * 1024)?,
            git.config_snapshot(root)?,
        ))
        .map_err(|e| err(&e.to_string()))?,
    );
    Ok(MergeState{delete_source_branch:false,source_ref:marker.request.source_ref.clone(),mainline:None,operation:"rebase".into(),suggested_message:None,phase:if valid{"pending"}else{"incomplete"}.into(),session_id:Some(marker.request.session_id.clone()),branch:Some(marker.branch.clone()),original_oid:Some(head),source_oid:Some(marker.request.source_oid.clone()),recovery_ref:Some(marker.recovery_ref.clone()),conflicts:conflicts(git,root)?,token,blocked_reason:(!valid).then(||"Rebase metadata changed or a native step was interrupted. Inspect the retained journal and native Git state before continuing.".into())})
}
pub(super) fn preview(
    git: &GitCli,
    root: &Path,
    request: &MergeRequest,
) -> Result<MergeStartPreview> {
    if request.delete_source_branch
        || request.mainline.is_some()
        || request.method != MergeMethod::Auto
    {
        return Err(err(
            "Rebase does not accept merge methods, mainline or source deletion.",
        ));
    }
    crate::validate_oid(&request.source_oid)?;
    let status = git.status(root)?;
    let branch = status
        .branch
        .clone()
        .ok_or_else(|| err("Check out a local branch before rebasing."))?;
    let original = status
        .head
        .clone()
        .ok_or_else(|| err("Commit the initial history before rebasing."))?;
    let d = git.discover(root)?;
    let initial_config = git.config_snapshot(root)?;
    let initial_attrs = attributes(git, root)?;
    let mut request = request.clone();
    if request.session_id.is_empty() {
        request.session_id = uuid::Uuid::new_v4().to_string();
    }
    let id =
        uuid::Uuid::parse_str(&request.session_id).map_err(|_| err("Invalid rebase session."))?;
    if let Some(reference) = &request.source_ref {
        if !reference.starts_with("refs/")
            || git
                .text(
                    root,
                    &["rev-parse", "--verify", &format!("{reference}^{{commit}}")],
                )?
                .trim()
                != request.source_oid
        {
            return Err(err("The selected target ref moved. Review again."));
        }
    }
    let base = git
        .text(root, &["merge-base", &original, &request.source_oid])?
        .trim()
        .to_owned();
    crate::validate_oid(&base)?;
    let range = git
        .read(
            root,
            &[
                "rev-list".as_ref(),
                "--reverse".as_ref(),
                "--parents".as_ref(),
                format!("{base}..{original}").as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    let range = String::from_utf8(range).map_err(|_| err("Invalid rebase range."))?;
    let mut commits = Vec::new();
    let mut blocked = None;
    for row in range.lines() {
        let fields = row.split_whitespace().collect::<Vec<_>>();
        if fields.len() != 2 {
            blocked=Some("This range contains merge or root commits. Topology-preserving rebase is not supported yet.".into());
        }
        if let Some(oid) = fields.first() {
            crate::validate_oid(oid)?;
            commits.push((*oid).to_string());
        }
    }
    if commits.len() > 100 {
        return Err(err(
            "Rebase exceeds the 100-commit preview budget. Choose a smaller range.",
        ));
    }
    if commits.is_empty() || base == request.source_oid {
        blocked = Some(
            "This branch is already based on the selected target; no rebase is needed.".into(),
        );
    }
    if git
        .optional_text(
            root,
            &[
                "config",
                "--get-regexp",
                r"^(merge\..*\.driver|branch\..*\.mergeoptions|merge\.renormalize)$",
            ],
        )?
        .is_some()
    {
        blocked=Some("Custom merge drivers, branch merge options or renormalization need a separately validated preview.".into());
    }
    let target_raw = git
        .read(
            root,
            &[
                "diff-tree".as_ref(),
                "--raw".as_ref(),
                "--no-renames".as_ref(),
                "-r".as_ref(),
                "-z".as_ref(),
                original.as_ref(),
                request.source_oid.as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    if !gitlink_changes(&target_raw)?.is_empty() {
        blocked=Some("This rebase changes submodule pointers. Review those histories separately before rebasing.".into());
    }
    let working = git.working_changes(root)?;
    if status.dirty || !working.files.is_empty() {
        blocked=Some("Commit or preserve uncommitted work before starting this operation. GitNova will not stash or discard it automatically.".into());
    }
    if git.merge_state(root)?.phase != "none"
        || d.git_dir.join("rebase-merge").exists()
        || d.git_dir.join("rebase-apply").exists()
    {
        blocked =
            Some("Finish or recover the current GitNova operation before starting another.".into());
    }
    if hidden_entries(git, root)? {
        blocked = Some("Hidden index entries must be reviewed with Git before rebasing.".into());
    }
    if git.multiple_branch_worktrees(root, &branch)? {
        blocked = Some(
            "This branch is checked out in multiple worktrees. Inspect them before rebasing."
                .into(),
        );
    }
    if d.git_dir.join("index.lock").exists()
        || d.git_dir.join("HEAD.lock").exists()
        || d.common_dir.join("config.lock").exists()
    {
        blocked = Some("Another Git operation holds a native lock. Wait and review again.".into());
    }
    let initial_refs = refs(git, root, &branch)?;
    let mut referenced_by = Vec::new();
    for oid in &commits {
        let out = git
            .read(
                root,
                &[
                    "for-each-ref".as_ref(),
                    format!("--contains={oid}").as_ref(),
                    "--format=%(refname)".as_ref(),
                    "refs/remotes".as_ref(),
                ],
                65536,
            )?
            .bytes;
        for name in String::from_utf8_lossy(&out).lines() {
            if !referenced_by.iter().any(|n| n == name) {
                referenced_by.push(name.to_string());
            }
        }
    }
    // Validate the entire replay range before prediction: a first conflict must
    // not hide a later submodule-pointer change that native Continue would replay.
    if blocked.is_none() {
        for oid in &commits {
            let parent = git.text(root, &["rev-parse", &format!("{oid}^")])?;
            let raw = git
                .read(
                    root,
                    &[
                        "diff-tree".as_ref(),
                        "--raw".as_ref(),
                        "--no-renames".as_ref(),
                        "-r".as_ref(),
                        "-z".as_ref(),
                        parent.trim().as_ref(),
                        oid.as_ref(),
                    ],
                    1024 * 1024,
                )?
                .bytes;
            if !gitlink_changes(&raw)?.is_empty() {
                blocked=Some("This rebase changes submodule pointers. Review those histories separately before rebasing.".into());
                break;
            }
        }
    }
    let mut files = Vec::new();
    let mut predicted = Vec::new();
    let mut result_tree = None;
    if blocked.is_none() {
        let scratch = Scratch::new(git, root)?;
        let mut cursor = request.source_oid.clone();
        for oid in &commits {
            let parent = git.text(root, &["rev-parse", &format!("{oid}^")])?;
            let old_tree = scratch.text(&["rev-parse", &format!("{}^{{tree}}", parent.trim())])?;
            let new_tree = scratch.text(&["rev-parse", &format!("{oid}^{{tree}}")])?;
            let virtual_base = scratch.commit(old_tree.trim(), None)?;
            let cursor_tree = scratch.text(&["rev-parse", &format!("{cursor}^{{tree}}")])?;
            let ours = scratch.commit(cursor_tree.trim(), Some(&virtual_base))?;
            let incoming = scratch.commit(new_tree.trim(), Some(&virtual_base))?;
            let output = scratch.run(
                &[
                    "merge-tree".into(),
                    "--write-tree".into(),
                    "--name-only".into(),
                    "--no-messages".into(),
                    "-z".into(),
                    ours.into(),
                    incoming.into(),
                ],
                None,
                true,
            )?;
            let mut parts = output.split(|b| *b == 0);
            let tree = std::str::from_utf8(parts.next().unwrap_or_default())
                .map_err(|_| err("Invalid preview tree."))?
                .trim();
            crate::validate_oid(tree)?;
            predicted = parts
                .filter(|b| !b.is_empty())
                .map(|b| Ok(NativePath::new(&crate::path_from_bytes(b.to_vec())?)))
                .collect::<Result<Vec<_>>>()?;
            if !predicted.is_empty() {
                break;
            }
            cursor = scratch.commit(tree, Some(&cursor))?;
            result_tree = Some(tree.to_string());
        }
        if predicted.is_empty() {
            if let Some(tree) = &result_tree {
                let raw = scratch.run(
                    &[
                        "diff-tree".into(),
                        "--name-only".into(),
                        "--no-renames".into(),
                        "-r".into(),
                        "-z".into(),
                        original.clone().into(),
                        tree.into(),
                    ],
                    None,
                    false,
                )?;
                files = raw
                    .split(|b| *b == 0)
                    .filter(|b| !b.is_empty())
                    .map(|b| {
                        Ok(BranchFileChange {
                            path: NativePath::new(&crate::path_from_bytes(b.to_vec())?),
                            status: "M".into(),
                        })
                    })
                    .collect::<Result<_>>()?;
            }
        }
    }
    if git.working_changes(root)?.token != working.token
        || git.config_snapshot(root)? != initial_config
        || attributes(git, root)? != initial_attrs
        || refs(git, root, &branch)? != initial_refs
    {
        return Err(err(
            "Repository changed during rebase prediction. Review again.",
        ));
    }
    let plan = RebasePlan {
        base_oid: base,
        commits,
        referenced_by,
    };
    let token = digest(
        &serde_json::to_vec(&(
            &request,
            &original,
            &working.token,
            &plan,
            &result_tree,
            &predicted,
            initial_config,
            initial_attrs,
            initial_refs,
            NativePath::new(&d.git_dir),
            NativePath::new(&d.common_dir),
        ))
        .map_err(|e| err(&e.to_string()))?,
    );
    Ok(MergeStartPreview {
        rebase: Some(plan),
        preserved_submodules: vec![],
        parents: vec![],
        submodules: vec![],
        suggested_message: String::new(),
        request,
        branch,
        original_oid: original,
        result_tree,
        mode: "rebase".into(),
        files,
        conflicts: predicted,
        recovery_ref: format!("refs/gitnova/recovery/merge-{}", id.simple()),
        token,
        blocked_reason: blocked,
    })
}
fn args() -> Vec<OsString> {
    [
        "-c",
        "core.editor=true",
        "-c",
        "submodule.recurse=false",
        "-c",
        "rebase.autoStash=false",
        "-c",
        "rebase.updateRefs=false",
        "-c",
        "rebase.autoSquash=false",
        "-c",
        "rerere.enabled=false",
        "-c",
        "rerere.autoupdate=false",
        "rebase",
    ]
    .into_iter()
    .map(Into::into)
    .collect()
}
fn after_step(git: &GitCli, root: &Path, marker: &mut Marker, outcome: Result<()>) -> Result<()> {
    if active(git, root)? {
        let journal = marker
            .rebase
            .as_mut()
            .ok_or_else(|| err("Missing rebase journal."))?;
        journal.native = native(git, root)?;
        journal.head = git
            .status(root)?
            .head
            .ok_or_else(|| err("Missing rebase HEAD."))?;
        marker.prepared = true;
        save(git, root, marker)?;
        if !conflicts(git, root)?.is_empty() {
            return Ok(());
        }
        // Empty-stop is intentional and requires an explicit Skip review.
        if git
            .discover(root)?
            .git_dir
            .join("rebase-merge/stopped-sha")
            .is_file()
        {
            return Ok(());
        }
        return outcome;
    }
    outcome?;
    complete(git, root, marker)
}
pub(super) fn begin(git: &GitCli, root: &Path, request: &MergeRequest, token: &str) -> Result<()> {
    let p = preview(git, root, request)?;
    if p.token != token {
        return Err(err("Rebase preview changed. Review again."));
    }
    if let Some(reason) = p.blocked_reason {
        return Err(err(&reason));
    }
    if !request.acknowledge_rewrite {
        return Err(err(
            "Acknowledge that rebase rewrites commit IDs before starting.",
        ));
    }
    let plan = p.rebase.ok_or_else(|| err("Missing rebase plan."))?;
    let id =
        uuid::Uuid::parse_str(&p.request.session_id).map_err(|_| err("Invalid rebase session."))?;
    let d = git.discover(root)?;
    let dir = d.git_dir.join(format!("gitnova-merge-{}", id.simple()));
    recovery::private_dir(&dir)?;
    let authors = plan
        .commits
        .iter()
        .map(|oid| author(git, root, oid))
        .collect::<Result<Vec<_>>>()?;
    let mut marker = Marker {
        version: 1,
        request: p.request.clone(),
        branch: p.branch.clone(),
        original_oid: p.original_oid.clone(),
        recovery_ref: p.recovery_ref.clone(),
        session_dir: NativePath::new(&dir),
        prepared: false,
        native_metadata: BTreeMap::new(),
        preserved_submodules: vec![],
        suggested_message: None,
        rebase: Some(Journal {
            plan: plan.clone(),
            skipped: vec![],
            predicted_tree: if p.conflicts.is_empty() {
                p.result_tree.clone()
            } else {
                None
            },
            authors,
            refs: refs(git, root, &p.branch)?,
            native: String::new(),
            head: p.original_oid.clone(),
        }),
    };
    recovery::private_file(&d.git_dir.join(MARKER), &marker_bytes(&marker)?)?;
    crate::working::run_ref_transaction(
        root,
        &format!(
            "start\ncreate {} {}\ncreate {}-incoming {}\nprepare\ncommit\n",
            p.recovery_ref, p.original_oid, p.recovery_ref, p.request.source_oid
        ),
    )?;
    let mut command = args();
    command.extend(
        [
            "--merge",
            "--no-autostash",
            "--no-update-refs",
            "--no-autosquash",
            "--no-rebase-merges",
            "--reapply-cherry-picks",
            "--keep-empty",
            "--empty=ask",
            "--onto",
            &p.request.source_oid,
            &plan.base_oid,
            &p.branch,
        ]
        .into_iter()
        .map(Into::into),
    );
    let result = run_offline_write(root, &command);
    after_step(git, root, &mut marker, result)
}
pub(super) fn preview_action(
    git: &GitCli,
    root: &Path,
    marker: &Marker,
    action: &MergeAction,
) -> Result<MergeActionPreview> {
    // Metadata-only session release is shared by all operations in merge/mod.rs.
    if !["finish", "abort", "skip"].contains(&action.kind.as_str())
        || action.path.is_some()
        || action.choice.is_some()
        || action.content.is_some()
    {
        return Err(err(
            "Choose Continue, Skip, Abort or session inspection for this rebase.",
        ));
    }
    let state = state(git, root, marker)?;
    let mut blocked = state.blocked_reason.clone();
    let d = git.discover(root)?;
    if d.git_dir.join("index.lock").exists()
        || d.git_dir.join("HEAD.lock").exists()
        || d.common_dir.join("config.lock").exists()
    {
        blocked = Some("Another Git operation holds a native lock. Wait and review again.".into());
    }
    let (snapshot, paths) = recovery::fingerprint(git, root, marker)?;
    let mut expected = None;
    let mut diff = String::new();
    let mut truncated = false;
    if action.kind == "finish" || action.kind == "skip" {
        if !state.conflicts.is_empty() {
            blocked = Some("Resolve every rebase conflict before continuing.".into());
        } else {
            let (scratch, tree) = index_tree(git, root)?;
            let head = state
                .original_oid
                .as_deref()
                .ok_or_else(|| err("Missing rebase HEAD."))?;
            let empty = git
                .text(root, &["rev-parse", &format!("{head}^{{tree}}")])?
                .trim()
                == tree;
            if action.kind == "skip" && !empty {
                blocked=Some("Only an empty replay result can be skipped here. Review the staged changes instead.".into());
            }
            if action.kind == "finish" && empty {
                blocked = Some(
                    "This replay result is empty. Review Skip commit explicitly, or abort.".into(),
                );
            }
            if git.working_changes(root)?.files.iter().any(|f| f.unstaged) {
                blocked = Some(
                    "Review and stage the working result before continuing the rebase.".into(),
                );
            }
            (diff, truncated) = scratch.diff_preview(head, &tree)?;
            expected = Some(tree);
        }
    }
    let token = digest(
        &serde_json::to_vec(&(action, &state, &snapshot, &expected))
            .map_err(|e| err(&e.to_string()))?,
    );
    Ok(MergeActionPreview {
        truncated,
        action: action.clone(),
        state,
        diff,
        paths,
        recovery_dir: Some(NativePath::new(
            &marker
                .session_dir
                .to_path()?
                .join(format!("{}-{token}", action.kind)),
        )),
        expected_tree: expected,
        token,
        blocked_reason: blocked,
    })
}
pub(super) fn apply_action(
    git: &GitCli,
    root: &Path,
    marker: &Marker,
    action: &MergeAction,
    token: &str,
) -> Result<()> {
    let p = preview_action(git, root, marker, action)?;
    if p.token != token {
        return Err(err("Rebase files or metadata changed. Review again."));
    }
    if let Some(reason) = &p.blocked_reason {
        return Err(err(reason));
    }
    recovery::save(git, root, marker, &p)?;
    if preview_action(git, root, marker, action)?.token != token {
        return Err(err("Rebase changed while saving recovery. Review again."));
    }
    let mut marker = marker.clone();
    marker.prepared = false;
    save(git, root, &marker)?;
    if action.kind == "skip" {
        let journal = marker
            .rebase
            .as_mut()
            .ok_or_else(|| err("Missing rebase journal."))?;
        journal.predicted_tree = None;
        let stopped = git.text(root, &["rev-parse", "--verify", "REBASE_HEAD"])?;
        let index = journal
            .plan
            .commits
            .iter()
            .position(|oid| oid == stopped.trim())
            .ok_or_else(|| err("Stopped commit is not in this rebase plan."))?;
        journal.skipped.push(journal.plan.commits.remove(index));
        journal.authors.remove(index);
        save(git, root, &marker)?;
    }
    let mut command = args();
    command.push(
        match action.kind.as_str() {
            "abort" => "--abort",
            "skip" => "--skip",
            _ => "--continue",
        }
        .into(),
    );
    let result = run_offline_write(root, &command);
    if action.kind == "abort" {
        result?;
        let status = git.status(root)?;
        if active(git, root)?
            || status.branch.as_deref() != Some(&marker.branch)
            || status.head.as_deref() != Some(&marker.original_oid)
        {
            return Err(err(
                "Abort could not be verified. Inspect retained recovery snapshots.",
            ));
        }
        recovery::private_file(
            &marker.session_dir.to_path()?.join("aborted.json"),
            &marker_bytes(&marker)?,
        )?;
        fs::remove_file(git.discover(root)?.git_dir.join(MARKER))?;
        return Ok(());
    }
    after_step(git, root, &mut marker, result)
}
