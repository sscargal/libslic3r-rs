//! CSG (Constructive Solid Geometry) CLI subcommands.
//!
//! Provides boolean operations (union, difference, intersection, xor),
//! plane splitting, mesh hollowing, primitive generation, and mesh
//! information display from the command line.

use std::path::PathBuf;
use std::time::Instant;

use clap::{Subcommand, ValueEnum};

use slicecore_fileio::{load_mesh, save_mesh};
use slicecore_math::Point3;
use slicecore_math::Vec3;
use slicecore_mesh::csg::{
    hollow_mesh, mesh_difference, mesh_intersection, mesh_split_at_plane, mesh_union, mesh_xor,
    primitive_box, primitive_cone, primitive_cylinder, primitive_ngon_prism, primitive_plane,
    primitive_rounded_box, primitive_sphere, primitive_torus, primitive_wedge, CsgReport,
    DrainHole, HollowOptions, SplitOptions, SplitPlane,
};

/// CSG subcommands for mesh boolean operations, splitting, hollowing,
/// primitive generation, and mesh information.
#[derive(Subcommand)]
pub enum CsgCommand {
    /// Compute boolean union of two meshes
    Union {
        /// First input mesh file
        a: PathBuf,
        /// Second input mesh file
        b: PathBuf,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Compute boolean difference (A minus B)
    Difference {
        /// First input mesh file
        a: PathBuf,
        /// Second input mesh file
        b: PathBuf,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Compute boolean intersection
    Intersection {
        /// First input mesh file
        a: PathBuf,
        /// Second input mesh file
        b: PathBuf,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Compute boolean XOR (symmetric difference)
    Xor {
        /// First input mesh file
        a: PathBuf,
        /// Second input mesh file
        b: PathBuf,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Split mesh at a plane
    Split {
        /// Input mesh file
        input: PathBuf,
        /// Plane definition as "nx,ny,nz,offset" (e.g., "0,0,1,10" for z=10)
        #[arg(long)]
        plane: String,
        /// Output files for above-plane and below-plane halves (two paths required)
        #[arg(short, long, num_args = 2)]
        output: Vec<PathBuf>,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
        /// Don't cap the cut faces
        #[arg(long)]
        no_cap: bool,
    },
    /// Hollow a mesh (create a shell)
    Hollow {
        /// Input mesh file
        input: PathBuf,
        /// Wall thickness in mm
        #[arg(long)]
        wall: f64,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Drain hole diameter in mm (default: no drain hole)
        #[arg(long)]
        drain_diameter: Option<f64>,
        /// Tapered drain hole
        #[arg(long)]
        drain_tapered: bool,
        /// Output CsgReport as JSON
        #[arg(long)]
        json: bool,
        /// Verbose output (progress, timing, triangle counts)
        #[arg(short, long)]
        verbose: bool,
    },
    /// Generate a mesh primitive
    Primitive {
        /// Primitive type
        #[arg(value_enum)]
        shape: PrimitiveShape,
        /// Dimensions (meaning depends on shape type)
        #[arg(long, num_args = 1..)]
        dims: Vec<f64>,
        /// Tessellation segments for curved surfaces
        #[arg(long, default_value = "32")]
        segments: u32,
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },
    /// Show comprehensive mesh information
    Info {
        /// Input mesh file
        input: PathBuf,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
}

/// Primitive shape types for mesh generation.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum PrimitiveShape {
    /// Box (width, height, depth)
    Box,
    /// Rounded box (width, height, depth, fillet_radius)
    RoundedBox,
    /// Cylinder (radius, height)
    Cylinder,
    /// Sphere (radius)
    Sphere,
    /// Cone (radius, height)
    Cone,
    /// Torus (major_radius, minor_radius)
    Torus,
    /// Plane (width, depth)
    Plane,
    /// Wedge (width, height, depth)
    Wedge,
    /// N-gon prism (sides, radius, height)
    NgonPrism,
}

/// Runs a CSG CLI subcommand.
///
/// # Errors
///
/// Returns an error if file I/O, mesh loading, or CSG operations fail.
#[allow(clippy::too_many_lines)]
pub fn run_csg(cmd: CsgCommand, cli_out: &crate::cli_output::CliOutput) -> Result<(), Box<dyn std::error::Error>> {
    match cmd {
        CsgCommand::Union {
            a,
            b,
            output,
            json,
            verbose,
        } => run_boolean("union", &a, &b, &output, json, verbose, mesh_union, cli_out),
        CsgCommand::Difference {
            a,
            b,
            output,
            json,
            verbose,
        } => run_boolean(
            "difference",
            &a,
            &b,
            &output,
            json,
            verbose,
            mesh_difference,
            cli_out,
        ),
        CsgCommand::Intersection {
            a,
            b,
            output,
            json,
            verbose,
        } => run_boolean(
            "intersection",
            &a,
            &b,
            &output,
            json,
            verbose,
            mesh_intersection,
            cli_out,
        ),
        CsgCommand::Xor {
            a,
            b,
            output,
            json,
            verbose,
        } => run_boolean("xor", &a, &b, &output, json, verbose, mesh_xor, cli_out),
        CsgCommand::Split {
            input,
            plane,
            output,
            json,
            verbose,
            no_cap,
        } => run_split(&input, &plane, &output, json, verbose, no_cap, cli_out),
        CsgCommand::Hollow {
            input,
            wall,
            output,
            drain_diameter,
            drain_tapered,
            json,
            verbose,
        } => run_hollow(
            &input,
            wall,
            &output,
            drain_diameter,
            drain_tapered,
            json,
            verbose,
            cli_out,
        ),
        CsgCommand::Primitive {
            shape,
            dims,
            segments,
            output,
            verbose,
        } => run_primitive(shape, &dims, segments, &output, verbose, cli_out),
        CsgCommand::Info { input, json } => crate::csg_info::run_info(&input, json),
    }
}

/// Loads a mesh from a file path, reading the bytes and auto-detecting format.
fn load_mesh_file(
    path: &std::path::Path,
) -> Result<slicecore_mesh::TriangleMesh, Box<dyn std::error::Error>> {
    let data =
        std::fs::read(path).map_err(|e| format!("failed to read '{}': {e}", path.display()))?;
    let mesh = load_mesh(&data)
        .map_err(|e| format!("failed to parse mesh from '{}': {e}", path.display()))?;
    Ok(mesh)
}

/// Function pointer type for binary boolean CSG operations.
type BooleanOpFn =
    fn(
        &slicecore_mesh::TriangleMesh,
        &slicecore_mesh::TriangleMesh,
    ) -> Result<(slicecore_mesh::TriangleMesh, CsgReport), slicecore_mesh::csg::CsgError>;

/// Runs a binary boolean operation (union, difference, intersection, xor).
fn run_boolean(
    op_name: &str,
    a_path: &std::path::Path,
    b_path: &std::path::Path,
    output_path: &std::path::Path,
    json: bool,
    verbose: bool,
    op_fn: BooleanOpFn,
    cli_out: &crate::cli_output::CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    if verbose {
        cli_out.info(&format!("Loading mesh A: {}", a_path.display()));
    }
    let mesh_a = load_mesh_file(a_path)?;

    if verbose {
        cli_out.info(&format!("Loading mesh B: {}", b_path.display()));
    }
    let mesh_b = load_mesh_file(b_path)?;

    if verbose {
        cli_out.info(&format!(
            "Running {op_name}: A ({} triangles) + B ({} triangles)",
            mesh_a.triangle_count(),
            mesh_b.triangle_count(),
        ));
    }

    let (result, report) = op_fn(&mesh_a, &mesh_b)?;

    save_mesh(&result, output_path)?;

    if verbose {
        let elapsed = start.elapsed();
        cli_out.info(&format!(
            "Done in {:.1}ms: {} output triangles -> {}",
            elapsed.as_secs_f64() * 1000.0,
            report.output_triangles,
            output_path.display(),
        ));
    }

    if json {
        let json_str = serde_json::to_string_pretty(&report)?;
        println!("{json_str}");
    }

    Ok(())
}

/// Parses a plane string "nx,ny,nz,offset" into a `SplitPlane`.
fn parse_plane(s: &str) -> Result<SplitPlane, Box<dyn std::error::Error>> {
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() != 4 {
        return Err(format!(
            "plane must be 'nx,ny,nz,offset' (got {} components)",
            parts.len()
        )
        .into());
    }
    let nx: f64 = parts[0].trim().parse()?;
    let ny: f64 = parts[1].trim().parse()?;
    let nz: f64 = parts[2].trim().parse()?;
    let offset: f64 = parts[3].trim().parse()?;
    Ok(SplitPlane::new(Vec3::new(nx, ny, nz), offset))
}

/// Runs the split subcommand.
#[allow(clippy::fn_params_excessive_bools)]
fn run_split(
    input_path: &std::path::Path,
    plane_str: &str,
    outputs: &[PathBuf],
    json: bool,
    verbose: bool,
    no_cap: bool,
    cli_out: &crate::cli_output::CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    if outputs.len() != 2 {
        return Err("split requires exactly 2 output paths (above, below)".into());
    }
    let start = Instant::now();

    if verbose {
        cli_out.info(&format!("Loading mesh: {}", input_path.display()));
    }
    let mesh = load_mesh_file(input_path)?;

    let plane = parse_plane(plane_str)?;
    let options = SplitOptions { cap: !no_cap };

    if verbose {
        cli_out.info(&format!(
            "Splitting {} triangles at plane ({},{},{}) offset={}",
            mesh.triangle_count(),
            plane_str.split(',').next().unwrap_or("?"),
            plane_str.split(',').nth(1).unwrap_or("?"),
            plane_str.split(',').nth(2).unwrap_or("?"),
            plane_str.split(',').nth(3).unwrap_or("?"),
        ));
    }

    let result = mesh_split_at_plane(&mesh, &plane, &options)?;

    save_mesh(&result.above, &outputs[0])?;
    save_mesh(&result.below, &outputs[1])?;

    if verbose {
        let elapsed = start.elapsed();
        cli_out.info(&format!(
            "Done in {:.1}ms: above={} triangles, below={} triangles",
            elapsed.as_secs_f64() * 1000.0,
            result.above.triangle_count(),
            result.below.triangle_count(),
        ));
    }

    if json {
        let json_str = serde_json::to_string_pretty(&result.report)?;
        println!("{json_str}");
    }

    Ok(())
}

/// Runs the hollow subcommand.
#[allow(clippy::too_many_arguments, clippy::fn_params_excessive_bools)]
fn run_hollow(
    input_path: &std::path::Path,
    wall: f64,
    output_path: &std::path::Path,
    drain_diameter: Option<f64>,
    drain_tapered: bool,
    json: bool,
    verbose: bool,
    cli_out: &crate::cli_output::CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    if verbose {
        cli_out.info(&format!("Loading mesh: {}", input_path.display()));
    }
    let mesh = load_mesh_file(input_path)?;

    let drain_hole = drain_diameter.map(|d| {
        // Place drain at bottom-center of mesh bounding box.
        let aabb = mesh.aabb();
        let center_x = (aabb.min.x + aabb.max.x) / 2.0;
        let center_y = (aabb.min.y + aabb.max.y) / 2.0;
        DrainHole {
            position: Point3::new(center_x, center_y, aabb.min.z),
            direction: Vec3::new(0.0, 0.0, -1.0),
            diameter: d,
            tapered: drain_tapered,
        }
    });

    let options = HollowOptions {
        wall_thickness: wall,
        drain_hole,
    };

    if verbose {
        cli_out.info(&format!(
            "Hollowing {} triangles, wall={wall}mm",
            mesh.triangle_count(),
        ));
    }

    let (result, report) = hollow_mesh(&mesh, &options)?;

    save_mesh(&result, output_path)?;

    if verbose {
        let elapsed = start.elapsed();
        cli_out.info(&format!(
            "Done in {:.1}ms: {} output triangles -> {}",
            elapsed.as_secs_f64() * 1000.0,
            report.output_triangles,
            output_path.display(),
        ));
    }

    if json {
        let json_str = serde_json::to_string_pretty(&report)?;
        println!("{json_str}");
    }

    Ok(())
}

/// Runs the primitive subcommand.
fn run_primitive(
    shape: PrimitiveShape,
    dims: &[f64],
    segments: u32,
    output_path: &std::path::Path,
    verbose: bool,
    cli_out: &crate::cli_output::CliOutput,
) -> Result<(), Box<dyn std::error::Error>> {
    let start = Instant::now();

    let mesh = match shape {
        PrimitiveShape::Box => {
            let (w, h, d) = get_dims_3(dims, "box (width, height, depth)")?;
            primitive_box(w, h, d)
        }
        PrimitiveShape::RoundedBox => {
            let vals = get_dims_n(dims, 4, "rounded-box (width, height, depth, fillet_radius)")?;
            primitive_rounded_box(vals[0], vals[1], vals[2], vals[3], segments)
        }
        PrimitiveShape::Cylinder => {
            let (r, h) = get_dims_2(dims, "cylinder (radius, height)")?;
            primitive_cylinder(r, h, segments)
        }
        PrimitiveShape::Sphere => {
            let r = get_dims_1(dims, "sphere (radius)")?;
            primitive_sphere(r, segments)
        }
        PrimitiveShape::Cone => {
            let (r, h) = get_dims_2(dims, "cone (radius, height)")?;
            primitive_cone(r, h, segments)
        }
        PrimitiveShape::Torus => {
            let (major, minor) = get_dims_2(dims, "torus (major_radius, minor_radius)")?;
            primitive_torus(major, minor, segments, segments)
        }
        PrimitiveShape::Plane => {
            let (w, d) = get_dims_2(dims, "plane (width, depth)")?;
            primitive_plane(w, d)
        }
        PrimitiveShape::Wedge => {
            let (w, h, d) = get_dims_3(dims, "wedge (width, height, depth)")?;
            primitive_wedge(w, h, d)
        }
        PrimitiveShape::NgonPrism => {
            let (sides, r, h) = get_dims_3(dims, "ngon-prism (sides, radius, height)")?;
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let sides_u32 = sides as u32;
            primitive_ngon_prism(sides_u32, r, h)
        }
    };

    save_mesh(&mesh, output_path)?;

    if verbose {
        let elapsed = start.elapsed();
        cli_out.info(&format!(
            "Generated {shape:?} with {} triangles in {:.1}ms -> {}",
            mesh.triangle_count(),
            elapsed.as_secs_f64() * 1000.0,
            output_path.display(),
        ));
    }

    Ok(())
}

/// Extracts exactly 1 dimension from a slice.
fn get_dims_1(dims: &[f64], usage: &str) -> Result<f64, Box<dyn std::error::Error>> {
    if dims.is_empty() {
        return Err(format!("expected 1 dimension for {usage}").into());
    }
    Ok(dims[0])
}

/// Extracts exactly 2 dimensions from a slice.
fn get_dims_2(dims: &[f64], usage: &str) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    if dims.len() < 2 {
        return Err(format!("expected 2 dimensions for {usage}").into());
    }
    Ok((dims[0], dims[1]))
}

/// Extracts exactly 3 dimensions from a slice.
fn get_dims_3(dims: &[f64], usage: &str) -> Result<(f64, f64, f64), Box<dyn std::error::Error>> {
    if dims.len() < 3 {
        return Err(format!("expected 3 dimensions for {usage}").into());
    }
    Ok((dims[0], dims[1], dims[2]))
}

/// Extracts exactly N dimensions from a slice.
fn get_dims_n(dims: &[f64], n: usize, usage: &str) -> Result<Vec<f64>, Box<dyn std::error::Error>> {
    if dims.len() < n {
        return Err(format!("expected {n} dimensions for {usage}").into());
    }
    Ok(dims[..n].to_vec())
}
