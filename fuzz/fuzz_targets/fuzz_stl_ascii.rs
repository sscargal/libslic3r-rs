#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed data that starts with "solid " to trigger ASCII STL path.
    // The parser must handle malformed ASCII STL without panic.
    let mut input = b"solid fuzz\n".to_vec();
    input.extend_from_slice(data);
    let _ = slicecore_fileio::load_mesh(&input);
});
