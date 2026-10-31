//! Bounded native line attribution for one committed regular UTF-8 file.
use super::*;
fn invalid(s: &str) -> AppError {
    AppError::new("investigation", s)
}
fn filename(text: &str) -> Result<String> {
    if !text.starts_with('"') {
        return Ok(text.into());
    }
    let raw = text
        .strip_prefix('"')
        .and_then(|t| t.strip_suffix('"'))
        .ok_or_else(|| invalid("Invalid blame filename."))?
        .as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < raw.len() {
        if raw[i] != b'\\' {
            out.push(raw[i]);
            i += 1;
            continue;
        }
        i += 1;
        if i >= raw.len() {
            return Err(invalid("Invalid blame filename escape."));
        }
        let value = match raw[i] {
            b'0'..=b'3'
                if i + 2 < raw.len()
                    && raw[i + 1..=i + 2].iter().all(|c| matches!(c, b'0'..=b'7')) =>
            {
                let v = (raw[i] - b'0') * 64 + (raw[i + 1] - b'0') * 8 + raw[i + 2] - b'0';
                i += 2;
                v
            }
            b't' => b'\t',
            b'n' => b'\n',
            b'r' => b'\r',
            b'\\' => b'\\',
            b'"' => b'"',
            _ => return Err(invalid("Unsupported blame filename encoding.")),
        };
        out.push(value);
        i += 1;
    }
    String::from_utf8(out)
        .map_err(|_| invalid("Non-UTF-8 historical filenames require inspection with Git."))
}
impl GitCli {
    pub(crate) fn blame_file(
        &self,
        root: &Path,
        r: &InvestigationRequest,
        e: &mut InvestigationEvidence,
    ) -> Result<Vec<Commit>> {
        let before = self.fingerprint(root)?;
        let config = self.config_snapshot(root)?;
        if self
            .optional_text(root, &["config", "--get-all", "blame.ignoreRevsFile"])?
            .is_some_and(|v| v.lines().any(|line| !line.is_empty()))
        {
            return Err(invalid(
                "Configured blame ignore files require inspection with Git.",
            ));
        }
        let reference = self.investigation_ref(if r.source.is_empty() {
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
                    &format!("{reference}^{{commit}}"),
                ],
            )?
            .trim()
            .to_owned();
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
        let fields = entry.split(|b| *b == b'\t').collect::<Vec<_>>();
        if fields.len() != 2
            || fields[1].strip_suffix(&[0]) != Some(r.path.as_bytes())
            || entry.iter().filter(|b| **b == 0).count() != 1
            || !(entry.starts_with(b"100644 ") || entry.starts_with(b"100755 "))
        {
            return Err(invalid(
                "Line attribution requires one regular committed file.",
            ));
        }
        let blob = std::str::from_utf8(fields[0])
            .map_err(|_| invalid("Invalid file entry."))?
            .split_whitespace()
            .nth(2)
            .ok_or_else(|| invalid("Invalid file entry."))?;
        validate_oid(blob)?;
        let size = self
            .text(root, &["cat-file", "-s", blob])?
            .trim()
            .parse::<usize>()
            .map_err(|_| invalid("Invalid blob size."))?;
        if size > 1024 * 1024 {
            return Err(invalid(
                "Line attribution is limited to text files up to 1 MiB.",
            ));
        }
        let content = self
            .read(
                root,
                &["cat-file".as_ref(), "blob".as_ref(), blob.as_ref()],
                1024 * 1024,
            )?
            .bytes;
        if content.contains(&0) {
            return Err(invalid("Binary files have no text line attribution."));
        }
        let text = std::str::from_utf8(&content)
            .map_err(|_| invalid("Line attribution requires UTF-8 text."))?;
        let count = text.lines().count() as u32;
        let start = r.line_start.unwrap_or(1);
        let end = r.line_end.unwrap_or(200).min(count);
        if count == 0 || start > count {
            return Err(invalid(
                "The selected line range is outside this revision's file.",
            ));
        }
        let output = self.text(
            root,
            &[
                "-c",
                "core.quotePath=true",
                "blame",
                "--line-porcelain",
                "--root",
                "--no-textconv",
                "-L",
                &format!("{start},{end}"),
                &revision,
                "--",
                &r.path,
            ],
        )?;
        let mut records = output.lines();
        let mut lines = Vec::new();
        while let Some(header) = records.next() {
            let parts = header.split_whitespace().collect::<Vec<_>>();
            if parts.len() < 3 || parts.len() > 4 {
                return Err(invalid("Invalid blame header."));
            }
            validate_oid(parts[0])?;
            let original_line = parts[1]
                .parse::<u32>()
                .map_err(|_| invalid("Invalid original line."))?;
            let line = parts[2]
                .parse::<u32>()
                .map_err(|_| invalid("Invalid attributed line."))?;
            if line != start + lines.len() as u32 || line > end || original_line == 0 {
                return Err(invalid("Invalid blame range."));
            }
            let mut path = None;
            let mut body = None;
            for field in records.by_ref() {
                if let Some(value) = field.strip_prefix("filename ") {
                    path = Some(filename(value)?);
                }
                if let Some(value) = field.strip_prefix('\t') {
                    body = Some(value.to_string());
                    break;
                }
            }
            let path = path.ok_or_else(|| invalid("Missing blame filename."))?;
            if path.starts_with('/') || path.split('/').any(|p| p == "..") {
                return Err(invalid("Invalid historical filename."));
            }
            let path = NativePath::new(Path::new(&path));
            e.history_paths
                .entry(parts[0].to_owned())
                .or_insert(path.clone());
            lines.push(LineAttribution {
                oid: parts[0].into(),
                path,
                line,
                original_line,
                content: body.ok_or_else(|| invalid("Missing blamed content."))?,
            });
            if lines.len() > 200 {
                return Err(invalid("Line attribution exceeds its range budget."));
            }
        }
        if lines.len() as u32 != end - start + 1 {
            return Err(invalid("Incomplete line attribution."));
        }
        let ids = lines
            .iter()
            .map(|l| l.oid.clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut args: Vec<OsString> = [
            "log",
            "--no-walk",
            "--no-show-signature",
            "--format=%H%x00%P%x00%an%x00%ae%x00%at%x00%s",
            "-z",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        args.extend(ids.into_iter().map(Into::into));
        args.push("--".into());
        let raw = self
            .read(
                root,
                &args.iter().map(OsString::as_os_str).collect::<Vec<_>>(),
                2 * 1024 * 1024,
            )?
            .bytes;
        let commits = crate::investigation::parse(&raw)?;
        e.scanned = lines.len();
        e.line_attribution = lines;
        e.source_oid = Some(revision);
        e.dirty = Some(self.status(root)?.dirty);
        e.shallow = self
            .text(root, &["rev-parse", "--is-shallow-repository"])?
            .trim()
            == "true";
        if before != self.fingerprint(root)? || config != self.config_snapshot(root)? {
            return Err(invalid(
                "Repository refs changed. Run the investigation again.",
            ));
        }
        Ok(commits)
    }
}
