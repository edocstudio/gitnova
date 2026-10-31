//! Windows ownership proof and handle-based archival. Never delete a native Git lock.
use super::*;
use std::{
    fs::{File, OpenOptions},
    io::Read,
    os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle},
};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, GetLastError, ERROR_INVALID_PARAMETER, ERROR_NO_MORE_FILES, GENERIC_READ,
        HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
    },
    Storage::FileSystem::{
        FileRenameInfo, GetFileInformationByHandle, SetFileInformationByHandle,
        BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_RENAME_INFO,
        FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, SYNCHRONIZE,
    },
    System::{
        Diagnostics::ToolHelp::{
            CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
            TH32CS_SNAPPROCESS,
        },
        Threading::{OpenProcess, WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION},
    },
};
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.0);
        }
    }
}
fn dead(pid: u32) -> Result<()> {
    let raw = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, 0, pid) };
    if raw.is_null() {
        return if unsafe { GetLastError() } == ERROR_INVALID_PARAMETER {
            Ok(())
        } else {
            Err(fail("The lock owner is still running or cannot be verified. Close or finish that operation first."))
        };
    }
    let handle = Handle(raw);
    if unsafe { WaitForSingleObject(handle.0, 0) } == WAIT_OBJECT_0 {
        Ok(())
    } else {
        Err(fail("The lock owner is still running or cannot be verified. Close or finish that operation first."))
    }
}
fn open(path: &Path, recover: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options
        .access_mode(GENERIC_READ | if recover { DELETE } else { 0 })
        // A recovery handle denies other writers/deleters. Read-only identity
        // probes remain possible, while a second recovery cannot acquire DELETE.
        .share_mode(if recover {
            FILE_SHARE_READ
        } else {
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    let file = options.open(path)?;
    let info = information(&file)?;
    if info.dwFileAttributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
        return Err(fail(
            "The operation lock is not a regular file. Inspect it manually.",
        ));
    }
    Ok(file)
}
fn information(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(info)
}
fn file_id(info: &BY_HANDLE_FILE_INFORMATION) -> (u32, u32, u32) {
    (
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    )
}
fn identity(path: &Path) -> Result<(u32, String)> {
    let mut file = open(path, false)?;
    let info = information(&file)?;
    let mut bytes = Vec::new();
    file.by_ref().take(129).read_to_end(&mut bytes)?;
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| fail("The operation lock has an unknown format. Inspect it manually."))?;
    let pid = text
        .strip_prefix("GitNova pid=")
        .and_then(|s| s.strip_suffix('\n'))
        .filter(|s| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<u32>().ok())
        .filter(|p| *p > 0 && *p <= i32::MAX as u32)
        .ok_or_else(|| fail("The operation lock has an unknown format. Inspect it manually."))?;
    if bytes.len() > 128 {
        return Err(fail(
            "The operation lock has an unknown format. Inspect it manually.",
        ));
    }
    dead(pid)?;
    Ok((
        pid,
        format!(
            "{:?}:{}:{}:{}:{}:{text}",
            file_id(&info),
            info.ftLastWriteTime.dwHighDateTime,
            info.ftLastWriteTime.dwLowDateTime,
            info.nFileSizeHigh,
            info.nFileSizeLow
        ),
    ))
}
fn no_git_process() -> Result<()> {
    let fail_probe = || fail("Process ownership could not be verified. Try inspection again.");
    let raw = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
    if raw == INVALID_HANDLE_VALUE {
        return Err(fail_probe());
    }
    let handle = Handle(raw);
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    if unsafe { Process32FirstW(handle.0, &mut entry) } == 0 {
        return Err(fail_probe());
    }
    for _ in 0..16384 {
        let end = entry
            .szExeFile
            .iter()
            .position(|c| *c == 0)
            .ok_or_else(fail_probe)?;
        let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
        if name == "git.exe" || name.starts_with("git-") {
            return Err(fail(
                "A Git process is running. Wait for Git to finish, then inspect this lock again.",
            ));
        }
        if unsafe { Process32NextW(handle.0, &mut entry) } == 0 {
            return if unsafe { GetLastError() } == ERROR_NO_MORE_FILES {
                Ok(())
            } else {
                Err(fail_probe())
            };
        }
    }
    Err(fail_probe())
}
pub(super) fn inspect(git: &GitCli, root: &Path, path: &Path) -> Result<(u32, String)> {
    super::inspect(git, root, path, identity, dead, no_git_process)
}
pub(super) fn recover(git: &GitCli, root: &Path, token: &str) -> Result<NativePath> {
    let d = git.discover(root)?;
    // Prevent replacing the immediate metadata directory during archival.
    let _directory = OpenOptions::new()
        .access_mode(GENERIC_READ)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(&d.common_dir)?;
    let path = d.common_dir.join("gitnova-operation.lock");
    let file = open(&path, true)?;
    let before = information(&file)?;
    let (pid, actual) = inspect(git, root, &path)?;
    if token.len() != 64
        || actual != token
        || file_id(&information(&open(&path, false)?)?) != file_id(&before)
    {
        return Err(fail(
            "The lock changed. Inspect it again before recovering.",
        ));
    }
    let archive = d.common_dir.join(format!(
        "gitnova-operation.lock.stale-{pid}-{}",
        uuid::Uuid::new_v4().simple()
    ));
    let wide: Vec<u16> = archive.as_os_str().encode_wide().chain(Some(0)).collect();
    let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
    let length = offset + wide.len() * 2;
    let mut buffer = vec![0usize; length.div_ceil(std::mem::size_of::<usize>())];
    // Aligned, zero-initialized variable-length FILE_RENAME_INFO. Never replace
    // an archive, and rename this verified handle rather than reopening a path.
    let info = buffer.as_mut_ptr().cast::<FILE_RENAME_INFO>();
    unsafe {
        (*info).FileNameLength = ((wide.len() - 1) * 2) as u32;
        std::ptr::copy_nonoverlapping(
            wide.as_ptr(),
            buffer.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(),
            wide.len(),
        );
        if SetFileInformationByHandle(
            file.as_raw_handle(),
            FileRenameInfo,
            info.cast(),
            length as u32,
        ) == 0
        {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    let moved = open(&archive, false)?;
    if file_id(&information(&moved)?) != file_id(&before) {
        return Err(AppError::new(
            "operation_unknown",
            "Lock archival needs manual inspection. No Git operation was retried.",
        ));
    }
    Ok(NativePath::new(&archive))
}
