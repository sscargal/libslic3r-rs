//! Event system for monitoring slicing pipeline progress.
//!
//! Provides a pub/sub [`EventBus`] that dispatches [`SliceEvent`]s to
//! registered [`EventSubscriber`] implementations. Use this to monitor
//! long-running slicing operations for progress reporting, logging, or
//! streaming output.
//!
//! # Built-in subscribers
//!
//! - [`CallbackSubscriber`]: Wraps a closure for ad-hoc event handling
//! - [`NdjsonSubscriber`]: Writes each event as a newline-delimited JSON line
//!
//! # Example
//!
//! ```
//! use slicecore_engine::event::{EventBus, CallbackSubscriber, SliceEvent};
//! use std::sync::{Arc, Mutex};
//!
//! let events = Arc::new(Mutex::new(Vec::new()));
//! let events_clone = Arc::clone(&events);
//!
//! let mut bus = EventBus::new();
//! bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
//!     events_clone.lock().unwrap().push(e.clone());
//! })));
//!
//! bus.emit(&SliceEvent::StageChanged {
//!     stage: "slicing".to_string(),
//!     progress: 0.0,
//! });
//!
//! assert_eq!(events.lock().unwrap().len(), 1);
//! ```

use std::io::Write;
use std::sync::{Arc, Mutex};

use serde::Serialize;

/// Events emitted during the slicing pipeline.
///
/// Each variant represents a distinct lifecycle or diagnostic event.
/// Events are tagged with `#[serde(tag = "type")]` for clean JSON output.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SliceEvent {
    /// Emitted at pipeline stage transitions (e.g., mesh slicing, per-layer
    /// processing, G-code generation).
    StageChanged {
        /// Name of the pipeline stage.
        stage: String,
        /// Progress fraction (0.0 to 1.0) within the overall pipeline.
        progress: f32,
    },

    /// Emitted after each layer is processed.
    LayerComplete {
        /// Zero-based layer index.
        layer: usize,
        /// Total number of layers.
        total: usize,
        /// Z height of the completed layer in mm.
        z: f64,
    },

    /// Non-fatal warning during slicing.
    Warning {
        /// Human-readable warning message.
        message: String,
        /// Layer index where the warning occurred, if applicable.
        layer: Option<usize>,
    },

    /// Fatal error during slicing.
    Error {
        /// Human-readable error message.
        message: String,
    },

    /// Timing data for a pipeline stage.
    PerformanceMetric {
        /// Name of the timed stage.
        stage: String,
        /// Duration of the stage in milliseconds.
        duration_ms: u64,
    },

    /// Final summary emitted after slicing completes.
    Complete {
        /// Total number of layers produced.
        layers: usize,
        /// Total slicing time in seconds.
        time_seconds: f64,
    },
}

/// Trait for receiving slicing events.
///
/// Implementations must be `Send + Sync` to allow use from any thread.
pub trait EventSubscriber: Send + Sync {
    /// Called when an event is emitted on the [`EventBus`].
    fn on_event(&self, event: &SliceEvent);
}

/// Dispatches [`SliceEvent`]s to registered [`EventSubscriber`]s.
///
/// Subscribers are invoked in registration order. The bus takes ownership
/// of subscribers via `Box<dyn EventSubscriber>`.
pub struct EventBus {
    subscribers: Vec<Box<dyn EventSubscriber>>,
}

impl EventBus {
    /// Creates a new empty event bus with no subscribers.
    pub fn new() -> Self {
        Self {
            subscribers: Vec::new(),
        }
    }

    /// Registers a subscriber to receive future events.
    pub fn subscribe(&mut self, subscriber: Box<dyn EventSubscriber>) {
        self.subscribers.push(subscriber);
    }

    /// Dispatches an event to all registered subscribers.
    pub fn emit(&self, event: &SliceEvent) {
        for sub in &self.subscribers {
            sub.on_event(event);
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

/// A subscriber that delegates to a closure.
///
/// Wraps any `Fn(&SliceEvent) + Send + Sync` for convenient inline
/// event handling without defining a separate struct.
pub struct CallbackSubscriber<F>
where
    F: Fn(&SliceEvent) + Send + Sync,
{
    callback: F,
}

impl<F> CallbackSubscriber<F>
where
    F: Fn(&SliceEvent) + Send + Sync,
{
    /// Creates a new callback subscriber from the given closure.
    pub fn new(callback: F) -> Self {
        Self { callback }
    }
}

impl<F> EventSubscriber for CallbackSubscriber<F>
where
    F: Fn(&SliceEvent) + Send + Sync,
{
    fn on_event(&self, event: &SliceEvent) {
        (self.callback)(event);
    }
}

/// A subscriber that writes each event as a newline-delimited JSON line.
///
/// Useful for streaming event data to files, pipes, or network sockets.
/// Each event is serialized as a single JSON object followed by a newline.
pub struct NdjsonSubscriber {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl NdjsonSubscriber {
    /// Creates a new NDJSON subscriber writing to the given output.
    pub fn new(writer: Box<dyn Write + Send>) -> Self {
        Self {
            writer: Arc::new(Mutex::new(writer)),
        }
    }
}

impl EventSubscriber for NdjsonSubscriber {
    fn on_event(&self, event: &SliceEvent) {
        if let Ok(mut w) = self.writer.lock() {
            if let Ok(json) = serde_json::to_string(event) {
                let _ = writeln!(w, "{}", json);
            }
        }
    }
}

// NdjsonSubscriber is Send + Sync because Arc<Mutex<..>> provides thread safety.
// The compiler derives this automatically given the bounds above.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn event_bus_dispatches_to_callback_subscriber() {
        let events: Arc<Mutex<Vec<SliceEvent>>> = Arc::new(Mutex::new(Vec::new()));
        let events_clone = Arc::clone(&events);

        let mut bus = EventBus::new();
        bus.subscribe(Box::new(CallbackSubscriber::new(move |e: &SliceEvent| {
            events_clone.lock().unwrap().push(e.clone());
        })));

        bus.emit(&SliceEvent::StageChanged {
            stage: "slicing".to_string(),
            progress: 0.0,
        });
        bus.emit(&SliceEvent::LayerComplete {
            layer: 0,
            total: 10,
            z: 0.2,
        });
        bus.emit(&SliceEvent::Warning {
            message: "test warning".to_string(),
            layer: Some(3),
        });
        bus.emit(&SliceEvent::Error {
            message: "test error".to_string(),
        });
        bus.emit(&SliceEvent::PerformanceMetric {
            stage: "infill".to_string(),
            duration_ms: 42,
        });
        bus.emit(&SliceEvent::Complete {
            layers: 10,
            time_seconds: 1.5,
        });

        let captured = events.lock().unwrap();
        assert_eq!(captured.len(), 6);

        // Verify first event is StageChanged.
        match &captured[0] {
            SliceEvent::StageChanged { stage, progress } => {
                assert_eq!(stage, "slicing");
                assert!(*progress < f32::EPSILON);
            }
            other => panic!("expected StageChanged, got {:?}", other),
        }

        // Verify LayerComplete.
        match &captured[1] {
            SliceEvent::LayerComplete { layer, total, z } => {
                assert_eq!(*layer, 0);
                assert_eq!(*total, 10);
                assert!((z - 0.2).abs() < 1e-9);
            }
            other => panic!("expected LayerComplete, got {:?}", other),
        }
    }

    #[test]
    fn event_bus_dispatches_to_multiple_subscribers() {
        let count1 = Arc::new(Mutex::new(0u32));
        let count2 = Arc::new(Mutex::new(0u32));
        let c1 = Arc::clone(&count1);
        let c2 = Arc::clone(&count2);

        let mut bus = EventBus::new();
        bus.subscribe(Box::new(CallbackSubscriber::new(move |_: &SliceEvent| {
            *c1.lock().unwrap() += 1;
        })));
        bus.subscribe(Box::new(CallbackSubscriber::new(move |_: &SliceEvent| {
            *c2.lock().unwrap() += 1;
        })));

        bus.emit(&SliceEvent::Complete {
            layers: 5,
            time_seconds: 1.0,
        });

        assert_eq!(*count1.lock().unwrap(), 1);
        assert_eq!(*count2.lock().unwrap(), 1);
    }

    #[test]
    fn ndjson_subscriber_produces_valid_json_lines() {
        let buffer: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let buf_clone = Arc::clone(&buffer);

        let writer: Box<dyn Write + Send> = Box::new(SharedWriter(Arc::clone(&buffer)));
        let sub = NdjsonSubscriber::new(writer);

        let mut bus = EventBus::new();
        bus.subscribe(Box::new(sub));

        bus.emit(&SliceEvent::StageChanged {
            stage: "slicing".to_string(),
            progress: 0.5,
        });
        bus.emit(&SliceEvent::LayerComplete {
            layer: 3,
            total: 20,
            z: 0.8,
        });

        let data = buf_clone.lock().unwrap();
        let output = String::from_utf8(data.clone()).unwrap();
        let lines: Vec<&str> = output.trim().split('\n').collect();
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON.
        let v0: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(v0["type"], "StageChanged");
        assert_eq!(v0["stage"], "slicing");

        let v1: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(v1["type"], "LayerComplete");
        assert_eq!(v1["layer"], 3);
    }

    #[test]
    fn empty_bus_emit_does_not_panic() {
        let bus = EventBus::new();
        bus.emit(&SliceEvent::Complete {
            layers: 0,
            time_seconds: 0.0,
        });
    }

    #[test]
    fn slice_event_serializes_with_type_tag() {
        let event = SliceEvent::Warning {
            message: "thin wall".to_string(),
            layer: Some(5),
        };
        let json = serde_json::to_string(&event).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["type"], "Warning");
        assert_eq!(v["message"], "thin wall");
        assert_eq!(v["layer"], 5);
    }

    /// Helper: a `Write` impl that writes to a shared buffer.
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    // Verify Send + Sync on NdjsonSubscriber (compile-time check).
    fn _assert_send_sync<T: Send + Sync>() {}
    fn _check() {
        _assert_send_sync::<NdjsonSubscriber>();
        _assert_send_sync::<CallbackSubscriber<fn(&SliceEvent)>>();
        _assert_send_sync::<EventBus>();
    }
}
