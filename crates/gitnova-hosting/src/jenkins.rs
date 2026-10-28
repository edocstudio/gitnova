//! Explicit Jenkins job mapping. Only Git BuildData for this repository and
//! exact revision can annotate a commit. No build triggers, scripts or logs.
use crate::{err, valid_oid, Credential};
use gitnova_domain::*;
use reqwest::blocking::Client;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{io::Read, time::Duration};
use url::Url;

pub fn account_scope(controller: &str, username: &str) -> Result<Url> {
    let value = crate::validated_ci_url(controller)?;
    let mut url = Url::parse(&value).map_err(|_| err("Invalid Jenkins address."))?;
    if url.query().is_some()
        || url.fragment().is_some()
        || username.is_empty()
        || username.len() > 256
        || username.contains(':')
        || username.chars().any(char::is_control)
    {
        return Err(err(
            "Enter the Jenkins username and controller HTTPS address.",
        ));
    }
    if !url.path().ends_with('/') {
        url.set_path(&format!("{}/", url.path()));
    }
    Ok(url)
}
fn account_key(controller: &str, username: &str) -> Result<String> {
    Ok(format!(
        "{}:{}",
        account_scope(controller, username)?,
        username
    ))
}
#[cfg(target_os = "macos")]
pub fn account_credential(controller: &str, username: &str) -> Result<Credential> {
    use security_framework::passwords::{generic_password, PasswordOptions};
    let bytes = generic_password(PasswordOptions::new_generic_password(
        "gitnova.jenkins.v1",
        &account_key(controller, username)?,
    ))
    .map_err(|_| err("Connect this Jenkins account first."))?;
    serde_json::from_slice(&bytes).map_err(|_| err("Reconnect this Jenkins account."))
}
#[cfg(not(target_os = "macos"))]
pub fn account_credential(_: &str, _: &str) -> Result<Credential> {
    Err(err(
        "Provider account storage is currently supported on macOS.",
    ))
}
#[cfg(target_os = "macos")]
pub fn connect_account(
    controller: &str,
    username: &str,
    prompt: &CredentialPrompt,
) -> Result<String> {
    let url = account_scope(controller, username)?;
    let cred = Credential {
        token: crate::secure_token(prompt, &format!("Jenkins {username} at {url}"))?,
        login: username.into(),
    };
    verify_controller(&http_client()?, &cred, &url)?;
    security_framework::passwords::set_generic_password(
        "gitnova.jenkins.v1",
        &account_key(controller, username)?,
        &serde_json::to_vec(&cred).map_err(|_| err("Could not encode account credentials."))?,
    )
    .map_err(|_| err("Could not store this account in macOS Keychain."))?;
    Ok(cred.login)
}
#[cfg(not(target_os = "macos"))]
pub fn connect_account(_: &str, _: &str, _: &CredentialPrompt) -> Result<String> {
    Err(err(
        "Provider account storage is currently supported on macOS.",
    ))
}
#[cfg(target_os = "macos")]
pub fn disconnect_account(controller: &str, username: &str) -> Result<()> {
    security_framework::passwords::delete_generic_password(
        "gitnova.jenkins.v1",
        &account_key(controller, username)?,
    )
    .map_err(|_| err("Could not remove the saved account. Inspect macOS Keychain."))
}
#[cfg(not(target_os = "macos"))]
pub fn disconnect_account(_: &str, _: &str) -> Result<()> {
    Err(err(
        "Provider account storage is currently supported on macOS.",
    ))
}
pub fn validate(selection: &JenkinsSelection) -> Result<(Url, Url)> {
    let parse = |raw: &str| -> Result<Url> {
        if raw.len() > 4096 || raw.chars().any(char::is_control) {
            return Err(err("Enter a credential-free Jenkins HTTPS address."));
        }
        let url =
            Url::parse(raw).map_err(|_| err("Enter a credential-free Jenkins HTTPS address."))?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(err("Enter a credential-free Jenkins HTTPS address."));
        }
        Ok(url)
    };
    let mut controller = parse(&selection.controller)?;
    if !controller.path().ends_with('/') {
        controller.set_path(&format!("{}/", controller.path()));
    }
    let mut job = parse(&selection.job_url)?;
    if !job.path().ends_with('/') {
        job.set_path(&format!("{}/", job.path()));
    }
    if controller.origin() != job.origin() {
        return Err(err("The Jenkins job must belong to this controller."));
    }
    let suffix = job
        .path()
        .strip_prefix(controller.path())
        .ok_or_else(|| err("The Jenkins job must belong to this controller."))?;
    let segments = suffix.trim_end_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() < 2
        || segments.len() > 32
        || segments.len() % 2 != 0
        || segments.chunks(2).any(|p| p[0] != "job" || p[1].is_empty())
    {
        return Err(err(
            "Enter the full Jenkins job URL, including folder and branch jobs.",
        ));
    }
    if selection.username.is_empty()
        || selection.username.len() > 256
        || selection.username.contains(':')
        || selection.username.chars().any(char::is_control)
        || !valid_oid(&selection.oid)
    {
        return Err(err(
            "Enter the Jenkins username and select a full commit SHA.",
        ));
    }
    Ok((controller, job))
}
fn credential_account(s: &JenkinsSelection) -> Result<String> {
    let (controller, _) = validate(s)?;
    Ok(format!("{}:{}", controller, s.username))
}
#[cfg(target_os = "macos")]
pub fn credential(s: &JenkinsSelection) -> Result<Credential> {
    use security_framework::passwords::{generic_password, PasswordOptions};
    let bytes = generic_password(PasswordOptions::new_generic_password(
        "gitnova.jenkins.v1",
        &credential_account(s)?,
    ))
    .map_err(|_| err("Connect this Jenkins account first."))?;
    serde_json::from_slice(&bytes).map_err(|_| err("Reconnect this Jenkins account."))
}
#[cfg(not(target_os = "macos"))]
pub fn credential(_: &JenkinsSelection) -> Result<Credential> {
    Err(err(
        "Provider account storage is currently supported on macOS.",
    ))
}
#[cfg(target_os = "macos")]
pub fn connect(s: &JenkinsSelection, prompt: &CredentialPrompt) -> Result<String> {
    let (controller, _) = validate(s)?;
    let token = crate::secure_token(prompt, &format!("Jenkins {} at {}", s.username, controller))?;
    let cred = Credential {
        token,
        login: s.username.clone(),
    };
    let client = JenkinsClient::new(s, &cred)?;
    client.verify_account()?;
    security_framework::passwords::set_generic_password(
        "gitnova.jenkins.v1",
        &credential_account(s)?,
        &serde_json::to_vec(&cred).map_err(|_| err("Could not encode account credentials."))?,
    )
    .map_err(|_| err("Could not store this account in macOS Keychain."))?;
    Ok(cred.login)
}
#[cfg(not(target_os = "macos"))]
pub fn connect(_: &JenkinsSelection, _: &CredentialPrompt) -> Result<String> {
    Err(err(
        "Provider account storage is currently supported on macOS.",
    ))
}

pub fn cache_key(s: &JenkinsSelection, remote: &str, c: &Credential) -> Result<String> {
    let (controller, job) = validate(s)?;
    Ok(format!(
        "{:x}",
        Sha256::digest(
            serde_json::to_vec(&(
                controller.as_str(),
                job.as_str(),
                &s.oid,
                remote,
                &c.login,
                &c.token
            ))
            .map_err(|_| err("Could not identify pipeline scope."))?
        )
    ))
}
// Preserve path case and nonstandard ports. Do not guess SSH aliases/mirrors.
fn repository_key(raw: &str) -> Option<String> {
    if raw.len() > 4096 || raw.chars().any(char::is_control) {
        return None;
    }
    let url = if raw.contains("://") {
        Url::parse(raw).ok()?
    } else {
        let (host, path) = raw.split_once(':')?;
        Url::parse(&format!("ssh://{}/{}", host, path.trim_start_matches('/'))).ok()?
    };
    if !matches!(url.scheme(), "https" | "ssh")
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || (url.scheme() == "https" && !url.username().is_empty())
    {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    let path = url
        .path()
        .trim_matches('/')
        .strip_suffix(".git")
        .unwrap_or(url.path().trim_matches('/'));
    if path.is_empty() {
        return None;
    }
    let port = url.port().filter(|p| {
        !((url.scheme() == "ssh" && *p == 22) || (url.scheme() == "https" && *p == 443))
    });
    Some(format!(
        "{host}{}{path}",
        port.map_or("/".into(), |p| format!(":{p}/"))
    ))
}
fn http_client() -> Result<Client> {
    Client::builder()
        .use_native_tls()
        .user_agent("GitNova/0.1")
        .timeout(Duration::from_secs(20))
        .connect_timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .pool_max_idle_per_host(1)
        .build()
        .map_err(|_| err("Could not initialize the provider connection."))
}
fn get_json(
    client: &Client,
    credential: &Credential,
    url: Url,
    tree: Option<&str>,
) -> Result<Value> {
    let mut req = client
        .get(url)
        .basic_auth(&credential.login, Some(&credential.token))
        .header("Accept", "application/json");
    if let Some(tree) = tree {
        req = req.query(&[("tree", tree)]);
    }
    let response = req
        .send()
        .map_err(|_| err("Jenkins could not be reached. Check the controller and connection."))?;
    let status = response.status();
    if !status.is_success() {
        return Err(err(match status.as_u16() {
            401 | 403 => {
                "Jenkins denied access. Reconnect with an API token and job read permission."
            }
            404 => "Jenkins job was not found or is not accessible.",
            429 => "Jenkins rate limit reached. Wait before refreshing again.",
            300..=399 => "Jenkins redirected the request. Check the configured HTTPS address.",
            _ => "Jenkins could not return build status.",
        }));
    }
    let mut bytes = vec![];
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| err("Jenkins response could not be read."))?;
    if bytes.len() > 2 * 1024 * 1024 {
        return Err(err("Jenkins response exceeded the 2 MiB limit."));
    }
    serde_json::from_slice(&bytes).map_err(|_| err("Invalid Jenkins API response."))
}
fn verify_controller(client: &Client, credential: &Credential, controller: &Url) -> Result<()> {
    let v = get_json(
        client,
        credential,
        controller
            .join("whoAmI/api/json")
            .map_err(|_| err("Invalid Jenkins address."))?,
        None,
    )?;
    if v["authenticated"].as_bool() != Some(true) || v["name"].as_str() != Some(&credential.login) {
        return Err(err(
            "Jenkins did not verify this account. Check the username and API token.",
        ));
    }
    Ok(())
}
pub struct JenkinsClient<'a> {
    selection: &'a JenkinsSelection,
    credential: &'a Credential,
    client: Client,
    controller: Url,
    job: Url,
}
impl<'a> JenkinsClient<'a> {
    pub fn new(s: &'a JenkinsSelection, c: &'a Credential) -> Result<Self> {
        let (controller, job) = validate(s)?;
        if c.login != s.username {
            return Err(err("Reconnect this Jenkins account."));
        }
        let client = http_client()?;
        Ok(Self {
            selection: s,
            credential: c,
            client,
            controller,
            job,
        })
    }
    fn get(&self, url: Url, tree: Option<&str>) -> Result<Value> {
        get_json(&self.client, self.credential, url, tree)
    }
    pub fn verify_account(&self) -> Result<()> {
        verify_controller(&self.client, self.credential, &self.controller)
    }
    pub fn builds(&self, remote: &str) -> Result<JenkinsSnapshot> {
        let key = repository_key(remote).ok_or_else(|| {
            err("Choose a credential-free HTTPS or SSH Git remote for Jenkins mapping.")
        })?;
        let tree="builds[number,building,result,timestamp,duration,actions[_class,remoteUrls,lastBuiltRevision[SHA1]]]{0,21}";
        let data = self.get(
            self.job
                .join("api/json")
                .map_err(|_| err("Invalid Jenkins job address."))?,
            Some(tree),
        )?;
        let builds = data["builds"].as_array().ok_or_else(|| {
            err("This Jenkins URL did not return job builds. Choose a branch job, not a folder.")
        })?;
        let mut runs = vec![];
        let mut unlinked = 0;
        let mut active_unlinked = false;
        for build in builds.iter().take(20) {
            let active = build["building"].as_bool() == Some(true);
            let mut revisions = std::collections::BTreeSet::new();
            let mut ambiguous = build["actions"].as_array().is_none_or(|a| a.len() > 128);
            for action in build["actions"].as_array().into_iter().flatten().take(128) {
                if action["_class"].as_str() != Some("hudson.plugins.git.util.BuildData") {
                    continue;
                }
                let Some(urls) = action["remoteUrls"].as_array() else {
                    continue;
                };
                let keys = urls
                    .iter()
                    .take(32)
                    .filter_map(|v| v.as_str().and_then(repository_key))
                    .collect::<Vec<_>>();
                if !keys.contains(&key) {
                    continue;
                }
                if urls.len() > 32 || keys.len() != urls.len() || keys.iter().any(|k| k != &key) {
                    ambiguous = true;
                    continue;
                }
                if let Some(sha) = action["lastBuiltRevision"]["SHA1"]
                    .as_str()
                    .filter(|s| valid_oid(s))
                {
                    revisions.insert(sha.to_owned());
                } else {
                    ambiguous = true;
                }
            }
            if ambiguous || revisions.len() != 1 {
                unlinked += 1;
                active_unlinked |= active;
                continue;
            }
            if !revisions.contains(&self.selection.oid) {
                continue;
            }
            let number = build["number"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or_else(|| err("Invalid Jenkins build identity."))?;
            let status = if active {
                "running"
            } else {
                match build["result"].as_str() {
                    Some("SUCCESS") => "success",
                    Some("FAILURE") => "failed",
                    Some("UNSTABLE") => "unstable",
                    Some("ABORTED") => "canceled",
                    Some("NOT_BUILT") => "not_built",
                    _ => "unknown",
                }
            };
            runs.push(PipelineRun {
                id: number.to_string(),
                sha: self.selection.oid.clone(),
                reference: self.job.path().chars().take(256).collect(),
                source: "Jenkins Git BuildData".into(),
                status: status.into(),
                url: self
                    .job
                    .join(&format!("{number}/"))
                    .map_err(|_| err("Invalid Jenkins build address."))?
                    .into(),
                jobs: vec![],
                jobs_partial: false,
            });
        }
        Ok(JenkinsSnapshot {
            pipeline: PipelineSnapshot {
                oid: self.selection.oid.clone(),
                project: self.job.path().chars().take(512).collect(),
                origin: self.controller.as_str().into(),
                runs,
                partial: builds.len() > 20,
                checked_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs(),
            },
            scanned: builds.len().min(20),
            unlinked,
            more_builds: builds.len() > 20,
            active_unlinked,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
        time::Instant,
    };
    fn config() -> JenkinsSelection {
        JenkinsSelection {
            remote: "origin".into(),
            oid: "a".repeat(40),
            controller: "https://ci.example.com/jenkins/".into(),
            job_url: "https://ci.example.com/jenkins/job/team/job/app/job/develop/".into(),
            username: "alice".into(),
        }
    }
    fn cred() -> Credential {
        Credential {
            login: "alice".into(),
            token: "mock-secret".into(),
        }
    }
    fn server(status: u16, body: Value) -> (Url, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = Url::parse(&format!(
            "http://{}/jenkins/",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let handle = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(10);
            let mut socket = loop {
                match listener.accept() {
                    Ok((s, _)) => break s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(Instant::now() < deadline);
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(e) => panic!("{e}"),
                }
            };
            // Accepted sockets can inherit the listener's nonblocking mode on
            // macOS. Read the request with the explicit timeout below rather
            // than racing the client's first bytes with immediate WouldBlock.
            socket.set_nonblocking(false).unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut data = vec![];
            loop {
                let mut bytes = [0; 4096];
                let n = socket.read(&mut bytes).unwrap();
                assert!(n > 0);
                data.extend_from_slice(&bytes[..n]);
                if data.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
                assert!(data.len() < 16384);
            }
            let headers = String::from_utf8(data).unwrap();
            assert!(headers.starts_with("GET /jenkins/"));
            assert!(headers
                .to_ascii_lowercase()
                .contains("authorization: basic ywxpy2u6bw9jay1zzwnyzxq="));
            if headers.contains("/job/team/") {
                let target = headers
                    .lines()
                    .next()
                    .unwrap()
                    .split_whitespace()
                    .nth(1)
                    .unwrap();
                let url = Url::parse(&format!("http://fixture{target}")).unwrap();
                let tree = url
                    .query_pairs()
                    .find(|(k, _)| k == "tree")
                    .unwrap()
                    .1
                    .into_owned();
                assert!(tree.contains("remoteUrls,lastBuiltRevision[SHA1]"));
                assert!(tree.ends_with("{0,21}"));
            }
            let bytes = serde_json::to_vec(&body).unwrap();
            write!(socket,"HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nLocation: https://other.example.com/\r\nConnection: close\r\n\r\n",bytes.len()).unwrap();
            socket.write_all(&bytes).unwrap();
        });
        (url, handle)
    }
    fn action(remote: &str, sha: &str) -> Value {
        json!({"_class":"hudson.plugins.git.util.BuildData","remoteUrls":[remote],"lastBuiltRevision":{"SHA1":sha}})
    }
    #[test]
    fn global_jenkins_identity_needs_no_repository_job_or_revision() {
        assert_eq!(
            account_scope("https://ci.example.com/jenkins", "alice")
                .unwrap()
                .as_str(),
            "https://ci.example.com/jenkins/"
        );
        assert_eq!(
            account_key("https://ci.example.com/jenkins/", "alice").unwrap(),
            "https://ci.example.com/jenkins/:alice"
        );
        for controller in [
            "http://ci.example.com",
            "https://user:secret@ci.example.com",
            "https://ci.example.com/?token=secret",
        ] {
            assert!(account_scope(controller, "alice").is_err());
        }
        assert!(account_scope("https://ci.example.com", "").is_err());
        assert!(account_scope("https://ci.example.com", "alice:secret").is_err());
    }
    #[test]
    fn exact_repository_and_revision_keep_running_unstable_and_multiscm_distinct() {
        let s = config();
        let c = cred();
        let (url, task) = server(
            200,
            json!({"builds":[
             {"number":9,"building":true,"result":null,"url":"https://evil.example.com/","actions":[action("git@gitlab.com:team/project.git",&s.oid)]},
             {"number":8,"building":false,"result":"UNSTABLE","actions":[action("https://gitlab.com/team/project",&s.oid),action("git@gitlab.com:team/other.git",&"b".repeat(40))]},
             {"number":7,"building":false,"result":"SUCCESS","actions":[action("git@gitlab.com:team/project.git",&"b".repeat(40))]},
             {"number":6,"building":false,"result":"SUCCESS","actions":[action("git@gitlab.com:other/project.git",&s.oid)]}
            ]}),
        );
        let mut client = JenkinsClient::new(&s, &c).unwrap();
        client.controller = url.clone();
        client.job = url.join("job/team/job/app/job/develop/").unwrap();
        let result = client.builds("git@gitlab.com:team/project.git").unwrap();
        task.join().unwrap();
        assert_eq!(result.pipeline.runs.len(), 2);
        assert_eq!(result.pipeline.runs[0].status, "running");
        assert_eq!(result.pipeline.runs[1].status, "unstable");
        assert_eq!(result.unlinked, 1);
        assert!(!result.pipeline.runs[0].url.contains("evil"));
        assert!(result.pipeline.runs[0].url.ends_with("/9/"));
        assert!(!serde_json::to_string(&result)
            .unwrap()
            .contains("mock-secret"));
    }
    #[test]
    fn missing_ambiguous_and_parameter_only_revisions_are_unlinked_and_pollable() {
        let s = config();
        let c = cred();
        let mut mirror = action("git@gitlab.com:team/project.git", &s.oid);
        mirror["remoteUrls"] = json!([
            "git@gitlab.com:team/project.git",
            "git@gitlab.com:team/other.git"
        ]);
        let builds = json!({"builds":[
         {"number":1,"building":true,"actions":[{"parameters":[{"name":"GIT_COMMIT","value":s.oid}]}]},
         {"number":2,"building":false,"actions":[action("git@gitlab.com:team/project.git",&s.oid),action("git@gitlab.com:team/project.git",&"b".repeat(40))]},
         {"number":3,"building":false,"actions":[mirror]}
        ]});
        let (url, task) = server(200, builds);
        let mut client = JenkinsClient::new(&s, &c).unwrap();
        client.job = url.join("job/team/job/app/job/develop/").unwrap();
        let result = client.builds("git@gitlab.com:team/project.git").unwrap();
        task.join().unwrap();
        assert!(result.pipeline.runs.is_empty());
        assert_eq!(result.unlinked, 3);
        assert!(result.active_unlinked);
    }
    #[test]
    fn browser_links_are_https_only_without_credentials_or_local_schemes() {
        assert!(crate::validated_ci_url("https://ci.example.com/job/app/9/").is_ok());
        for url in [
            "file:///tmp/private",
            "javascript:alert(1)",
            "http://ci.example.com/job/app/9/",
            "https://alice:token@ci.example.com/job/app/9/",
            "https://ci.example.com/\n",
        ] {
            assert!(crate::validated_ci_url(url).is_err());
        }
    }
    #[test]
    fn scope_requires_https_controller_job_and_unambiguous_user() {
        let base = config();
        assert!(validate(&base).is_ok());
        for url in [
            "http://ci.example.com/jenkins/job/app/",
            "https://other.example.com/job/app/",
            "https://ci.example.com/jenkins/view/team/",
            "https://alice:token@ci.example.com/jenkins/job/app/",
            "https://ci.example.com/jenkins/job/app/?token=abc",
        ] {
            let mut s = base.clone();
            s.job_url = url.into();
            assert!(validate(&s).is_err(), "{url}");
        }
        let mut s = base.clone();
        s.job_url = "https://ci.example.com/jenkins/job/team/job/app/job/feature%252Ftest/".into();
        assert!(validate(&s).is_ok());
        s.username = "alice:token".into();
        assert!(validate(&s).is_err());
        assert_ne!(
            repository_key("ssh://git@gitlab.com:2222/team/project.git"),
            repository_key("https://gitlab.com/team/project.git")
        );
        assert_ne!(
            repository_key("git@gitlab.com:Team/project.git"),
            repository_key("git@gitlab.com:team/project.git")
        );
    }
    #[test]
    fn account_and_http_failures_do_not_follow_redirects_or_become_success() {
        let s = config();
        let c = cred();
        for (status, payload, ok) in [
            (200, json!({"authenticated":true,"name":"alice"}), true),
            (
                200,
                json!({"authenticated":false,"name":"anonymous"}),
                false,
            ),
            (401, json!({}), false),
            (403, json!({}), false),
            (404, json!({}), false),
            (429, json!({}), false),
            (302, json!({}), false),
        ] {
            let (url, task) = server(status, payload);
            let mut client = JenkinsClient::new(&s, &c).unwrap();
            client.controller = url;
            assert_eq!(client.verify_account().is_ok(), ok);
            task.join().unwrap();
        }
    }
    #[test]
    fn scan_is_bounded_and_new_credentials_or_job_invalidate_cache_identity() {
        let s = config();
        let c = cred();
        let builds=(1..=21).map(|n|json!({"number":n,"building":false,"result":"SUCCESS","actions":[action("git@gitlab.com:team/project.git",&s.oid)]})).collect::<Vec<_>>();
        let (url, task) = server(200, json!({"builds":builds}));
        let mut client = JenkinsClient::new(&s, &c).unwrap();
        client.job = url.join("job/team/job/app/job/develop/").unwrap();
        let result = client.builds("git@gitlab.com:team/project.git").unwrap();
        task.join().unwrap();
        assert_eq!(result.pipeline.runs.len(), 20);
        assert_eq!(result.scanned, 20);
        assert!(result.more_builds);
        let key = cache_key(&s, "repo", &c).unwrap();
        assert!(!key.contains("mock-secret"));
        assert_ne!(
            key,
            cache_key(
                &s,
                "repo",
                &Credential {
                    login: c.login.clone(),
                    token: "replacement".into()
                }
            )
            .unwrap()
        );
        let mut changed = s.clone();
        changed.job_url = "https://ci.example.com/jenkins/job/other/".into();
        assert_ne!(key, cache_key(&changed, "repo", &c).unwrap());
    }
}
