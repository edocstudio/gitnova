//! Windows directory tokens use volume/file identity, never creation timestamps.
use gitnova_domain::*;
use std::{
    fs::OpenOptions,
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{
    GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
    FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
};
pub(crate) fn directory(path: &Path) -> Result<String> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY == 0
        || info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    {
        return Err(AppError::new(
            "path",
            "Repository folder changed. Reopen it before continuing.",
        ));
    }
    Ok(format!(
        "{}:{}:{}",
        info.dwVolumeSerialNumber, info.nFileIndexHigh, info.nFileIndexLow
    ))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recreated_directory_has_a_distinct_identity_even_with_the_same_name() {
        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("folder");
        std::fs::create_dir(&path).unwrap();
        let before = directory(&path).unwrap();
        std::fs::rename(&path, d.path().join("retained")).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_ne!(directory(&path).unwrap(), before);
        assert_eq!(directory(&d.path().join("retained")).unwrap(), before);
    }
}
