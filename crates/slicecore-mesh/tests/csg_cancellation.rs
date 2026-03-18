//! Integration tests for CSG cancellation token support.

use std::thread;
use std::time::Duration;

use slicecore_mesh::csg::{
    boolean::mesh_union_with, primitives::primitive_box, primitives::primitive_sphere,
    CsgCancellationToken, CsgError, CsgOptions,
};

#[test]
fn test_cancellation_stops_union() {
    let a = primitive_box(2.0, 2.0, 2.0);
    let b = primitive_box(2.0, 2.0, 2.0);

    let token = CsgCancellationToken::new();
    // Cancel immediately before calling the operation.
    token.cancel();

    let opts = CsgOptions {
        cancellation_token: Some(token),
        ..CsgOptions::default()
    };

    let result = mesh_union_with(&a, &b, &opts);
    match &result {
        Err(CsgError::Cancelled) => {} // expected
        Err(e) => panic!("expected Cancelled, got Err({e})"),
        Ok(_) => panic!("expected Cancelled, got Ok(...)"),
    }
}

#[test]
fn test_cancellation_during_computation() {
    // Use moderately complex meshes so the operation takes measurable time.
    let a = primitive_sphere(2.0, 32);
    let b = primitive_sphere(2.0, 32);

    let token = CsgCancellationToken::new();
    let token_clone = token.clone();

    // Spawn a thread that cancels after 10ms.
    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        token_clone.cancel();
    });

    let opts = CsgOptions {
        cancellation_token: Some(token),
        ..CsgOptions::default()
    };

    let result = mesh_union_with(&a, &b, &opts);
    handle.join().expect("cancel thread panicked");

    // The operation may complete before cancellation on fast machines,
    // so we accept either Cancelled or Ok.
    match &result {
        Ok(_) | Err(CsgError::Cancelled) => {} // both acceptable
        Err(e) => panic!("expected Ok or Cancelled, got Err({e})"),
    }
}

#[test]
fn test_no_cancellation_completes() {
    let a = primitive_box(2.0, 2.0, 2.0);
    let b = primitive_box(2.0, 2.0, 2.0);

    // No cancellation token.
    let opts = CsgOptions::default();
    let result = mesh_union_with(&a, &b, &opts);
    match &result {
        Ok(_) => {} // expected
        Err(e) => panic!("union without cancellation should succeed, got Err({e})"),
    }
}
