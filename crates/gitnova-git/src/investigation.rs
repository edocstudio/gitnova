//! Bounded offline archaeology and branch comparison. No persistent code index.
use super::*;
fn invalid(message: &str) -> AppError {
    AppError::new("investigation", message)
}
fn parse(bytes: &[u8]) -> Result<Vec<Commit>> {
    let fields = bytes
        .strip_suffix(&[0])
        .unwrap_or(bytes)
        .split(|b| *b == 0)
        .collect::<Vec<_>>();
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    if !fields.len().is_multiple_of(6) {
        return Err(invalid("Invalid history record."));
    }
    fields
        .as_chunks::<6>()
        .0
        .iter()
        .map(|record| parse_commit(record))
        .collect()
}
impl GitCli {
    pub fn validate_investigation(&self, r: &InvestigationRequest) -> Result<()> {
        if !matches!(r.mode.as_str(), "history" | "compare") || r.selection.cursor.is_some() {
            return Err(invalid("Choose file/code history or a branch comparison."));
        }
        if r.mode == "history" {
            if (r.path.is_empty() && r.needle.trim().is_empty())
                || r.path.len() > 1024
                || r.needle.len() > 256
                || r.path.contains('\\')
                || r.path.starts_with('/')
                || r.path.starts_with(':')
                || r.path.split('/').any(|p| p == "..")
                || r.path.chars().any(char::is_control)
                || r.needle.contains('\0')
            {
                return Err(invalid(
                    "Enter a repository-relative literal path or a code phrase (up to 256 bytes).",
                ));
            }
        } else {
            for value in [&r.source, &r.target] {
                self.investigation_ref(value)?;
            }
        }
        Ok(())
    }
    fn investigation_ref(&self, value: &str) -> Result<String> {
        if value == "HEAD" {
            return Ok(value.into());
        }
        let name = if value.starts_with("refs/heads/")
            || value.starts_with("refs/remotes/")
            || value.starts_with("refs/tags/")
        {
            value.to_string()
        } else {
            format!("refs/heads/{value}")
        };
        // Explicit names only. Do not accept rev expressions or ref globs.
        if value.is_empty()
            || value.starts_with('-')
            || (value.starts_with("refs/")
                && !value.starts_with("refs/heads/")
                && !value.starts_with("refs/remotes/")
                && !value.starts_with("refs/tags/"))
            || value.len() > 256
            || name.contains([' ', '\\', '~', '^', ':', '?', '*', '['])
            || name.contains("..")
            || name.contains("@{")
            || name.chars().any(char::is_control)
            || name.ends_with(['/', '.'])
            || name
                .split('/')
                .any(|s| s.is_empty() || s.starts_with('.') || s.ends_with(".lock"))
        {
            return Err(invalid(
                "Enter an exact local branch or full refs/heads, refs/remotes or refs/tags name.",
            ));
        }
        Ok(name)
    }
    pub fn investigate_history(
        &self,
        root: &Path,
        r: &InvestigationRequest,
        evidence: &mut InvestigationEvidence,
    ) -> Result<Vec<Commit>> {
        self.validate_investigation(r)?;
        let snapshot = self.fingerprint(root)?;
        let status = self.status(root)?;
        evidence.dirty = (!status.bare).then_some(status.dirty);
        evidence.shallow = self
            .text(root, &["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true";
        let format = "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s";
        let mut args: Vec<OsString> = [
            "--literal-pathspecs",
            "log",
            "--no-show-signature",
            "--no-decorate",
            "--no-ext-diff",
            "--no-textconv",
            "--diff-merges=first-parent",
            "--no-patch",
            "-z",
            format,
            "--max-count=101",
        ]
        .iter()
        .map(OsString::from)
        .collect();
        let bytes = if r.mode == "compare" {
            let resolve = |value: &str| -> Result<String> {
                let name = self.investigation_ref(value)?;
                let oid = self
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
                validate_oid(&oid)?;
                Ok(oid)
            };
            let source = resolve(&r.source)?;
            let target = resolve(&r.target)?;
            let counts = self.text(
                root,
                &[
                    "rev-list",
                    "--left-right",
                    "--count",
                    &format!("{source}...{target}"),
                    "--",
                ],
            )?;
            let fields = counts
                .split_whitespace()
                .map(str::parse::<usize>)
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| invalid("Invalid branch counts."))?;
            if fields.len() != 2 {
                return Err(invalid("Invalid branch counts."));
            }
            evidence.source_only = Some(fields[0]);
            evidence.target_only = Some(fields[1]);
            let files = self.read(
                root,
                &[
                    "diff".as_ref(),
                    "--no-ext-diff".as_ref(),
                    "--no-textconv".as_ref(),
                    "--no-renames".as_ref(),
                    "--name-only".as_ref(),
                    "-z".as_ref(),
                    source.as_ref(),
                    target.as_ref(),
                    "--".as_ref(),
                ],
                2 * 1024 * 1024,
            )?;
            evidence.changed_files = Some(
                files
                    .bytes
                    .split(|b| *b == 0)
                    .filter(|s| !s.is_empty())
                    .count(),
            );
            evidence.source_oid = Some(source.clone());
            evidence.target_oid = Some(target.clone());
            evidence.scanned = fields[0].min(101);
            args.extend([
                "--date-order".into(),
                format!("{target}..{source}").into(),
                "--".into(),
            ]);
            self.read(
                root,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                1024 * 1024,
            )?
            .bytes
        } else {
            // Enumerate a bounded candidate set before potentially expensive pickaxe.
            // --no-walk ensures older history cannot be scanned accidentally.
            let mut candidates = self.history_range(root, 0, 2001)?;
            evidence.limited = candidates.len() > 2000;
            candidates.truncate(2000);
            evidence.scanned = candidates.len();
            if candidates.is_empty() {
                return Ok(vec![]);
            }
            args.extend(["--no-walk=unsorted".into(), "--stdin".into()]);
            if !r.needle.is_empty() {
                args.push(format!("-S{}", r.needle).into());
            }
            args.push("--".into());
            if !r.path.is_empty() {
                args.push(r.path.clone().into());
            }
            let input = candidates
                .iter()
                .map(|c| format!("{}\n", c.oid))
                .collect::<String>();
            self.read_input(
                root,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                input.as_bytes(),
                1024 * 1024,
            )?
            .bytes
        };
        let mut commits = parse(&bytes)?;
        evidence.limited |= commits.len() > 100;
        commits.truncate(100);
        if self.fingerprint(root)? != snapshot {
            return Err(invalid(
                "Repository refs changed. Run the investigation again.",
            ));
        }
        Ok(commits)
    }
}
