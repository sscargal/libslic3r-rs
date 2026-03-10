//! CPU software triangle rasterizer for thumbnail/preview image generation.
//!
//! This crate provides a complete software rendering pipeline that converts
//! a [`TriangleMesh`](slicecore_mesh::TriangleMesh) into RGBA pixel buffers
//! and PNG-encoded images from multiple camera angles. It is used for:
//!
//! - 3MF thumbnail embedding
//! - G-code preview images
//! - Print preview generation
//!
//! The renderer uses orthographic projection, Gouraud shading, and z-buffered
//! scanline rasterization -- all in software with no GPU or external rendering
//! dependencies, ensuring full WASM compatibility.

mod camera;
mod framebuffer;
mod pipeline;
mod png_encode;
mod rasterizer;
mod shading;
mod types;

pub use camera::CameraAngle;
