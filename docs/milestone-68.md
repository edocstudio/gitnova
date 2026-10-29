# Milestone 68 — Initialize a local repository

Initialization lives in the existing workspace picker beside Open and Clone, because an uninitialized folder has no graph context. No repository dashboard or persistent navigation is added. The same form is available from the empty workspace and the repository switcher, in English and Thai.

Users enter a full destination path and an initial branch (default main). The Open field supplies an initial path to the form, making it useful after trying to open a non-Git project. Review shows the canonical destination and whether the directory already exists. Confirmation creates only local Git metadata and opens the registered repository graph. Existing content stays unstaged and unchanged; no initial commit, remote or network activity is performed. Successful initialization clears the previous Open error.

Core and Git initialization are independent of Tauri. Preview validates workspace, branch syntax and destination; confirmation rechecks the token bound to workspace, canonical destination, initial branch and parent/destination identities. Creating .git exclusively rejects competing initialization and gitfiles. No files are automatically deleted after partial failures. Registration failure identifies that Git initialization succeeded and directs the user to Add existing.

The initial scope excludes bare initialization, nested initialization inside another repository, destination symlinks, ignore/license templates and parent-directory creation. Git templates are disabled; users can add files through normal Working changes and configure publication through Push. Existing repositories, including worktrees, remain handled through Open.

Verification uses temporary repositories and the real Rust/Git backend; no company repository is initialized or modified. See the dated verification report for actual results and packaging status.
