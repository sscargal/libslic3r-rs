#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed data that starts with OBJ vertex lines to trigger OBJ parser path.
    // The parser must handle malformed OBJ without panic.
    let mut input = b"v 0 0 0\nv 1 0 0\nv 0 1 0\n".to_vec();
    input.extend_from_slice(data);
    let _ = slicecore_fileio::load_mesh(&input);
});
