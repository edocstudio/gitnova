use gitnova_domain::NativePath;
use std::path::Path;

#[test]
fn round_trip_ignores_display_and_accepts_uppercase_hex() {
    let path = Path::new("folder/ภาษาไทย space/file.txt");
    let mut encoded = NativePath::new(path);
    encoded.display = "untrusted display".into();
    encoded.hex.make_ascii_uppercase();
    assert_eq!(encoded.to_path().unwrap(), path);
}

#[test]
fn malformed_and_oversized_paths_return_errors() {
    let mut encoded = NativePath::new(Path::new("file"));
    for hex in ["0", "zz", "é", "00"] {
        encoded.hex = hex.into();
        assert!(encoded.to_path().is_err(), "{hex}");
    }
    encoded.hex = "61".repeat(128 * 1024 + 1);
    assert!(encoded.to_path().is_err());
    encoded.hex = "6161".into();
    encoded.encoding = "other-platform".into();
    assert!(encoded.to_path().is_err());
}

#[cfg(unix)]
#[test]
fn non_utf8_paths_round_trip_losslessly() {
    use std::{ffi::OsStr, os::unix::ffi::OsStrExt};
    let path = Path::new(OsStr::from_bytes(b"folder/\xff.txt"));
    assert_eq!(NativePath::new(path).to_path().unwrap(), path);
}

#[cfg(windows)]
#[test]
fn windows_code_units_round_trip_without_treating_zero_bytes_as_nul() {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    let path = std::path::PathBuf::from(OsString::from_wide(&[0x41, 0xd800, 0x42]));
    assert_eq!(NativePath::new(&path).to_path().unwrap(), path);
    let mut encoded = NativePath::new(&path);
    encoded.hex = "410000004200".into();
    assert!(encoded.to_path().is_err());
    encoded.hex = "41".into();
    assert!(encoded.to_path().is_err());
}
