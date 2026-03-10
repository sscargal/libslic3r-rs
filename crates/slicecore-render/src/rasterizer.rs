//! Scanline triangle rasterization with edge functions.

use crate::framebuffer::Framebuffer;

/// A screen-space vertex with interpolated color.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ScreenVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

/// Computes the signed area of triangle (a, b, p) using the 2D cross product.
/// `(b - a) x (p - a)`. Positive when p is to the left of edge a->b (CCW).
#[inline]
fn edge_function(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Rasterizes a single triangle into the framebuffer using edge functions.
///
/// Back-face culling: triangles with clockwise screen winding (area <= 0) are skipped.
/// Uses barycentric coordinate interpolation for z-depth and vertex colors.
pub(crate) fn rasterize_triangle(
    fb: &mut Framebuffer,
    v0: ScreenVertex,
    v1: ScreenVertex,
    v2: ScreenVertex,
) {
    // Compute signed area (2x) using standard 2D cross product:
    // (v1 - v0) x (v2 - v0) = (v1.x-v0.x)*(v2.y-v0.y) - (v1.y-v0.y)*(v2.x-v0.x)
    // Positive for CCW winding in screen space (Y-down).
    let area = (v1.x - v0.x) * (v2.y - v0.y) - (v1.y - v0.y) * (v2.x - v0.x);
    if area <= 0.0 {
        return; // Back-face cull (CW or degenerate)
    }

    let inv_area = 1.0 / area;

    // Bounding box, clamped to framebuffer
    let min_x = v0.x.min(v1.x).min(v2.x).max(0.0) as u32;
    let max_x = v0
        .x
        .max(v1.x)
        .max(v2.x)
        .min(fb.width() as f32 - 1.0)
        .max(0.0) as u32;
    let min_y = v0.y.min(v1.y).min(v2.y).max(0.0) as u32;
    let max_y = v0
        .y
        .max(v1.y)
        .max(v2.y)
        .min(fb.height() as f32 - 1.0)
        .max(0.0) as u32;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            let w0 = edge_function(v1.x, v1.y, v2.x, v2.y, px, py);
            let w1 = edge_function(v2.x, v2.y, v0.x, v0.y, px, py);
            let w2 = edge_function(v0.x, v0.y, v1.x, v1.y, px, py);

            if w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0 {
                let w0 = w0 * inv_area;
                let w1 = w1 * inv_area;
                let w2 = w2 * inv_area;

                // Interpolate depth
                let z = v0.z * w0 + v1.z * w1 + v2.z * w2;

                // Interpolate color
                let r = (v0.r * w0 + v1.r * w1 + v2.r * w2).clamp(0.0, 255.0) as u8;
                let g = (v0.g * w0 + v1.g * w1 + v2.g * w2).clamp(0.0, 255.0) as u8;
                let b = (v0.b * w0 + v1.b * w1 + v2.b * w2).clamp(0.0, 255.0) as u8;

                fb.set_pixel(x, y, z, [r, g, b, 255]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rasterize_ccw_triangle_fills_pixels() {
        let mut fb = Framebuffer::new(20, 20, [0, 0, 0, 0]);

        // A CCW triangle covering roughly the center
        let v0 = ScreenVertex {
            x: 5.0,
            y: 5.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };
        let v1 = ScreenVertex {
            x: 15.0,
            y: 5.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };
        let v2 = ScreenVertex {
            x: 10.0,
            y: 15.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };

        rasterize_triangle(&mut fb, v0, v1, v2);

        // Check that some interior pixel is filled
        let idx = 10 * 20 + 10; // (10, 10) should be inside
        assert_ne!(fb.pixels()[idx], [0, 0, 0, 0], "Interior pixel should be filled");
    }

    #[test]
    fn cw_triangle_is_culled() {
        let mut fb = Framebuffer::new(20, 20, [0, 0, 0, 0]);

        // CW winding (swap v1/v2)
        let v0 = ScreenVertex {
            x: 5.0,
            y: 5.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };
        let v1 = ScreenVertex {
            x: 10.0,
            y: 15.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };
        let v2 = ScreenVertex {
            x: 15.0,
            y: 5.0,
            z: 0.5,
            r: 200.0,
            g: 100.0,
            b: 50.0,
        };

        rasterize_triangle(&mut fb, v0, v1, v2);

        // All pixels should remain background
        for px in fb.pixels() {
            assert_eq!(*px, [0, 0, 0, 0], "CW triangle should be culled");
        }
    }
}
