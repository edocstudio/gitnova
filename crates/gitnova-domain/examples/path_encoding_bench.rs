//! Synthetic microbenchmark; does not measure Git operations, UI latency or application RAM.
use gitnova_domain::NativePath;
use std::{hint::black_box, path::Path, time::Instant};

fn legacy(path: &Path) -> NativePath {
    #[cfg(unix)]
    let (encoding, bytes) = {
        use std::os::unix::ffi::OsStrExt;
        ("unix", path.as_os_str().as_bytes().to_vec())
    };
    #[cfg(windows)]
    let (encoding, bytes) = {
        use std::os::windows::ffi::OsStrExt;
        (
            "windows",
            path.as_os_str()
                .encode_wide()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    };
    NativePath {
        display: path.to_string_lossy().into(),
        encoding: encoding.into(),
        hex: bytes.iter().map(|b| format!("{b:02x}")).collect(),
    }
}

fn elapsed(encode: fn(&Path) -> NativePath, path: &Path, iterations: usize) -> f64 {
    let start = Instant::now();
    for _ in 0..iterations {
        black_box(encode(black_box(path)));
    }
    start.elapsed().as_secs_f64() * 1000.0
}

fn main() {
    let iterations = 5000;
    let rounds = 5;
    println!("{{\"kind\":\"gitnova-native-path-microbenchmark\",\"schemaVersion\":1,\"iterationsPerSample\":{iterations},\"rounds\":{rounds},\"scenarios\":[");
    for (index, length) in [64, 1024].into_iter().enumerate() {
        let raw = "x".repeat(length);
        let path = Path::new(&raw);
        assert_eq!(legacy(path), NativePath::new(path));
        black_box(elapsed(legacy, path, 100));
        black_box(elapsed(NativePath::new, path, 100));
        let mut before = Vec::new();
        let mut after = Vec::new();
        for round in 0..rounds {
            if round % 2 == 0 {
                before.push(elapsed(legacy, path, iterations));
                after.push(elapsed(NativePath::new, path, iterations));
            } else {
                after.push(elapsed(NativePath::new, path, iterations));
                before.push(elapsed(legacy, path, iterations));
            }
        }
        if index > 0 {
            println!(",");
        }
        println!("{{\"pathCharacters\":{length},\"beforeMilliseconds\":{before:?},\"afterMilliseconds\":{after:?}}}");
    }
    println!("]}}");
}
