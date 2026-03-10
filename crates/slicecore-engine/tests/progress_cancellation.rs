//! Integration tests for progress events and cancellation.
//!
//! Verifies that the engine emits SliceEvent::Progress events with correct
//! fields, that CancellationToken stops slicing mid-flight, and that the
//! Cancelled error is properly returned.

use std::sync::{Arc, Mutex};

use slicecore_engine::{
    CallbackSubscriber, CancellationToken, Engine, EngineError, EventBus, PrintConfig, SliceEvent,
};
use slicecore_math::Point3;
use slicecore_mesh::TriangleMesh;

/// Creates a 20mm calibration cube mesh centered at (100, 100) on a 220x220 bed.
fn make_cube_mesh() -> TriangleMesh {
    let ox = 90.0;
    let oy = 90.0;
    let vertices = vec![
        Point3::new(ox, oy, 0.0),
        Point3::new(ox + 20.0, oy, 0.0),
        Point3::new(ox + 20.0, oy + 20.0, 0.0),
        Point3::new(ox, oy + 20.0, 0.0),
        Point3::new(ox, oy, 20.0),
        Point3::new(ox + 20.0, oy, 20.0),
        Point3::new(ox + 20.0, oy + 20.0, 20.0),
        Point3::new(ox, oy + 20.0, 20.0),
    ];
    let indices = vec![
        [4, 5, 6],
        [4, 6, 7],
        [1, 0, 3],
        [1, 3, 2],
        [1, 2, 6],
        [1, 6, 5],
        [0, 4, 7],
        [0, 7, 3],
        [3, 7, 6],
        [3, 6, 2],
        [0, 1, 5],
        [0, 5, 4],
    ];
    TriangleMesh::new(vertices, indices).expect("cube mesh should be valid")
}

// ---------------------------------------------------------------------------
// Test 1: Pre-cancelled token returns EngineError::Cancelled
// ---------------------------------------------------------------------------

#[test]
fn test_cancellation_returns_cancelled_error() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let token = CancellationToken::new();
    token.cancel();

    let result = engine.slice(&mesh, Some(token));
    assert!(result.is_err(), "Expected Err when token is pre-cancelled");

    let err = result.unwrap_err();
    match err {
        EngineError::Cancelled => {}
        other => panic!("Expected EngineError::Cancelled, got: {:?}", other),
    }

    // Verify the error message contains "cancelled".
    let msg = format!("{}", err);
    assert!(
        msg.to_lowercase().contains("cancelled"),
        "Error message should contain 'cancelled', got: {}",
        msg
    );
}

// ---------------------------------------------------------------------------
// Test 2: Cancellation mid-slice via LayerComplete callback
// ---------------------------------------------------------------------------

#[test]
fn test_cancellation_mid_slice() {
    // Use sequential mode so LayerComplete events trigger cancellation callback.
    let config = PrintConfig {
        parallel_slicing: false,
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let token = CancellationToken::new();
    let token_for_callback = token.clone();

    let layer_count = Arc::new(Mutex::new(0usize));
    let layer_count_clone = Arc::clone(&layer_count);

    let mut bus = EventBus::new();
    bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
        if let SliceEvent::LayerComplete { .. } = e {
            let mut count = layer_count_clone.lock().unwrap();
            *count += 1;
            if *count >= 3 {
                token_for_callback.cancel();
            }
        }
    })));

    let result = engine.slice_with_events(&mesh, &bus, Some(token));
    assert!(result.is_err(), "Expected Err when cancelled mid-slice");

    match result.unwrap_err() {
        EngineError::Cancelled => {}
        other => panic!("Expected EngineError::Cancelled, got: {:?}", other),
    }

    // Should have processed at least 3 layers before cancellation.
    let count = *layer_count.lock().unwrap();
    assert!(
        count >= 3,
        "Expected at least 3 LayerComplete events before cancel, got {}",
        count
    );
}

// ---------------------------------------------------------------------------
// Test 3: None cancellation produces normal result
// ---------------------------------------------------------------------------

#[test]
fn test_no_cancellation_produces_normal_result() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let result = engine.slice(&mesh, None).expect("slice with None cancel should succeed");

    assert!(result.layer_count > 0, "Should have non-zero layer count");
    assert!(!result.gcode.is_empty(), "Should have non-empty gcode");
}

// ---------------------------------------------------------------------------
// Test 4: Progress events are emitted with correct fields
// ---------------------------------------------------------------------------

#[test]
fn test_progress_events_emitted() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let mut bus = EventBus::new();
    bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
        events_clone.lock().unwrap().push(e.clone());
    })));

    let _result = engine
        .slice_with_events(&mesh, &bus, None)
        .expect("slice should succeed");

    let all_events = events.lock().unwrap();
    let progress_events: Vec<_> = all_events
        .iter()
        .filter_map(|e| match e {
            SliceEvent::Progress {
                overall_percent,
                stage_percent,
                stage,
                layer,
                total_layers,
                elapsed_seconds,
                eta_seconds,
                layers_per_second,
            } => Some((
                *overall_percent,
                *stage_percent,
                stage.clone(),
                *layer,
                *total_layers,
                *elapsed_seconds,
                *eta_seconds,
                *layers_per_second,
            )),
            _ => None,
        })
        .collect();

    // At least 1 Progress event (cube has ~100 layers at 0.2mm).
    assert!(
        !progress_events.is_empty(),
        "Expected at least 1 Progress event"
    );

    // First Progress event has overall_percent >= 10.0 (layer processing starts at 10%).
    let first = &progress_events[0];
    assert!(
        first.0 >= 10.0,
        "First Progress overall_percent should be >= 10.0, got {}",
        first.0
    );

    // Last Progress event has overall_percent <= 90.0.
    let last = &progress_events[progress_events.len() - 1];
    assert!(
        last.0 <= 90.0,
        "Last Progress overall_percent should be <= 90.0, got {}",
        last.0
    );

    // All Progress events have stage == "layer_processing".
    for (i, evt) in progress_events.iter().enumerate() {
        assert_eq!(
            evt.2, "layer_processing",
            "Progress event {} should have stage 'layer_processing', got '{}'",
            i, evt.2
        );
    }

    // total_layers is consistent across all Progress events.
    let expected_total = progress_events[0].4;
    for (i, evt) in progress_events.iter().enumerate() {
        assert_eq!(
            evt.4, expected_total,
            "Progress event {} total_layers should be {}, got {}",
            i, expected_total, evt.4
        );
    }

    // layer values increase monotonically.
    for i in 1..progress_events.len() {
        assert!(
            progress_events[i].3 > progress_events[i - 1].3,
            "Layer values should increase monotonically: {} vs {}",
            progress_events[i - 1].3,
            progress_events[i].3
        );
    }

    // stage_percent increases monotonically.
    for i in 1..progress_events.len() {
        assert!(
            progress_events[i].1 >= progress_events[i - 1].1,
            "stage_percent should increase monotonically: {} vs {}",
            progress_events[i - 1].1,
            progress_events[i].1
        );
    }
}

// ---------------------------------------------------------------------------
// Test 5: ETA is None for first 3 layers, then Some
// ---------------------------------------------------------------------------

#[test]
fn test_progress_eta_none_for_first_layers() {
    // Use sequential mode so per-layer Progress events with ETA are emitted.
    let config = PrintConfig {
        parallel_slicing: false,
        ..PrintConfig::default()
    };
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = Arc::clone(&events);

    let mut bus = EventBus::new();
    bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
        events_clone.lock().unwrap().push(e.clone());
    })));

    let _result = engine
        .slice_with_events(&mesh, &bus, None)
        .expect("slice should succeed");

    let all_events = events.lock().unwrap();
    let progress_events: Vec<_> = all_events
        .iter()
        .filter_map(|e| match e {
            SliceEvent::Progress {
                layer, eta_seconds, ..
            } => Some((*layer, *eta_seconds)),
            _ => None,
        })
        .collect();

    // First 3 layers (layer indices 0, 1) should have eta_seconds == None.
    // ETA returns None when layers_done < 3 (i.e., layer_idx 0 and 1).
    for evt in progress_events.iter().take(2) {
        assert!(
            evt.1.is_none(),
            "Layer {} should have eta_seconds == None, got {:?}",
            evt.0,
            evt.1
        );
    }

    // After layer 3 (layer_idx >= 2, meaning layers_done >= 3), eta should be Some with positive value.
    // Find events where layers_done >= 3.
    let late_events: Vec<_> = progress_events
        .iter()
        .filter(|evt| evt.0 >= 2) // layer_idx >= 2 means layers_done >= 3
        .collect();

    assert!(
        !late_events.is_empty(),
        "Should have Progress events after 3 layers"
    );

    for evt in &late_events {
        assert!(
            evt.1.is_some(),
            "Layer {} should have eta_seconds == Some, got None",
            evt.0
        );
        if let Some(eta) = evt.1 {
            assert!(
                eta >= 0.0,
                "ETA should be non-negative, got {} for layer {}",
                eta,
                evt.0
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Test 6: CancellationToken clone shares state
// ---------------------------------------------------------------------------

#[test]
fn test_cancellation_token_clone_shares_state() {
    let token = CancellationToken::new();
    let clone = token.clone();

    assert!(!token.is_cancelled());
    assert!(!clone.is_cancelled());

    // Cancel the clone.
    clone.cancel();

    // Original should also be cancelled.
    assert!(token.is_cancelled());
    assert!(clone.is_cancelled());
}

// ---------------------------------------------------------------------------
// Test 7: slice_with_preview respects cancellation
// ---------------------------------------------------------------------------

#[test]
fn test_slice_with_preview_respects_cancellation() {
    let config = PrintConfig::default();
    let engine = Engine::new(config);
    let mesh = make_cube_mesh();

    let token = CancellationToken::new();
    token.cancel();

    let result = engine.slice_with_preview(&mesh, Some(token));
    assert!(result.is_err(), "Expected Err when token is pre-cancelled");

    match result.unwrap_err() {
        EngineError::Cancelled => {}
        other => panic!("Expected EngineError::Cancelled, got: {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Test 8: Cancelled error Display message
// ---------------------------------------------------------------------------

#[test]
fn test_cancelled_error_display() {
    let err = EngineError::Cancelled;
    let msg = format!("{}", err);
    assert_eq!(msg, "Slicing operation was cancelled");
}
