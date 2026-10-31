//! Git credential-protocol adapter for the same host accounts used by provider APIs.
//! Only HTTPS, exact authorities and segment-aligned host base paths are eligible.
use super::credential_account;
use super::{host_context, Credential};
use gitnova_domain::*;
use std::collections::BTreeMap;
use url::Url;

pub fn parse_request(input: &[u8]) -> Result<BTreeMap<String, String>> {
    let fail = || AppError::new("git_auth", "Invalid Git credential request.");
    if input.len() > 16384 || input.contains(&0) {
        return Err(fail());
    }
    let text = std::str::from_utf8(input).map_err(|_| fail())?;
    let mut fields = BTreeMap::new();
    for (position, line) in text.lines().enumerate() {
        if line.is_empty() {
            break;
        }
        if position >= 64 {
            return Err(fail());
        }
        let (key, value) = line.split_once('=').ok_or_else(fail)?;
        if key.is_empty()
            || key.chars().any(char::is_control)
            || value.chars().any(char::is_control)
            || fields.len() >= 64
        {
            return Err(fail());
        }
        if matches!(key, "protocol" | "host" | "path" | "username") && fields.contains_key(key) {
            return Err(fail());
        }
        fields.insert(key.to_owned(), value.to_owned());
    }
    Ok(fields)
}

pub fn request_matches_targets(input: &[u8], targets: &[String]) -> Result<bool> {
    let fields = parse_request(input)?;
    if fields.get("protocol").map(String::as_str) != Some("https") {
        return Ok(false);
    }
    let Some(host) = fields.get("host") else {
        return Ok(false);
    };
    if host.contains(['/', '\\', '@', '?', '#']) {
        return Ok(false);
    }
    let path = fields.get("path").map_or("", String::as_str);
    if path.starts_with('/')
        || path.contains(['?', '#', '\\'])
        || path.split('/').any(|v| matches!(v, "." | ".."))
    {
        return Ok(false);
    }
    let Ok(request) = Url::parse(&format!("https://{host}/{path}")) else {
        return Ok(false);
    };
    Ok(targets.iter().take(32).any(|raw| {
        Url::parse(raw).is_ok_and(|target| {
            target.scheme() == "https"
                && target.username().is_empty()
                && target.password().is_none()
                && target.query().is_none()
                && target.fragment().is_none()
                && target.origin() == request.origin()
                && target.path().trim_end_matches('/') == request.path().trim_end_matches('/')
        })
    }))
}

fn response_with(
    input: &[u8],
    mut lookup: impl FnMut(&HostingContext) -> Result<Option<Credential>>,
) -> Result<Option<Vec<u8>>> {
    let fields = parse_request(input)?;
    if fields.get("protocol").map(String::as_str) != Some("https") {
        return Ok(None);
    }
    let Some(host) = fields.get("host") else {
        return Ok(None);
    };
    if host.is_empty()
        || host
            .chars()
            .any(|c| matches!(c, '/' | '\\' | '@' | '?' | '#' | '\'' | '"'))
    {
        return Ok(None);
    }
    let base = Url::parse(&format!("https://{host}/"))
        .map_err(|_| AppError::new("git_auth", "Invalid Git credential host."))?;
    if base.host_str().is_none() || base.password().is_some() || !base.username().is_empty() {
        return Ok(None);
    }
    let path = fields.get("path").map_or("", String::as_str);
    if path.starts_with('/')
        || path.contains(['?', '#', '\\'])
        || path.split('/').any(|v| matches!(v, "." | ".."))
    {
        return Ok(None);
    }
    // The last segment is a repository, not a hosting base path. Longest base
    // wins; never fall through a denied/ambiguous more-specific connection.
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() > 17 {
        return Ok(None);
    }
    for end in (0..segments.len()).rev() {
        let origin = format!(
            "{}{}",
            base.as_str().trim_end_matches('/'),
            if end == 0 {
                String::new()
            } else {
                format!("/{}", segments[..end].join("/"))
            }
        );
        let providers: &[&str] = match base.host_str() {
            Some("github.com") => &["github"],
            Some("gitlab.com") => &["gitlab"],
            _ => &["github", "gitlab"],
        };
        let mut found = Vec::new();
        for provider in providers {
            let context = host_context(&HostProfile {
                provider: (*provider).into(),
                origin: origin.clone(),
            })?;
            if let Some(credential) = lookup(&context)? {
                found.push(credential);
            }
        }
        if found.len() > 1 {
            return Err(AppError::new(
                "git_auth",
                "More than one saved provider account matches this Git host.",
            ));
        }
        if let Some(credential) = found.pop() {
            if credential.login.is_empty()
                || credential.token.is_empty()
                || credential.login.len() > 256
                || credential.token.len() > 8192
                || credential.login.chars().any(char::is_control)
                || credential.token.chars().any(char::is_control)
            {
                return Err(AppError::new("git_auth", "The saved account does not match the Git credential request. Check the host account and configured Git username."));
            }
            return Ok(Some(
                format!(
                    "username={}\npassword={}\n\n",
                    credential.login, credential.token
                )
                .into_bytes(),
            ));
        }
    }
    Ok(None)
}

pub fn response(input: &[u8]) -> Result<Option<Vec<u8>>> {
    response_with(input, |context| {
        let account = credential_account(context);
        #[cfg(target_os = "linux")]
        let bytes = if std::env::var_os("GITNOVA_BACKGROUND_FETCH").is_some() {
            crate::credential_store::read_background("gitnova.hosting.v1", &account)?
        } else {
            crate::credential_store::read("gitnova.hosting.v1", &account)?
        };
        #[cfg(not(target_os = "linux"))]
        let bytes = crate::credential_store::read("gitnova.hosting.v1", &account)?;
        bytes
            .map(|bytes| {
                serde_json::from_slice(&bytes)
                    .map_err(|_| AppError::new("git_auth", "Reconnect the saved host account."))
            })
            .transpose()
    })
}

/// Used only inside the separate Git credential-helper process.
#[cfg(target_os = "macos")]
pub fn disable_background_interaction(
) -> Result<security_framework::os::macos::keychain::KeychainUserInteractionLock> {
    security_framework::os::macos::keychain::SecKeychain::disable_user_interaction().map_err(|_| {
        AppError::new(
            "git_auth",
            "Background Keychain access is unavailable. Fetch manually to authorize access.",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn saved() -> Credential {
        Credential {
            login: "fixture".into(),
            token: "test-fixture-token".into(),
        }
    }
    #[test]
    fn exact_host_and_longest_base_select_the_connected_account() {
        let mut seen = vec![];
        let output = response_with(
            b"protocol=https\nhost=git.example\npath=platform/team/repo.git\n\n",
            |c| {
                seen.push((c.provider.clone(), c.origin.clone()));
                Ok(
                    (c.provider == "gitlab" && c.origin == "https://git.example/platform")
                        .then(saved),
                )
            },
        )
        .unwrap()
        .unwrap();
        assert!(String::from_utf8(output)
            .unwrap()
            .contains("password=test-fixture-token\n"));
        assert!(!seen.iter().any(|(_, o)| o == "https://git.example"));
    }
    #[test]
    fn insecure_or_malformed_requests_never_query_keychain() {
        for input in [
            "protocol=http\nhost=git.example\n\n",
            "protocol=https\nhost=git.example@evil.example\n\n",
            "protocol=https\nhost=git.example\npath=../repo\n\n",
        ] {
            assert!(
                response_with(input.as_bytes(), |_| panic!("No account lookup allowed"))
                    .unwrap()
                    .is_none()
            );
        }
        assert!(parse_request(b"protocol=https\nhost=one\nhost=two\n\n").is_err());
        assert!(parse_request(&vec![b'a'; 16385]).is_err());
    }
    #[test]
    fn company_port_and_base_are_not_inferred_from_substrings() {
        let output = response_with(
            b"protocol=https\nhost=git.example.evil:8443\npath=platform/team/repo.git\n\n",
            |c| Ok((c.origin == "https://git.example/platform").then(saved)),
        )
        .unwrap();
        assert!(output.is_none());
    }
    #[test]
    fn credential_requests_cannot_escape_the_selected_transport_destination() {
        let targets = vec!["https://git.example:8443/platform/team/repo.git".into()];
        assert!(request_matches_targets(
            b"protocol=https\nhost=git.example:8443\npath=platform/team/repo.git\n\n",
            &targets
        )
        .unwrap());
        for input in [
            "protocol=https\nhost=git.example\npath=platform/team/repo.git\n\n",
            "protocol=https\nhost=other.example:8443\npath=platform/team/repo.git\n\n",
            "protocol=https\nhost=git.example:8443\npath=platform/other.git\n\n",
        ] {
            assert!(!request_matches_targets(input.as_bytes(), &targets).unwrap());
        }
    }
    #[test]
    fn ambiguous_accounts_and_denied_access_fail_closed_but_saved_login_wins() {
        assert!(
            response_with(b"protocol=https\nhost=git.example\n\n", |_| Ok(Some(
                saved()
            )))
            .is_err()
        );
        let response = response_with(
            b"protocol=https\nhost=github.com\nusername=another\n\n",
            |_| Ok(Some(saved())),
        )
        .unwrap()
        .unwrap();
        assert!(String::from_utf8(response)
            .unwrap()
            .contains("username=fixture\n"));
        assert!(
            response_with(b"protocol=https\nhost=github.com\n\n", |_| Err(
                AppError::new("denied", "denied")
            ))
            .is_err()
        );
    }
}
