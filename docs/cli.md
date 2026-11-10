# Nova CLI: promote selected files

Open GitNova's existing **Terminal**, then run `nova --help`. The session supplies
the command without changing your shell startup files. For a standalone build,
run `cargo build -p gitnova-core --bin nova`; use `target/debug/nova` (`nova.exe`
on Windows).

On zsh, session-owned startup wrappers load your existing startup files and then
restore Nova's session command path, even if those files replace `PATH`. Your
startup files are not edited. After updating GitNova, end an old Terminal session
and open a new one to receive the current launcher.

With the Terminal dock visible, selecting another repository opens its own shell
at that repository's root (or shows its existing session). Existing sessions and
running commands are preserved, up to four sessions. A reused session retains
any directory you intentionally navigated to. New zsh sessions restore the
repository root after startup files, even if those files change directories.

## Terminal idle timeout

**Settings → General → Terminal idle timeout** applies across repositories on
this device. The default is **Never close automatically**; optional limits are
30 minutes and 1 hour. Preferences persist between app launches.

Automatic closure requires a verified idle prompt with no background jobs.
Typing, shell output and commands reset or suspend the timer. Partial input,
silent foreground commands, background jobs and unknown shell states stay open.
Currently idle detection integrates with zsh; other shells conservatively stay
open. Hiding the dock or selecting another repository does not itself close a
session. Expiry checks run about every 15 seconds while the app can run timers;
sleep or OS suspension can delay them. An expired session is closed and removed,
with a status message. Open Terminal again to start a new session.

## Source → review branch → destination

Nova copies **whole file snapshots** from source onto the destination tree, creates
a new review branch, publishes it, and creates a GitHub pull request or GitLab
merge request. It does not merge either branch or cherry-pick commits.

Preview exact selected files:

```sh
nova promote --source develop --target release --branch promote/config -- config/app.yaml src/service.ts
```

Both branches are read from `origin`, not potentially stale local branches. Use
`--remote NAME` for another configured remote. Each invocation fetches the two
branches into an isolated bare repository, without tags or submodule recursion.
The preview shows exact paths, actions, statistics, pinned commits and committer.

To publish and open the request:

```sh
nova promote --source develop --target release --branch promote/config --provider gitlab --title "Promote selected configuration" --description "Review the selected configuration and service changes." --execute -- config/app.yaml src/service.ts
```

Use `--provider github` for GitHub or `--provider gitlab` for self-managed GitLab.
Public hosts can be detected. `--host` accepts a provider HTTPS web origin,
including a base path, when it differs from the Git transport host. Connect the
matching provider account in **Settings → Connections** first. Tokens remain in
the existing OS credential store; SSH uses the configured SSH agent. Do not put
credentials in arguments, URLs, descriptions or files.

Type `publish promote/config` when asked. After verifying publication and the
provider request preview, separately confirm `create promote/config -> release`.
Existing matching requests are reported instead of duplicated. `--no-request`
publishes only and does not require a connected provider account.

## Boundaries and recovery

- Current branch, working files and index stay in place, including dirty work.
  Nothing is stashed, checked out or committed in the original repository.
- Embedded-terminal identity precedence is repository identity, matching GitNova
  connection defaults, then effective Git configuration. Standalone Nova uses
  repository/effective Git configuration. Check the displayed identity.
- Select 1–200 unique, literal repository-relative regular file paths after `--`.
  Binary files are supported. Directories, symlinks, submodule pointers and
  `.gitmodules` require their existing reviewed workflows.
- Contents and executable modes come from source. Unselected files retain the
  destination version. Destination edits in selected files can be replaced;
  inspect the resulting request before merging.
- Missing source files are rejected unless `--allow-deletions` explicitly allows
  deletion of selected destination files. Unchanged selections create no commit.
- Original repository hooks and checkout filters do not run in this isolated
  flow. Provider rules and CI still apply; reviews are not bypassed.
- Source/target movement invalidates publication review. A create-only remote
  lease prevents overwriting an existing review branch.
- Before publication, Nova prints a retained recovery repository containing the
  candidate commit. Publication and request creation are separate effects,
  **not atomic**. On failure, inspect remote branches and existing requests before
  retrying; there is no automatic retry or rollback.
- Retained repositories include source history and are not automatically deleted.
  Remove the printed folder only after verifying results and deciding recovery
  is no longer needed. They reside in the OS temporary directory, which the OS
  may clean; copy recovery evidence elsewhere if it must survive cleanup/restart.
- This first CLI flow creates requests within the same remote project. Cross-fork
  promotion, an interactive file picker and automatic merging are not implemented.

## สรุปการใช้งาน

เปิด Terminal ใน GitNova แล้วพิมพ์ `nova --help` ใส่ชื่อไฟล์ที่เลือกหลัง `--`
ระบบจะนำไฟล์ทั้งไฟล์จาก source มาวางบนฐาน target แล้วสร้าง branch ใหม่เพื่อเปิด
PR/MR เช่น `develop → promote/config → release` โดยไม่เปลี่ยน branch ที่เปิดอยู่
หรือแตะไฟล์และ staging ที่แก้ค้างไว้ คำสั่งปกติเป็น preview เท่านั้น เพิ่ม `--execute`
เพื่อยืนยัน push และยืนยันเปิด PR/MR อีกครั้ง ทั้งสองขั้นตอนไม่ใช่ transaction เดียวกัน
หากล้มเหลวให้ตรวจ remote และคำขอที่มีอยู่ก่อนลองใหม่ ตำแหน่ง recovery จะแสดงไว้ใน
Terminal โดยเก็บใน temporary directory ซึ่งระบบปฏิบัติการอาจล้างได้ หากต้องเก็บระยะยาว
ให้คัดลอกข้อมูล recovery ไปที่อื่นก่อน
