//! Owned native linear rebase. A prepared journal precedes every native step.
use super::*;
use std::collections::BTreeSet;
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
    let labels = git
        .read(
            root,
            &[
                "for-each-ref".as_ref(),
                "--format=%(refname) %(objectname) %(symref)".as_ref(),
                "refs/rewritten/".as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    if String::from_utf8_lossy(&labels)
        .lines()
        .any(|line| line.split_whitespace().count() != 2)
    {
        return Err(err(
            "Rebase label refs cannot be symbolic. Inspect retained native state.",
        ));
    }
    if labels.is_empty() {
        return Ok(digest(
            &serde_json::to_vec(&rows).map_err(|e| err(&e.to_string()))?,
        ));
    }
    Ok(digest(
        &serde_json::to_vec(&(rows, labels)).map_err(|e| err(&e.to_string()))?,
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
                if journal.plan.preserve_merges {
                    "--topo-order"
                } else {
                    "--ancestry-path"
                },
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
    if journal.plan.preserve_merges {
        verify_topology(git, root, marker, &tip)?;
    } else {
        for (i, oid) in commits.iter().enumerate() {
            if author(git, root, oid)? != journal.authors[i] {
                return Err(err(
                    "Rebased authorship did not match the reviewed sequence. Inspect with Git.",
                ));
            }
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
fn verify_topology(git: &GitCli, root: &Path, marker: &Marker, tip: &str) -> Result<()> {
    let j = marker
        .rebase
        .as_ref()
        .ok_or_else(|| err("Missing rebase journal."))?;
    let mut pending = vec![(
        if j.plan.tip.is_empty() {
            marker.original_oid.clone()
        } else {
            j.plan.tip.clone()
        },
        tip.to_string(),
    )];
    let mut mapped = BTreeMap::new();
    let mut reverse = BTreeMap::new();
    while let Some((old, new)) = pending.pop() {
        if !j.plan.commits.contains(&old) {
            if &new
                != if old == j.plan.base_oid {
                    &marker.request.source_oid
                } else {
                    &old
                }
            {
                return Err(err(
                    "Rebase changed an external parent. Inspect retained recovery.",
                ));
            }
            continue;
        }
        if let Some(known) = mapped.get(&old) {
            if known != &new {
                return Err(err("Rebase topology mapping is ambiguous."));
            }
            continue;
        }
        if reverse.insert(new.clone(), old.clone()).is_some() {
            return Err(err("Rebase collapsed reviewed commits."));
        }
        mapped.insert(old.clone(), new.clone());
        if author(git, root, &old)? != author(git, root, &new)?
            || git.text(root, &["show", "-s", "--format=%B", &old])?
                != git.text(root, &["show", "-s", "--format=%B", &new])?
        {
            return Err(err(
                "Rebased authorship or message differs from the reviewed topology.",
            ));
        }
        let row = git.text(root, &["rev-list", "--parents", "-n", "1", &new])?;
        let actual = row
            .split_whitespace()
            .skip(1)
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let old_parents = j
            .plan
            .parents
            .get(&old)
            .ok_or_else(|| err("Missing reviewed parent topology."))?;
        let expected = if old_parents.is_empty() {
            vec![marker.request.source_oid.clone()]
        } else {
            old_parents.clone()
        };
        if actual.len() != expected.len() {
            return Err(err("Rebase did not preserve merge parent counts."));
        }
        pending.extend(expected.into_iter().zip(actual));
    }
    if mapped.len() != j.plan.commits.len() {
        return Err(err("Rebase omitted part of the reviewed topology."));
    }
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
        .optional_text(root, &["merge-base", &original, &request.source_oid])?
        .unwrap_or_default()
        .trim()
        .to_owned();
    let replay_root = base.is_empty();
    if !replay_root {
        crate::validate_oid(&base)?;
    }
    if git
        .text(root, &["rev-parse", "--is-shallow-repository"])?
        .trim()
        == "true"
    {
        return Err(err("Complete history is required before rebasing; shallow boundaries cannot be treated as root commits."));
    }
    let range_spec = if replay_root {
        original.clone()
    } else {
        format!("{base}..{original}")
    };
    let range = git
        .read(
            root,
            &[
                "rev-list".as_ref(),
                "--reverse".as_ref(),
                "--topo-order".as_ref(),
                "--parents".as_ref(),
                range_spec.as_ref(),
            ],
            1024 * 1024,
        )?
        .bytes;
    let range = String::from_utf8(range).map_err(|_| err("Invalid rebase range."))?;
    let mut commits = Vec::new();
    let mut parents = BTreeMap::new();
    let mut preserve_merges = false;
    let mut blocked = None;
    for row in range.lines() {
        let fields = row.split_whitespace().collect::<Vec<_>>();
        if fields.len() > 9 || fields.len() < 2 && !replay_root {
            blocked =
                Some("Merge ranges with more than eight parents require manual Git review.".into());
        }
        if let Some(oid) = fields.first() {
            crate::validate_oid(oid)?;
            parents.insert(
                (*oid).to_string(),
                fields[1..]
                    .iter()
                    .map(|p| (*p).to_string())
                    .collect::<Vec<_>>(),
            );
            preserve_merges |= fields.len() >= 3;
            commits.push((*oid).to_string());
        }
    }
    if commits.len() > 100 {
        return Err(err(
            "Rebase exceeds the 100-commit preview budget. Choose a smaller range.",
        ));
    }
    if commits.is_empty() || base == request.source_oid && request.rebase_steps.is_empty() {
        blocked = Some(
            "This branch is already based on the selected target; no rebase is needed.".into(),
        );
    }
    let original_commits = commits.clone();
    if request.rebase_steps.len() > 100 {
        return Err(err("Replay editing exceeds the 100-commit review budget."));
    }
    if !request.rebase_steps.is_empty() {
        if !cfg!(unix) {
            return Err(err(
                "Interactive replay currently requires a Unix Git environment.",
            ));
        }
        if preserve_merges {
            return Err(err(
                "Edit a linear replay separately from topology-preserving Rebase.",
            ));
        }
        let mut seen = BTreeSet::new();
        let mut picks = Vec::new();
        for step in &request.rebase_steps {
            if !original_commits.contains(&step.oid)
                || !seen.insert(step.oid.clone())
                || !["pick", "drop", "fixup"].contains(&step.action.as_str())
            {
                return Err(err("Replay steps must include every original commit exactly once, using Pick, Drop or Fixup."));
            }
            if step.action == "pick" {
                picks.push(step.oid.clone());
            }
            if step.action == "fixup" && picks.is_empty() {
                return Err(err("Fixup needs an earlier kept commit."));
            }
        }
        if seen.len() != original_commits.len() || picks.is_empty() {
            return Err(err(
                "Keep at least one Pick and include every original commit in the replay plan.",
            ));
        }
        commits = picks;
    }
    let subjects_raw = git
        .read(
            root,
            &std::iter::once(OsString::from("show"))
                .chain(
                    ["-s", "--no-show-signature", "--format=%H%x00%s"]
                        .into_iter()
                        .map(Into::into),
                )
                .chain(original_commits.iter().map(Into::into))
                .collect::<Vec<_>>()
                .iter()
                .map(OsString::as_os_str)
                .collect::<Vec<_>>(),
            1024 * 1024,
        )?
        .bytes;
    let mut subjects = BTreeMap::new();
    for row in subjects_raw.split(|b| *b == b'\n') {
        if let Some(at) = row.iter().position(|b| *b == 0) {
            subjects.insert(
                String::from_utf8_lossy(&row[..at]).into_owned(),
                String::from_utf8_lossy(&row[at + 1..]).into_owned(),
            );
        }
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
    // Multiple --contains filters are ORed by Git: one bounded traversal
    // preserves publication evidence for every original commit, including Drop
    // and Fixup steps, without spawning up to 100 identical Git processes.
    let referenced_by = if original_commits.is_empty() {
        Vec::new()
    } else {
        let mut args: Vec<std::ffi::OsString> =
            vec!["for-each-ref".into(), "--format=%(refname)".into()];
        args.extend(
            original_commits
                .iter()
                .map(|oid| format!("--contains={oid}").into()),
        );
        args.push("refs/remotes".into());
        let borrowed: Vec<&std::ffi::OsStr> = args.iter().map(|arg| arg.as_os_str()).collect();
        let out = git.read(root, &borrowed, 65536)?.bytes;
        String::from_utf8_lossy(&out)
            .lines()
            .map(str::to_owned)
            .collect()
    };
    // Validate the entire replay range before prediction: a first conflict must
    // not hide a later submodule-pointer change that native Continue would replay.
    if blocked.is_none() {
        for oid in &original_commits {
            let original_parents = parents.get(oid).cloned().unwrap_or_default();
            let baselines = if original_parents.is_empty() {
                vec![None]
            } else {
                original_parents.iter().map(Some).collect()
            };
            for parent in baselines {
                let mut args: Vec<OsString> = [
                    "diff-tree",
                    "--no-commit-id",
                    "--raw",
                    "--no-renames",
                    "-r",
                    "-z",
                ]
                .into_iter()
                .map(Into::into)
                .collect();
                if let Some(parent) = parent {
                    args.push(parent.into());
                } else {
                    args.push("--root".into());
                }
                args.push(oid.into());
                let raw = git
                    .read(
                        root,
                        &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                        1024 * 1024,
                    )?
                    .bytes;
                if !gitlink_changes(&raw)?.is_empty() {
                    blocked=Some("This rebase changes submodule pointers. Review those histories separately before rebasing.".into());
                    break;
                }
            }
            if blocked.is_some() {
                break;
            }
        }
    }
    let mut files = Vec::new();
    let mut predicted = Vec::new();
    let mut result_tree = None;
    if blocked.is_none() {
        let scratch = Scratch::new(git, root)?;
        if replay_root {
            scratch.run(&["mktree".into()], Some(b""), false)?;
        }
        let mut mapped = BTreeMap::<String, String>::new();
        let replay = if request.rebase_steps.is_empty() {
            original_commits.clone()
        } else {
            request
                .rebase_steps
                .iter()
                .filter(|s| s.action != "drop")
                .map(|s| s.oid.clone())
                .collect()
        };
        let mut cursor = request.source_oid.clone();
        for (replay_index, oid) in replay.iter().enumerate() {
            let original_parents = parents.get(oid).cloned().unwrap_or_default();
            let rewritten = if !preserve_merges {
                vec![cursor.clone()]
            } else if original_parents.is_empty() {
                vec![request.source_oid.clone()]
            } else {
                original_parents
                    .iter()
                    .map(|p| {
                        mapped.get(p).cloned().unwrap_or_else(|| {
                            if p == &base {
                                request.source_oid.clone()
                            } else {
                                p.clone()
                            }
                        })
                    })
                    .collect::<Vec<_>>()
            };
            let (ours, incoming) = if original_parents.len() >= 2 {
                (rewritten[0].clone(), rewritten[1].clone())
            } else {
                let parent = replay_parent(git, root, oid)?;
                let old_tree =
                    scratch.text(&["rev-parse", &format!("{}^{{tree}}", parent.trim())])?;
                let new_tree = scratch.text(&["rev-parse", &format!("{oid}^{{tree}}")])?;
                let virtual_base = scratch.commit(old_tree.trim(), None)?;
                let cursor_tree =
                    scratch.text(&["rev-parse", &format!("{}^{{tree}}", rewritten[0])])?;
                (
                    scratch.commit(cursor_tree.trim(), Some(&virtual_base))?,
                    scratch.commit(new_tree.trim(), Some(&virtual_base))?,
                )
            };
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
            let mut tree = std::str::from_utf8(parts.next().unwrap_or_default())
                .map_err(|_| err("Invalid preview tree."))?
                .trim()
                .to_owned();
            crate::validate_oid(&tree)?;
            predicted = parts
                .filter(|b| !b.is_empty())
                .map(|b| Ok(NativePath::new(&crate::path_from_bytes(b.to_vec())?)))
                .collect::<Result<Vec<_>>>()?;
            if predicted.is_empty() && original_parents.len() > 2 {
                for n in 2..rewritten.len() {
                    let mut args = vec!["commit-tree".into(), tree.clone().into()];
                    for parent in &rewritten[..n] {
                        args.extend(["-p".into(), parent.into()]);
                    }
                    let accumulated = String::from_utf8(scratch.run(
                        &args,
                        Some(b"GitNova isolated octopus preview\n"),
                        false,
                    )?)
                    .map_err(|_| err("Invalid preview commit."))?;
                    let output = scratch.run(
                        &[
                            "merge-tree".into(),
                            "--write-tree".into(),
                            "--name-only".into(),
                            "--no-messages".into(),
                            "-z".into(),
                            accumulated.trim().into(),
                            rewritten[n].clone().into(),
                        ],
                        None,
                        true,
                    )?;
                    let mut parts = output.split(|b| *b == 0);
                    tree = std::str::from_utf8(parts.next().unwrap_or_default())
                        .map_err(|_| err("Invalid preview tree."))?
                        .trim()
                        .to_owned();
                    crate::validate_oid(&tree)?;
                    predicted = parts
                        .filter(|b| !b.is_empty())
                        .map(|b| Ok(NativePath::new(&crate::path_from_bytes(b.to_vec())?)))
                        .collect::<Result<Vec<_>>>()?;
                    if !predicted.is_empty() {
                        break;
                    }
                }
            }
            if !predicted.is_empty() {
                if parents.values().any(|p| p.len() > 2) {
                    blocked=Some("Octopus replay requires a conflict-free prediction. Review interacting changes with Git.".into());
                }
                break;
            }
            let mut command = vec!["commit-tree".into(), tree.clone().into()];
            for parent in &rewritten {
                command.extend(["-p".into(), parent.into()]);
            }
            let value = String::from_utf8(scratch.run(
                &command,
                Some(b"GitNova isolated topology preview\n"),
                false,
            )?)
            .map_err(|_| err("Invalid preview commit."))?;
            mapped.insert(oid.clone(), value.trim().into());
            cursor = value.trim().to_owned();
            if replay_index + 1 == replay.len() {
                result_tree = Some(tree.to_string());
            }
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
        subjects,
        tip: original.clone(),
        root: replay_root,
        preserve_merges,
        parents,
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
fn replay_parent(git: &GitCli, root: &Path, oid: &str) -> Result<String> {
    let row = git.text(root, &["rev-list", "--parents", "-n", "1", oid])?;
    let fields = row.split_whitespace().collect::<Vec<_>>();
    if fields.len() == 2 {
        return Ok(fields[1].into());
    }
    if fields.len() != 1 {
        return Err(err("Merge replay needs topology-preserving review."));
    }
    // hash-object without -w computes the repository's object-format-specific empty tree.
    let out = git.read_input(
        root,
        &[
            "hash-object".as_ref(),
            "-t".as_ref(),
            "tree".as_ref(),
            "--stdin".as_ref(),
        ],
        b"",
        128,
    )?;
    let oid = String::from_utf8(out.bytes).map_err(|_| err("Invalid empty tree revision."))?;
    crate::validate_oid(oid.trim())?;
    Ok(oid.trim().into())
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
        "rebase.instructionFormat=%s",
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
    if !p.request.rebase_steps.is_empty() {
        let editor = replay_editor(&dir, &p.request.rebase_steps)?;
        command.pop();
        command.extend([
            "-c".into(),
            format!("sequence.editor={editor}").into(),
            "rebase".into(),
            "--interactive".into(),
        ]);
    }
    command.extend(
        [
            "--merge",
            "--no-autostash",
            "--no-update-refs",
            "--no-autosquash",
            if plan.preserve_merges {
                "--rebase-merges=no-rebase-cousins"
            } else {
                "--no-rebase-merges"
            },
            "--reapply-cherry-picks",
            "--keep-empty",
            "--empty=ask",
            "--onto",
            &p.request.source_oid,
        ]
        .into_iter()
        .map(Into::into),
    );
    if plan.root {
        command.push("--root".into());
    } else {
        command.push(plan.base_oid.clone().into());
    }
    command.push(p.branch.clone().into());
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
            if action.kind == "skip"
                && marker
                    .rebase
                    .as_ref()
                    .is_some_and(|j| j.plan.preserve_merges)
            {
                let stopped =
                    git.optional_text(root, &["rev-parse", "--verify", "--quiet", "REBASE_HEAD"])?;
                if stopped
                    .as_ref()
                    .and_then(|s| marker.rebase.as_ref()?.plan.parents.get(s.trim()))
                    .is_some_and(|p| p.len() > 1)
                {
                    blocked=Some("Skipping a recreated merge would change topology. Continue its reviewed result or abort.".into());
                }
            }
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
        let removed = journal.plan.commits[index].clone();
        let old_parents = journal.plan.parents.remove(&removed).unwrap_or_default();
        let replacement = old_parents
            .first()
            .cloned()
            .unwrap_or_else(|| marker.request.source_oid.clone());
        if journal.plan.tip == removed {
            journal.plan.tip = replacement.clone();
        }
        for parents in journal.plan.parents.values_mut() {
            for parent in parents {
                if parent == &removed {
                    *parent = replacement.clone();
                }
            }
        }
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

fn replay_editor(dir: &Path, steps: &[RebaseEdit]) -> Result<String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let recipe = dir.join("reviewed-todo");
        let script = dir.join("apply-reviewed-todo.sh");
        let text = steps
            .iter()
            .map(|s| format!("{} {}\n", s.action, s.oid))
            .collect::<String>();
        recovery::private_file(&recipe, text.as_bytes())?;
        let quote = |p: &Path| -> Result<String> {
            let s = p
                .to_str()
                .ok_or_else(|| err("Replay editor paths must be UTF-8."))?;
            Ok(format!("'{}'", s.replace('\'', "'\\''")))
        };
        recovery::private_file(
            &script,
            format!("#!/bin/sh\nexec /bin/cat {} > \"$1\"\n", quote(&recipe)?).as_bytes(),
        )?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700))?;
        quote(&script)
    }
    #[cfg(not(unix))]
    {
        let _ = (dir, steps);
        Err(err(
            "Interactive replay currently requires a Unix Git environment.",
        ))
    }
}
