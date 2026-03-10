//! Gouraud shading with directional light and ambient term.

use crate::types::Vec3f;

/// Default light direction: upper-right-front, normalized.
pub(crate) const LIGHT_DIR: Vec3f = Vec3f {
    x: 0.577,
    y: 0.577,
    z: 0.577,
};

/// Default ambient light intensity.
pub(crate) const DEFAULT_AMBIENT: f32 = 0.2;

/// Computes the shading intensity for a vertex.
///
/// Uses a simple Lambertian diffuse model:
/// `intensity = ambient + max(0, dot(normal, light_dir)) * (1.0 - ambient)`
///
/// Returns a value in [ambient, 1.0].
pub(crate) fn shade_vertex(normal: Vec3f, light_dir: Vec3f, ambient: f32) -> f32 {
    let n_dot_l = normal.dot(light_dir).max(0.0);
    ambient + n_dot_l * (1.0 - ambient)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_facing_light_high_intensity() {
        let normal = LIGHT_DIR; // directly facing the light
        let intensity = shade_vertex(normal, LIGHT_DIR, DEFAULT_AMBIENT);
        // dot(normalized, normalized) ~ 1.0 (LIGHT_DIR isn't quite normalized, but close)
        assert!(
            intensity > DEFAULT_AMBIENT + 0.5,
            "Facing light should have high intensity, got {}",
            intensity
        );
    }

    #[test]
    fn normal_away_from_light_ambient_only() {
        let normal = Vec3f::new(-0.577, -0.577, -0.577).normalize(); // facing away
        let intensity = shade_vertex(normal, LIGHT_DIR, DEFAULT_AMBIENT);
        assert!(
            (intensity - DEFAULT_AMBIENT).abs() < 0.01,
            "Away from light should be ambient only, got {}",
            intensity
        );
    }

    #[test]
    fn perpendicular_normal_ambient_only() {
        // Perpendicular to light: should get ~ambient
        let normal = Vec3f::new(1.0, -1.0, 0.0).normalize();
        let intensity = shade_vertex(normal, LIGHT_DIR, DEFAULT_AMBIENT);
        // dot should be near 0, so intensity should be near ambient
        assert!(
            intensity >= DEFAULT_AMBIENT - 0.01,
            "Perpendicular should be near ambient, got {}",
            intensity
        );
    }
}
