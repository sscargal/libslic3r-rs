#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The parser must never panic on arbitrary input.
    // It should return Ok or Err gracefully.
    let _ = slicecore_fileio::load_mesh(data);
});
