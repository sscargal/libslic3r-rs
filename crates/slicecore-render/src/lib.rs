//! CPU software triangle rasterizer for thumbnail/preview image generation.
//!
//! This crate provides a complete software rendering pipeline that converts
//! a [`TriangleMesh`] into RGBA pixel buffers
//! and PNG-encoded images from multiple camera angles. It is used for:
//!
//! - 3MF thumbnail embedding
//! - G-code preview images
//! - Print preview generation
//!
//! The renderer uses orthographic projection, Gouraud shading, and z-buffered
//! scanline rasterization -- all in software with no GPU or external rendering
//! dependencies, ensuring full WASM compatibility.
//!
//! # Usage
//!
//! ```rust,no_run
//! use slicecore_render::{render_mesh, ThumbnailConfig, CameraAngle};
//! # use slicecore_mesh::TriangleMesh;
//! # fn example(mesh: &TriangleMesh) {
//! let config = ThumbnailConfig {
//!     angles: CameraAngle::all(),
//!     ..ThumbnailConfig::default()
//! };
//! let thumbnails = render_mesh(mesh, &config);
//! for thumb in &thumbnails {
//!     // thumb.png_data contains the PNG-encoded image
//!     // thumb.rgba contains raw RGBA pixel data
//! }
//! # }
//! ```

mod camera;
mod framebuffer;
pub mod gcode_embed;
mod pipeline;
mod png_encode;
mod rasterizer;
mod shading;
#[allow(dead_code)]
mod types;

pub use camera::CameraAngle;
pub use gcode_embed::{
    format_gcode_thumbnail_block, thumbnail_format_for_dialect, ThumbnailFormat,
};

use slicecore_mesh::TriangleMesh;

/// Configuration for thumbnail rendering.
pub struct ThumbnailConfig {
    /// Output image width in pixels.
    pub width: u32,
    /// Output image height in pixels.
    pub height: u32,
    /// Camera angles to render from.
    pub angles: Vec<CameraAngle>,
    /// Background color as RGBA.
    pub background: [u8; 4],
    /// Model surface color as RGB.
    pub model_color: [u8; 3],
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            width: 300,
            height: 300,
            angles: vec![CameraAngle::Isometric],
            background: [0, 0, 0, 0],     // transparent
            model_color: [200, 200, 200], // #C8C8C8 light gray
        }
    }
}

/// A rendered thumbnail image.
pub struct Thumbnail {
    /// The camera angle this thumbnail was rendered from.
    pub angle: CameraAngle,
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// Raw RGBA pixel data (row-major, top-to-bottom).
    pub rgba: Vec<[u8; 4]>,
    /// PNG-encoded image data.
    pub png_data: Vec<u8>,
}

/// Renders a mesh into thumbnails from the configured camera angles.
///
/// For each angle in `config.angles`, produces a [`Thumbnail`] containing
/// both raw RGBA pixel data and PNG-encoded image data.
pub fn render_mesh(mesh: &TriangleMesh, config: &ThumbnailConfig) -> Vec<Thumbnail> {
    config
        .angles
        .iter()
        .map(|angle| {
            let fb = pipeline::render_to_framebuffer(mesh, angle, config);
            let png_data = png_encode::encode_png(config.width, config.height, fb.pixels());

            Thumbnail {
                angle: angle.clone(),
                width: config.width,
                height: config.height,
                rgba: fb.pixels().to_vec(),
                png_data,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use slicecore_math::Point3;

    /// Creates a simple cube mesh for testing.
    fn make_cube() -> TriangleMesh {
        let vertices = vec![
            Point3::new(0.0, 0.0, 0.0),
            Point3::new(10.0, 0.0, 0.0),
            Point3::new(10.0, 10.0, 0.0),
            Point3::new(0.0, 10.0, 0.0),
            Point3::new(0.0, 0.0, 10.0),
            Point3::new(10.0, 0.0, 10.0),
            Point3::new(10.0, 10.0, 10.0),
            Point3::new(0.0, 10.0, 10.0),
        ];
        let indices = vec![
            // Front (z=0)
            [0, 2, 1],
            [0, 3, 2],
            // Back (z=10)
            [4, 5, 6],
            [4, 6, 7],
            // Bottom (y=0)
            [0, 1, 5],
            [0, 5, 4],
            // Top (y=10)
            [3, 6, 2],
            [3, 7, 6],
            // Left (x=0)
            [0, 4, 7],
            [0, 7, 3],
            // Right (x=10)
            [1, 2, 6],
            [1, 6, 5],
        ];
        TriangleMesh::new(vertices, indices).unwrap()
    }

    #[test]
    fn render_mesh_default_config_returns_one_thumbnail() {
        let mesh = make_cube();
        let config = ThumbnailConfig::default();
        let thumbs = render_mesh(&mesh, &config);
        assert_eq!(thumbs.len(), 1);
        assert_eq!(thumbs[0].width, 300);
        assert_eq!(thumbs[0].height, 300);
        assert!(!thumbs[0].png_data.is_empty());
        assert_eq!(thumbs[0].rgba.len(), 300 * 300);
    }

    #[test]
    fn render_mesh_all_angles_returns_six() {
        let mesh = make_cube();
        let config = ThumbnailConfig {
            width: 100,
            height: 100,
            angles: CameraAngle::all(),
            background: [0, 0, 0, 0],
            model_color: [200, 200, 200],
        };
        let thumbs = render_mesh(&mesh, &config);
        assert_eq!(thumbs.len(), 6);
    }

    #[test]
    fn render_mesh_isometric_has_non_background_pixels() {
        let mesh = make_cube();
        let config = ThumbnailConfig {
            width: 100,
            height: 100,
            angles: vec![CameraAngle::Isometric],
            background: [0, 0, 0, 0],
            model_color: [200, 200, 200],
        };
        let thumbs = render_mesh(&mesh, &config);
        assert_eq!(thumbs.len(), 1);

        let non_bg = thumbs[0]
            .rgba
            .iter()
            .filter(|px| **px != [0, 0, 0, 0])
            .count();
        assert!(
            non_bg > 0,
            "Isometric render should have some non-background pixels"
        );
    }

    #[test]
    fn render_mesh_six_angles_have_distinct_content() {
        let mesh = make_cube();
        let config = ThumbnailConfig {
            width: 50,
            height: 50,
            angles: CameraAngle::all(),
            background: [0, 0, 0, 0],
            model_color: [200, 200, 200],
        };
        let thumbs = render_mesh(&mesh, &config);

        // Count non-background pixels for each angle
        let counts: Vec<usize> = thumbs
            .iter()
            .map(|t| t.rgba.iter().filter(|px| **px != [0, 0, 0, 0]).count())
            .collect();

        // At least some angles should have different pixel counts (distinct content)
        let all_same = counts.windows(2).all(|w| w[0] == w[1]);
        assert!(
            !all_same,
            "All 6 angles should not produce identical pixel counts: {:?}",
            counts
        );
    }

    #[test]
    fn render_mesh_png_valid_magic() {
        let mesh = make_cube();
        let config = ThumbnailConfig::default();
        let thumbs = render_mesh(&mesh, &config);
        let png = &thumbs[0].png_data;

        assert!(png.len() > 8);
        assert_eq!(png[0], 0x89);
        assert_eq!(png[1], b'P');
        assert_eq!(png[2], b'N');
        assert_eq!(png[3], b'G');
    }

    #[test]
    fn render_empty_mesh_returns_background_only() {
        // TriangleMesh::new requires non-empty vertices and indices, so we can't
        // truly test an empty mesh. Instead, test that a tiny mesh (single degenerate
        // triangle with vertices at same point) produces mostly background.
        let vertices = vec![
            Point3::new(5.0, 5.0, 5.0),
            Point3::new(5.0, 5.0, 5.0),
            Point3::new(5.0, 5.0, 5.0),
        ];
        let indices = vec![[0, 1, 2]];
        let mesh = TriangleMesh::new(vertices, indices).unwrap();
        let config = ThumbnailConfig {
            width: 50,
            height: 50,
            angles: vec![CameraAngle::Isometric],
            background: [255, 255, 255, 255],
            model_color: [200, 200, 200],
        };
        let thumbs = render_mesh(&mesh, &config);
        // Degenerate triangle should produce ~all background
        let bg_count = thumbs[0]
            .rgba
            .iter()
            .filter(|px| **px == [255, 255, 255, 255])
            .count();
        let total = (50 * 50) as usize;
        assert!(
            bg_count > total * 9 / 10,
            "Degenerate mesh should be mostly background: {} of {}",
            bg_count,
            total
        );
    }
}
