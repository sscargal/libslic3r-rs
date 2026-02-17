//! WASM spiral infill plugin for slicecore.
//!
//! This is an example plugin demonstrating how to create a WebAssembly
//! infill pattern plugin using the Component Model. It generates a spiral
//! infill pattern -- concentric inward-spiraling paths from the bounding
//! box toward the center.
//!
//! # Building
//!
//! ```bash
//! # Ensure the wasm32-wasip2 target is installed:
//! rustup target add wasm32-wasip2
//!
//! # Build the WASM component:
//! cargo build --target wasm32-wasip2 \
//!     --manifest-path plugins/examples/wasm-spiral-infill/Cargo.toml
//! ```
//!
//! The resulting `.wasm` file appears in `target/wasm32-wasip2/debug/` and
//! can be placed alongside the `plugin.toml` manifest for discovery by the
//! slicecore `PluginRegistry`.

wit_bindgen::generate!({
    world: "infill-plugin",
    path: "wit/slicecore-plugin.wit",
});

use slicecore::plugin::types::{InfillLine, Point2};

struct SpiralInfillPlugin;

export!(SpiralInfillPlugin);

impl Guest for SpiralInfillPlugin {
    fn name() -> String {
        "spiral".to_string()
    }

    fn description() -> String {
        "Spiral infill: concentric inward-spiraling paths for smooth top surfaces".to_string()
    }

    fn generate(request: InfillRequest) -> Result<InfillResult, String> {
        let (min_x, min_y, max_x, max_y) = compute_bbox(&request.boundary_points);

        // Need valid bounding box
        if min_x >= max_x || min_y >= max_y {
            return Ok(InfillResult { lines: Vec::new() });
        }

        let center_x = (min_x + max_x) / 2;
        let center_y = (min_y + max_y) / 2;

        let width = (max_x - min_x) as f64;
        let height = (max_y - min_y) as f64;
        let max_radius = width.min(height) / 2.0;

        // Spacing between spiral arms in mm, then convert to coordinates
        let density = if request.density < 0.01 {
            0.01
        } else if request.density > 1.0 {
            1.0
        } else {
            request.density
        };
        let spacing_mm = request.line_width / density;
        let coord_scale: f64 = 1_000_000.0;
        let spacing_coord = spacing_mm * coord_scale;

        if spacing_coord <= 0.0 {
            return Ok(InfillResult { lines: Vec::new() });
        }

        let mut lines = Vec::new();
        let num_revolutions = (max_radius / spacing_coord).ceil() as usize;
        if num_revolutions == 0 {
            return Ok(InfillResult { lines: Vec::new() });
        }

        let steps_per_rev: usize = 36; // 10-degree steps
        let total_steps = num_revolutions * steps_per_rev;

        let mut prev_x = center_x + (max_radius as i64);
        let mut prev_y = center_y;

        for step in 1..=total_steps {
            let angle =
                (step as f64) * (2.0 * core::f64::consts::PI / steps_per_rev as f64);
            let radius =
                max_radius - (step as f64 * spacing_coord / steps_per_rev as f64);

            if radius <= 0.0 {
                break;
            }

            let x = center_x + (radius * angle.cos()) as i64;
            let y = center_y + (radius * angle.sin()) as i64;

            lines.push(InfillLine {
                start: Point2 { x: prev_x, y: prev_y },
                end: Point2 { x, y },
            });

            prev_x = x;
            prev_y = y;
        }

        Ok(InfillResult { lines })
    }
}

/// Compute bounding box from a list of Point2 values.
/// Returns (min_x, min_y, max_x, max_y).
fn compute_bbox(points: &[Point2]) -> (i64, i64, i64, i64) {
    if points.is_empty() {
        return (0, 0, 0, 0);
    }

    let mut min_x = i64::MAX;
    let mut min_y = i64::MAX;
    let mut max_x = i64::MIN;
    let mut max_y = i64::MIN;

    for p in points {
        min_x = min_x.min(p.x);
        min_y = min_y.min(p.y);
        max_x = max_x.max(p.x);
        max_y = max_y.max(p.y);
    }

    (min_x, min_y, max_x, max_y)
}
