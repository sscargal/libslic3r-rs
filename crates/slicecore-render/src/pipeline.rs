//! Render pipeline: mesh -> framebuffer.
//!
//! Transforms mesh triangles through the view/projection pipeline,
//! applies Gouraud shading, and rasterizes to a framebuffer.

use slicecore_mesh::TriangleMesh;

use crate::camera::{build_camera, compute_vertex_normals, CameraAngle};
use crate::framebuffer::Framebuffer;
use crate::rasterizer::{rasterize_triangle, ScreenVertex};
use crate::shading::{shade_vertex, DEFAULT_AMBIENT, LIGHT_DIR};
use crate::types::Vec3f;
use crate::ThumbnailConfig;

/// Renders a mesh from a given camera angle into a framebuffer.
pub(crate) fn render_to_framebuffer(
    mesh: &TriangleMesh,
    angle: &CameraAngle,
    config: &ThumbnailConfig,
) -> Framebuffer {
    let mut fb = Framebuffer::new(config.width, config.height, config.background);

    // Handle empty/degenerate mesh
    if mesh.indices().is_empty() {
        return fb;
    }

    let vertices = mesh.vertices();
    let indices = mesh.indices();

    // Compute smooth vertex normals
    let vertex_normals = compute_vertex_normals(vertices, indices);

    // Build camera matrices
    let (view, proj) = build_camera(angle, mesh.aabb(), config.width, config.height);
    let mvp = proj.multiply(&view); // No model matrix -- mesh is in world space

    let w = config.width as f32;
    let h = config.height as f32;

    let model_r = config.model_color[0] as f32;
    let model_g = config.model_color[1] as f32;
    let model_b = config.model_color[2] as f32;

    for tri in indices {
        let i0 = tri[0] as usize;
        let i1 = tri[1] as usize;
        let i2 = tri[2] as usize;

        // Transform vertices to clip space
        let p0 = Vec3f::from_point3(&vertices[i0]);
        let p1 = Vec3f::from_point3(&vertices[i1]);
        let p2 = Vec3f::from_point3(&vertices[i2]);

        let clip0 = mvp.transform_point3(p0);
        let clip1 = mvp.transform_point3(p1);
        let clip2 = mvp.transform_point3(p2);

        // Perspective divide (w=1 for ortho, but keep general)
        let ndc0_x = clip0.x / clip0.w;
        let ndc0_y = clip0.y / clip0.w;
        let ndc0_z = clip0.z / clip0.w;

        let ndc1_x = clip1.x / clip1.w;
        let ndc1_y = clip1.y / clip1.w;
        let ndc1_z = clip1.z / clip1.w;

        let ndc2_x = clip2.x / clip2.w;
        let ndc2_y = clip2.y / clip2.w;
        let ndc2_z = clip2.z / clip2.w;

        // Viewport transform: NDC [-1,1] -> screen [0, width/height], flip Y
        let sx0 = (ndc0_x + 1.0) * 0.5 * w;
        let sy0 = (1.0 - ndc0_y) * 0.5 * h; // flip Y
        let sx1 = (ndc1_x + 1.0) * 0.5 * w;
        let sy1 = (1.0 - ndc1_y) * 0.5 * h;
        let sx2 = (ndc2_x + 1.0) * 0.5 * w;
        let sy2 = (1.0 - ndc2_y) * 0.5 * h;

        // Shade vertices (Gouraud: per-vertex shading, interpolated across triangle)
        let int0 = shade_vertex(vertex_normals[i0], LIGHT_DIR, DEFAULT_AMBIENT);
        let int1 = shade_vertex(vertex_normals[i1], LIGHT_DIR, DEFAULT_AMBIENT);
        let int2 = shade_vertex(vertex_normals[i2], LIGHT_DIR, DEFAULT_AMBIENT);

        let sv0 = ScreenVertex {
            x: sx0,
            y: sy0,
            z: ndc0_z,
            r: model_r * int0,
            g: model_g * int0,
            b: model_b * int0,
        };
        let sv1 = ScreenVertex {
            x: sx1,
            y: sy1,
            z: ndc1_z,
            r: model_r * int1,
            g: model_g * int1,
            b: model_b * int1,
        };
        let sv2 = ScreenVertex {
            x: sx2,
            y: sy2,
            z: ndc2_z,
            r: model_r * int2,
            g: model_g * int2,
            b: model_b * int2,
        };

        rasterize_triangle(&mut fb, sv0, sv1, sv2);
    }

    fb
}
