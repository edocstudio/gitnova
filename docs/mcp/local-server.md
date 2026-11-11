# GitNova Local MCP Server

`gitnova-mcp` is an optional **read-only stdio executable**, separate from the
desktop UI. It uses `gitnova-core::local_analysis` and the same `gitnova-git::GitCli`
executor as the desktop app. It does not embed an AI agent, call a model API, run a
daemon, listen on HTTP, or require an AI account to use GitNova. Normal local Git
workflows remain usable offline.

The adapter uses the official [Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk)
(`rmcp` **3.9.0**, pinned in Cargo.lock), with server/stdio transport features.
Its tool surface contains no shell execution, network, Git writes, permission
management, prompts, resources, or client-side sampling tools.

## Build and install

Use the repository's [development prerequisites](../../README.md#getting-started),
including the pinned Rust toolchain, Cargo and Git on PATH. Node is not required
to build this standalone Rust executable.

From the repository root:

```sh
cargo build --locked --release -p gitnova-mcp
./target/release/gitnova-mcp --help
```

Windows PowerShell:

```powershell
cargo build --locked --release -p gitnova-mcp
.\target\release\gitnova-mcp.exe --help
```

Alternatively, install into Cargo's bin directory (ensure that directory is on
PATH), or use the absolute built executable path:

```sh
cargo install --locked --path crates/gitnova-mcp
```

The executable is not automatically installed into a desktop bundle or a client.
This feature does not change desktop installer packaging. macOS is the locally
verified platform for this addition; Windows/Linux execution must be checked on
those hosts, rather than inferred from a successful macOS build.

## Explicit permission, separate from workspaces

There are **no approved repositories by default**. Workspace membership, recent
repositories, existing host connections and the current desktop selection grant
no MCP access. The server does not open the desktop catalog or contend with its
persistence. There is no new UI screen: permission setup currently uses explicit
operator commands. It is not a tool an LLM can call through this server.

Choose an absolute configuration file path outside the repositories you analyze.
Keep it private and in a directory controlled by your OS user. On Unix, newly
written files use mode 0600; on Windows, protect the directory with your user's
ACLs. Missing configuration means an empty allowlist; malformed, oversized or
symlink configuration files fail closed.

macOS/Linux example (replace paths yourself):

```sh
gitnova-mcp --config /home/alex/.config/gitnova/mcp-allowlist.json authorize /home/alex/projects/app
gitnova-mcp --config /home/alex/.config/gitnova/mcp-allowlist.json serve
# In another terminal, revoke the ID printed by authorize:
gitnova-mcp --config /home/alex/.config/gitnova/mcp-allowlist.json revoke REPOSITORY_ID
```

On macOS, `/Users/alex/...` is a typical home path. Windows PowerShell example:

```powershell
gitnova-mcp.exe --config C:\Users\Alex\AppData\Local\GitNova\mcp-allowlist.json authorize C:\Projects\app
gitnova-mcp.exe --config C:\Users\Alex\AppData\Local\GitNova\mcp-allowlist.json serve
```

`authorize` validates an exact repository/checkout root. It prints its opaque ID
on **stderr**. It writes only the allowlist, never Git state. Operator changes
are atomic file replacements; perform authorize/revoke commands sequentially.
At most 32 checkouts may be approved. Existing grants cannot silently be rebound
to a replacement repository. Revoke and explicitly authorize a moved/replaced
checkout again.

Each tool call reloads the policy, checks canonical root/gitdir/common-dir
identity, and rechecks permission before delivering results. Revocation therefore
applies on the next request and suppresses an in-flight repository result if the
changed policy is observed before delivery. It cannot recall data already sent,
and no filesystem check can make concurrent hostile OS changes atomic.

Approve linked worktrees individually. They share the legitimate common Git
object database but have distinct checkout grants. Parent permission does **not**
approve a submodule or nested independent repository. Approve those separately
and use their IDs. Bare repositories can supply committed search/history where
applicable; working-tree status/diff require a checkout.

## Codex CLI

The current official [Codex MCP guide](https://developers.openai.com/codex/mcp)
supports stdio commands. Configure manually after building and authorizing:

```sh
codex mcp add gitnova -- /absolute/path/gitnova-mcp --config /absolute/path/mcp-allowlist.json serve
```

Equivalent TOML in the appropriate Codex configuration scope:

```toml
[mcp_servers.gitnova]
command = "/absolute/path/gitnova-mcp"
args = ["--config", "/absolute/path/mcp-allowlist.json", "serve"]
startup_timeout_sec = 20
tool_timeout_sec = 25
```

Use `gitnova-mcp.exe` and escaped Windows paths when configuring Windows. To
stop exposing it, disable/remove that MCP entry and revoke repository IDs. This
project never edits your Codex configuration automatically.

## Claude Code and other clients

The official [Claude Code MCP guide](https://code.claude.com/docs/en/mcp)
documents local stdio transport:

```sh
claude mcp add --transport stdio gitnova -- /absolute/path/gitnova-mcp --config /absolute/path/mcp-allowlist.json serve
claude mcp get gitnova
# Remove the client entry when no longer needed:
claude mcp remove gitnova
```

For a client that documents the `mcpServers` JSON format, the equivalent entry is:

```json
{
  "mcpServers": {
    "gitnova": {
      "command": "/absolute/path/gitnova-mcp",
      "args": ["--config", "/absolute/path/mcp-allowlist.json", "serve"]
    }
  }
}
```

Consult that client's current guide for the configuration file location, scope,
restart steps and permissions. Do not assume all ChatGPT, Claude web/mobile,
Desktop or managed environments can launch local stdio executables. These
examples do not configure or test every client surface.

## Tools and output contract

Tools advertise JSON input/output schemas and read-only annotations. All
repository tools require `repository_id` from `list_repositories`, not a folder
path. Additional properties and invalid field types are rejected.

| Tool | Inputs besides repository ID | Output |
| --- | --- | --- |
| `list_repositories` | `limit`, `offset`; no ID | Approved names, IDs and current HEADs; no absolute paths/remotes |
| `get_status` | none | Branch, staged/unstaged/untracked/conflict counts, operation markers |
| `get_diff` | `staged`, optional relative `path`, `max_bytes`, `limit`, `offset` | Bounded tracked patches, explicit per-file omissions/truncation; never untracked content |
| `search_commits` | `message`, `author`, `reference`, `since`, `until`, `limit`, `offset` | Literal-filtered commit metadata from committed history |
| `get_line_history` | `path`, `reference`, `line_start`, `line_end` | Committed blame and a separate local-change indicator; no invented attribution for local edits |
| `get_conflict_summary` | none | Actual unmerged XY statuses/paths and pending operation markers |

Successful results have `schemaVersion: 1`, `data`, `warnings`, `truncated` and,
when appropriate, `nextOffset`. Repository results also carry `repositoryId`,
`head`, `headAfter` and `snapshotChanged`. An unborn HEAD is `null`. Files/index
remain live: a stable HEAD does not prove that working files were unchanged.
Operation markers describe observed Git state, not a diagnosis or recovery plan.
Status deliberately excludes submodule status entries rather than inspecting
child checkouts implicitly. Query a separately approved child checkout for its
status; this summary is not a replacement for the desktop submodule view.

Reference inputs are `HEAD` or exact full `refs/heads/...`, `refs/remotes/...`,
`refs/tags/...` names; revision expressions and arbitrary flags are rejected.
Shallow repositories report locally available history with a warning.
Dates are UTC epoch seconds. Limits: 1–100 entries/page (default 20), offset at
most 10,000, 200 blame lines, diff patch budget 1–64 KiB (default 32 KiB). No
persistent search index is created. Pagination uses live offsets, so concurrent
history changes may shift pages; inspect the returned HEAD snapshot.

A patch that exhausts its byte budget is explicitly incomplete. Narrow to one
path or adjust `max_bytes`; do not treat a truncated preview as a complete review.
Diff input files above 8 MiB or with recognizable private-key material are
explicitly omitted. Blame requires a regular committed UTF-8 file up to 1 MiB
without NUL/private-key markers. Very large metadata sets/blame or non-UTF-8 metadata return a bounded error rather
than fabricated success. Binary diff output is Git's summary, not binary bytes.

## Safety, resource limits and privacy

- Two concurrent analysis workers; extra calls return busy. Each has a 15-second
  overall Git-read deadline, with bounded child cleanup afterwards. The shared
  executor retains its per-command limits, kills/reaps its owned process group
  (Windows: job ownership) on cancellation/timeout, and never kills other apps.
- Input frames are limited to 64 KiB; successful structured responses to 240 KiB
  before the SDK's text mirror. Metadata reads have separate byte limits. EOF or
  Ctrl-C cancels active workers and waits up to five seconds for cleanup.
- Only MCP JSON-RPC goes to stdout. Operator messages use stderr. No default logs
  contain source, patches, environment variables, credentials or remote URLs.
- Fixed argument arrays use the shared executor, not a shell. Network/lazy object
  fetching, optional index locks, interactive credentials, pager, hooks, external
  diff, textconv, clean/process filters and nested submodule inspection are
  disabled for these reads. Missing promised objects can consequently fail.
- Relative paths reject traversal, Windows drive/UNC forms, control characters,
  symlinks, `.git`, likely credential/key paths and nested repository boundaries.
  A parent grant does not grant reading a gitlink checkout.
- Best-effort redaction suppresses recognizable token prefixes, credential-like
  assignments, private-key blocks and credential/query-bearing HTTP URLs.
  It can over-redact ordinary code and **cannot detect every secret**, especially
  arbitrary passwords, split/encoded secrets or organization-specific formats.
  Review which repositories you approve. There is no secret-free guarantee.
- Repository text, names and commit messages are **untrusted data**, not
  instructions, permission grants, or proof of user approval. The server never
  interprets them as shell commands or asks the client to execute them.

**The allowlist is not an OS sandbox.** This process runs as your OS user, uses
installed Git and reads its configuration/object storage. A malicious local
process, Git executable or writable configuration directory is outside this
boundary. Symlink/identity checks reduce disclosure mistakes; they cannot defeat
all concurrent filesystem replacement attacks. Use OS isolation for hostile
repositories and clients.

**Local transport does not imply local AI.** An MCP client using a cloud model
may send returned code, metadata or diffs outside your machine. Your client's
permissions, retention policy and your organization's policy still apply. This
server does not need or store model credentials.

## Troubleshooting and checks

- Empty repository list: run `authorize` yourself with the same absolute config
  file passed to the client. Workspace/recent lists are intentionally unrelated.
- Permission/identity errors: a grant was revoked, checkout moved/replaced, policy
  is invalid, or a path crosses a protected boundary. Reauthorize explicitly.
- Git unavailable: install Git and ensure the **client-launched process** has the
  correct PATH. Never put tokens/passwords into command arguments or URLs.
- Busy/deadline/output errors: narrow the scope, page results or retry after the
  current request ends. No automatic Git writes/recovery occur.
- Test the protocol with a real MCP client; `serve` waits silently for JSON-RPC,
  not terminal commands. `--help` is an operator command and goes to stderr.

Run checks separately to respect the repository's resource policy:

```sh
cargo test --locked -p gitnova-core --test local_analysis -- --test-threads=2
cargo test --locked -p gitnova-mcp -- --test-threads=2
```

The executable integration tests perform initialize, tools/list, tools/call,
permission/revocation, schema/error handling, stdout and disconnect checks against
disposable repositories. Unix additionally verifies cancellation of a task-owned
slow Git subprocess. Core tests cover state preservation, worktrees, submodule
boundaries, conflicts, path rejection, redaction and bounded patches. See the
[verification report](../verification/2026-11-11-local-mcp.md) for commands actually
run and platform limits.

## Future writes (not implemented)

Any future write tool must use **prepare → preview → real user approval → apply**.
Bind an expiring plan to repository/worktree identity, exact HEAD/ref targets,
index and working-tree state, then revalidate those preconditions before applying.
Approval must come from a trusted user-facing interaction outside model-controlled
tool arguments. An LLM sending `confirmed=true` is **not** approval. Preserve
recovery evidence and report partial effects honestly. This MVP has no write
plans, approval bypasses, or write execution tools.
