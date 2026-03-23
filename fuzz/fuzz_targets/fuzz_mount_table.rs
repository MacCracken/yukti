#![no_main]
use libfuzzer_sys::fuzz_target;
use std::path::Path;

fuzz_target!(|data: &[u8]| {
    if let Ok(table) = std::str::from_utf8(data) {
        let _ = yantra::storage::find_mount_in(Path::new("/dev/sda1"), table);
    }
});
