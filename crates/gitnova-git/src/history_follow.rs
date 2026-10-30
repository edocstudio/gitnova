//! Native rename-following history, with exact historical paths for navigation.
use super::*;
fn error(message: &str) -> AppError {
    AppError::new("investigation", message)
}
impl GitCli {
    pub(crate) fn follow_file_history(
        &self,
        root: &Path,
        r: &InvestigationRequest,
        evidence: &mut InvestigationEvidence,
    ) -> Result<Vec<Commit>> {
        let snapshot = self.fingerprint(root)?;
        let name = self.investigation_ref(if r.source.is_empty() {
            "HEAD"
        } else {
            &r.source
        })?;
        let revision = self
            .text(
                root,
                &[
                    "rev-parse",
                    "--verify",
                    "--end-of-options",
                    &format!("{name}^{{commit}}"),
                ],
            )?
            .trim()
            .to_string();
        validate_oid(&revision)?;
        let entry = self
            .read(
                root,
                &[
                    "--literal-pathspecs".as_ref(),
                    "ls-tree".as_ref(),
                    "-z".as_ref(),
                    revision.as_ref(),
                    "--".as_ref(),
                    r.path.as_ref(),
                ],
                65536,
            )?
            .bytes;
        let exact_path = entry
            .split(|b| *b == b'\t')
            .nth(1)
            .and_then(|v| v.strip_suffix(&[0]));
        if exact_path != Some(r.path.as_bytes()) {
            return Err(error(
                "Follow history requires one exact file path, not a directory pathspec.",
            ));
        }
        if entry.is_empty()
            || ![b"100644 ".as_slice(), b"100755 ".as_slice()]
                .iter()
                .any(|mode| entry.starts_with(mode))
            || entry.iter().filter(|v| **v == 0).count() != 1
        {
            return Err(error(
                "Follow history requires a regular file present at the selected revision.",
            ));
        }
        let out = self
            .read(
                root,
                &[
                    "--literal-pathspecs".as_ref(),
                    "log".as_ref(),
                    "--follow".as_ref(),
                    "--no-color".as_ref(),
                    "--no-show-signature".as_ref(),
                    "--no-decorate".as_ref(),
                    "--no-ext-diff".as_ref(),
                    "--no-textconv".as_ref(),
                    "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s".as_ref(),
                    "-z".as_ref(),
                    "--name-status".as_ref(),
                    "--max-count=101".as_ref(),
                    revision.as_ref(),
                    "--".as_ref(),
                    r.path.as_ref(),
                ],
                2 * 1024 * 1024,
            )?
            .bytes;
        let fields = out.split(|v| *v == 0).collect::<Vec<_>>();
        let mut pos = 0;
        let mut path = r.path.clone();
        let mut commits = Vec::new();
        while pos < fields.len() {
            while pos < fields.len() && fields[pos].is_empty() {
                pos += 1;
            }
            if pos == fields.len() {
                break;
            }
            if pos + 6 > fields.len() {
                return Err(error("Incomplete rename history metadata."));
            }
            let commit = parse_commit(&fields[pos..pos + 6])?;
            pos += 6;
            let current = path.clone();
            let mut previous = None;
            while pos < fields.len() {
                let status = fields[pos].strip_prefix(b"\n").unwrap_or(fields[pos]);
                if status.is_empty() {
                    pos += 1;
                    continue;
                }
                if status.len() == 40 || status.len() == 64 {
                    break;
                }
                if !matches!(
                    status.first(),
                    Some(b'A' | b'M' | b'D' | b'T' | b'R' | b'C')
                ) {
                    return Err(error("Unexpected rename history status."));
                }
                pos += 1;
                let count = if matches!(status.first(), Some(b'R' | b'C')) {
                    2
                } else {
                    1
                };
                if pos + count > fields.len() {
                    return Err(error("Incomplete rename history path."));
                }
                let first = std::str::from_utf8(fields[pos])
                    .map_err(|_| error("Non-UTF-8 historical paths need native Git inspection."))?;
                if count == 2 {
                    let next = std::str::from_utf8(fields[pos + 1]).map_err(|_| {
                        error("Non-UTF-8 historical paths need native Git inspection.")
                    })?;
                    if next == current && status.first() == Some(&b'R') {
                        previous = Some(first.to_owned());
                    }
                }
                pos += count;
            }
            evidence
                .history_paths
                .insert(commit.oid.clone(), NativePath::new(Path::new(&current)));
            commits.push(commit);
            if let Some(old) = previous {
                path = old;
            }
        }
        evidence.limited = commits.len() > 100;
        commits.truncate(100);
        evidence.scanned = commits.len();
        evidence.source_oid = Some(revision);
        evidence.dirty = Some(self.status(root)?.dirty);
        evidence.shallow = self
            .text(root, &["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true";
        if self.fingerprint(root)? != snapshot {
            return Err(error(
                "Repository refs changed. Run the investigation again.",
            ));
        }
        Ok(commits)
    }
}
