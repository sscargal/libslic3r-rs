//! Symbolic perturbation for resolving coplanar face degeneracies.
//!
//! When geometric predicates (e.g. `orient3d`) return exactly zero (indicating
//! coplanar points), the simulation-of-simplicity (SoS) technique breaks ties
//! deterministically using vertex indices. This ensures coplanar triangles always
//! get a definitive above/below classification without modifying geometry.

use robust::Coord3D;

/// Helper to convert a 3-element array to `Coord3D<f64>`.
#[inline]
fn to_coord(p: [f64; 3]) -> Coord3D<f64> {
    Coord3D {
        x: p[0],
        y: p[1],
        z: p[2],
    }
}

/// Computes the orientation predicate with symbolic perturbation.
///
/// Calls [`robust::orient3d`] for exact orientation. If the result is exactly
/// `0.0` (coplanar), applies simulation-of-simplicity tie-breaking using the
/// vertex indices to produce a deterministic non-zero result.
///
/// # Arguments
///
/// * `pa`, `pb`, `pc`, `pd` -- The four 3D points to test.
/// * `idx_a`, `idx_b`, `idx_c`, `idx_d` -- Unique vertex indices used for
///   tie-breaking when points are coplanar.
///
/// # Returns
///
/// A non-zero `f64` whose sign indicates whether `pd` is above (+) or
/// below (-) the plane defined by `(pa, pb, pc)`.
///
/// # Examples
///
/// ```
/// use slicecore_mesh::csg::perturb::perturbed_orient3d;
///
/// // Non-coplanar points: returns same as robust::orient3d
/// let result = perturbed_orient3d(
///     [0.0, 0.0, 0.0], [1.0, 0.0, 0.0],
///     [0.0, 1.0, 0.0], [0.0, 0.0, 1.0],
///     0, 1, 2, 3,
/// );
/// assert!(result != 0.0);
/// ```
pub fn perturbed_orient3d(
    pa: [f64; 3],
    pb: [f64; 3],
    pc: [f64; 3],
    pd: [f64; 3],
    idx_a: usize,
    idx_b: usize,
    idx_c: usize,
    idx_d: usize,
) -> f64 {
    let result = robust::orient3d(to_coord(pa), to_coord(pb), to_coord(pc), to_coord(pd));

    if result != 0.0 {
        return result;
    }

    // Simulation of Simplicity (SoS) tie-breaking.
    // Sort the four vertex indices and use the sorted order to produce a
    // deterministic sign. The key property: given the same four geometric
    // points with the same index assignments, the result is always the same
    // non-zero value.
    let mut indexed = [(idx_a, 0u8), (idx_b, 1), (idx_c, 2), (idx_d, 3)];
    indexed.sort_by_key(|&(idx, _)| idx);

    // The sign is determined by the parity of the permutation that maps
    // the original order (a,b,c,d) = (0,1,2,3) to the sorted order.
    let perm: [u8; 4] = [
        indexed[0].1,
        indexed[1].1,
        indexed[2].1,
        indexed[3].1,
    ];

    let parity = permutation_parity(&perm);

    // Return a small non-zero value with the determined sign.
    if parity {
        1.0
    } else {
        -1.0
    }
}

/// Computes the parity of a permutation of 4 elements.
///
/// Returns `true` for even permutations, `false` for odd.
fn permutation_parity(perm: &[u8; 4]) -> bool {
    let mut inversions = 0u32;
    for i in 0..4 {
        for j in (i + 1)..4 {
            if perm[i] > perm[j] {
                inversions += 1;
            }
        }
    }
    inversions % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_coplanar_returns_same_as_robust() {
        let pa = [0.0, 0.0, 0.0];
        let pb = [1.0, 0.0, 0.0];
        let pc = [0.0, 1.0, 0.0];
        let pd = [0.0, 0.0, 1.0];

        let expected = robust::orient3d(to_coord(pa), to_coord(pb), to_coord(pc), to_coord(pd));
        let result = perturbed_orient3d(pa, pb, pc, pd, 0, 1, 2, 3);

        assert_eq!(result, expected, "non-coplanar points should match robust::orient3d");
    }

    #[test]
    fn coplanar_points_produce_nonzero() {
        // Four coplanar points in the XY plane.
        let pa = [0.0, 0.0, 0.0];
        let pb = [1.0, 0.0, 0.0];
        let pc = [0.0, 1.0, 0.0];
        let pd = [0.5, 0.5, 0.0];

        let result = perturbed_orient3d(pa, pb, pc, pd, 0, 1, 2, 3);
        assert!(result != 0.0, "coplanar points should produce non-zero result");
    }

    #[test]
    fn coplanar_result_is_deterministic() {
        let pa = [0.0, 0.0, 0.0];
        let pb = [1.0, 0.0, 0.0];
        let pc = [0.0, 1.0, 0.0];
        let pd = [0.5, 0.5, 0.0];

        let r1 = perturbed_orient3d(pa, pb, pc, pd, 10, 20, 30, 40);
        let r2 = perturbed_orient3d(pa, pb, pc, pd, 10, 20, 30, 40);
        assert_eq!(r1, r2, "same indices should produce same result");
    }

    #[test]
    fn different_indices_can_produce_different_sign() {
        let pa = [0.0, 0.0, 0.0];
        let pb = [1.0, 0.0, 0.0];
        let pc = [0.0, 1.0, 0.0];
        let pd = [0.5, 0.5, 0.0];

        // Swapping indices changes the permutation parity.
        let r1 = perturbed_orient3d(pa, pb, pc, pd, 0, 1, 2, 3);
        let r2 = perturbed_orient3d(pa, pb, pc, pd, 1, 0, 2, 3);
        // They should both be non-zero.
        assert!(r1 != 0.0);
        assert!(r2 != 0.0);
        // And may differ in sign (depends on permutation parity).
        assert_ne!(r1, r2, "swapped indices should produce different sign");
    }

    #[test]
    fn identical_points_with_different_indices() {
        // All four points are identical (maximally degenerate).
        let p = [1.0, 2.0, 3.0];
        let result = perturbed_orient3d(p, p, p, p, 5, 10, 15, 20);
        assert!(result != 0.0, "even identical points should get a non-zero result");
    }
}
