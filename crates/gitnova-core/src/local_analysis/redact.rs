//! Deliberately conservative, best-effort disclosure reduction, not a secret scanner.
use serde_json::Value;
pub(super) fn value(v: &mut Value) -> bool {
    match v {
        Value::String(s) => {
            let safe = redact(s);
            let changed = safe != *s;
            *s = safe;
            changed
        }
        Value::Array(a) => a.iter_mut().fold(false, |changed, v| value(v) | changed),
        Value::Object(o) => o.values_mut().fold(false, |changed, v| value(v) | changed),
        _ => false,
    }
}
fn redact(s: &str) -> String {
    let mut private = false;
    s.split_inclusive('\n')
        .map(|line| {
            let lower = line.to_ascii_lowercase();
            if lower.contains("-----begin ") && lower.contains("private key-----") {
                private = true;
            }
            let sensitive = private
                || [
                    "password",
                    "passwd",
                    "secret",
                    "api_key",
                    "apikey",
                    "access_key",
                    "authorization",
                    "credential",
                    "token",
                ]
                .iter()
                .any(|key| lower.contains(key) && (line.contains('=') || line.contains(':')))
                || [
                    "ghp_",
                    "github_pat_",
                    "glpat-",
                    "AKIA",
                    "ASIA",
                    "sk_live_",
                    "sk-proj-",
                ]
                .iter()
                .any(|prefix| line.contains(prefix))
                || line.split_whitespace().any(|w| {
                    (w.starts_with("https://") || w.starts_with("http://"))
                        && (w.contains('@') || w.contains('?'))
                });
            if lower.contains("-----end ") && lower.contains("private key-----") {
                private = false;
            }
            if sensitive {
                if line.ends_with('\n') {
                    "[REDACTED]\n".to_string()
                } else {
                    "[REDACTED]".into()
                }
            } else {
                line.to_string()
            }
        })
        .collect()
}
#[cfg(test)]
mod tests {
    use super::redact;
    #[test]
    fn masks_keys_assignments_urls_and_multiline_private_keys() {
        for secret in [
            "token = abc",
            "+ api_key: xyz",
            "ghp_abc",
            "https://user:pass@example.test/repo",
            "-----BEGIN PRIVATE KEY-----\nabc\n-----END PRIVATE KEY-----",
        ] {
            let r = redact(secret);
            assert!(!r.contains("abc"));
            assert!(!r.contains("xyz"));
            assert!(!r.contains("pass@"));
        }
        assert_eq!(redact("let count = 3;\n"), "let count = 3;\n");
    }
}
