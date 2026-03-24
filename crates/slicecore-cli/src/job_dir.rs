//! Job directory management for isolated slice execution.
//!
//! Provides [`JobDir`] for creating and managing structured output directories,
//! [`Manifest`] for job metadata serialization, and PID-based file locking.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Errors that can occur during job directory operations.
#[derive(Debug, thiserror::Error)]
pub enum JobDirError {
    /// The target directory is not empty and `force` was not set.
    #[error("job directory '{}' is not empty (use --force to override)", .0.display())]
    NotEmpty(PathBuf),

    /// The target directory is locked by another process.
    #[error("job directory '{}' is locked by PID {pid}", .path.display())]
    Locked {
        /// Path to the locked directory.
        path: PathBuf,
        /// PID of the process holding the lock.
        pid: u32,
    },

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A JSON serialization error occurred.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

// ---------------------------------------------------------------------------
// JobDir
// ---------------------------------------------------------------------------

/// A managed job output directory with PID-based locking.
///
/// On creation the directory is created (with `mkdir -p` semantics), a `.lock`
/// file is written with the current PID, and (optionally) emptiness is verified.
/// The lock file is automatically removed when the `JobDir` is dropped.
pub struct JobDir {
    path: PathBuf,
    // Held open to keep the file descriptor alive; not read after creation.
    _lock_file: std::fs::File,
}

impl JobDir {
    /// Create a job directory at `path`.
    ///
    /// The directory and all parent directories are created if they do not
    /// exist. A `.lock` file containing the current PID is written. If
    /// `force` is `false`, the function returns [`JobDirError::NotEmpty`]
    /// when the directory contains files other than `.lock`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created, is already locked
    /// by a live process, or is non-empty without `force`.
    pub fn create(path: PathBuf, force: bool) -> Result<Self, JobDirError> {
        std::fs::create_dir_all(&path)?;

        // Lock first, then check emptiness (avoids TOCTOU race).
        let lock_file = acquire_lock(&path)?;

        if !force {
            let has_non_lock = std::fs::read_dir(&path)?.any(|entry| {
                entry
                    .ok()
                    .and_then(|e| e.file_name().into_string().ok())
                    .is_some_and(|name| name != ".lock")
            });
            if has_non_lock {
                // Release the lock before returning the error.
                release_lock(&path);
                return Err(JobDirError::NotEmpty(path));
            }
        }

        Ok(Self {
            path,
            _lock_file: lock_file,
        })
    }

    /// Create a job directory with an auto-generated UUID v4 name under
    /// `base_dir`.
    ///
    /// # Errors
    ///
    /// Delegates to [`JobDir::create`]; see its documentation.
    pub fn create_auto(base_dir: PathBuf, force: bool) -> Result<Self, JobDirError> {
        let dir_name = Uuid::new_v4().to_string();
        Self::create(base_dir.join(dir_name), force)
    }

    /// Resolve the base directory for auto-generated job directories.
    ///
    /// Priority: `job_base` argument > `SLICECORE_JOB_DIR` env var > CWD.
    pub fn resolve_base(job_base: Option<&Path>) -> PathBuf {
        if let Some(base) = job_base {
            return base.to_path_buf();
        }
        if let Ok(env_dir) = std::env::var("SLICECORE_JOB_DIR") {
            return PathBuf::from(env_dir);
        }
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    }

    /// Returns the root path of this job directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the G-code output path for a given model filename.
    ///
    /// The model extension is stripped and replaced with `.gcode`.
    /// For example, `"benchy.stl"` becomes `"{dir}/benchy.gcode"`.
    pub fn gcode_path(&self, model_name: &str) -> PathBuf {
        let stem = Path::new(model_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(model_name);
        self.path.join(format!("{stem}.gcode"))
    }

    /// Returns the G-code output path for plate mode.
    pub fn plate_gcode_path(&self) -> PathBuf {
        self.path.join("plate.gcode")
    }

    /// Returns the manifest file path.
    pub fn manifest_path(&self) -> PathBuf {
        self.path.join("manifest.json")
    }

    /// Returns the config snapshot file path.
    pub fn config_path(&self) -> PathBuf {
        self.path.join("config.toml")
    }

    /// Returns the slice log file path.
    pub fn log_path(&self) -> PathBuf {
        self.path.join("slice.log")
    }

    /// Returns the thumbnail file path.
    pub fn thumbnail_path(&self) -> PathBuf {
        self.path.join("thumbnail.png")
    }

    /// Write a manifest to `manifest.json` in this job directory.
    ///
    /// # Errors
    ///
    /// Returns an error if JSON serialization or file I/O fails.
    pub fn write_manifest(&self, manifest: &Manifest) -> Result<(), JobDirError> {
        let json = serde_json::to_string_pretty(manifest)?;
        std::fs::write(self.manifest_path(), json)?;
        Ok(())
    }

    /// Compute the SHA-256 checksum of a file.
    ///
    /// Returns a string in the format `"sha256:{hex_digest}"`.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read.
    pub fn file_checksum(path: &Path) -> std::io::Result<String> {
        let data = std::fs::read(path)?;
        let hash = Sha256::digest(&data);
        Ok(format!("sha256:{hash:x}"))
    }
}

impl Drop for JobDir {
    fn drop(&mut self) {
        release_lock(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Locking helpers
// ---------------------------------------------------------------------------

/// Acquire a PID-based lock on a directory.
///
/// If a `.lock` file exists and the recorded PID is still alive, returns
/// `JobDirError::Locked`. Stale locks (dead PIDs) are removed automatically.
fn acquire_lock(dir: &Path) -> Result<std::fs::File, JobDirError> {
    let lock_path = dir.join(".lock");
    if lock_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&lock_path) {
            if let Ok(pid) = content.trim().parse::<u32>() {
                if process_exists(pid) {
                    return Err(JobDirError::Locked {
                        path: dir.to_path_buf(),
                        pid,
                    });
                }
                // Stale lock -- remove it.
                let _ = std::fs::remove_file(&lock_path);
            }
        }
    }
    let mut file = std::fs::File::create(&lock_path)?;
    write!(file, "{}", std::process::id())?;
    file.sync_all()?;
    Ok(file)
}

/// Remove the `.lock` file from a directory, ignoring errors.
fn release_lock(dir: &Path) {
    let _ = std::fs::remove_file(dir.join(".lock"));
}

/// Check whether a process with the given PID is still running.
#[cfg(unix)]
fn process_exists(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// On non-unix platforms, conservatively assume the process is alive.
#[cfg(not(unix))]
fn process_exists(_pid: u32) -> bool {
    true
}

// ---------------------------------------------------------------------------
// Manifest and supporting types
// ---------------------------------------------------------------------------

/// Status of a slicing job.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// The job is currently running.
    Running,
    /// The job completed successfully.
    Success,
    /// The job failed.
    #[allow(dead_code)] // Used by external tooling and future failure-path wiring
    Failed,
}

/// Job manifest written to `manifest.json` in the job directory.
///
/// Created initially with [`Manifest::new_running`] and transitioned to a
/// terminal state via [`Manifest::into_success`] or [`Manifest::into_failed`].
#[derive(Debug, Clone, Serialize)]
pub struct Manifest {
    /// Schema version (always 1).
    pub schema_version: u32,
    /// Version of slicecore that produced this job.
    pub slicecore_version: String,
    /// Current job status.
    pub status: JobStatus,
    /// ISO 8601 timestamp when the manifest was created.
    pub created: String,
    /// ISO 8601 timestamp when slicing started.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started: Option<String>,
    /// ISO 8601 timestamp when slicing completed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed: Option<String>,
    /// Duration of slicing in milliseconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Input model file paths.
    pub input: Vec<PathBuf>,
    /// Output G-code filename (relative to job directory).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Config snapshot filename.
    pub config: String,
    /// Log filename.
    pub log: String,
    /// Thumbnail filename, if generated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    /// Full CLI command to reproduce this slice.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reproduce_command: Option<String>,
    /// SHA-256 checksums of output artifacts.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksums: Option<ArtifactChecksums>,
    /// Print statistics from the slicing result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub statistics: Option<PrintStats>,
    /// Profile provenance information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile_provenance: Option<Vec<ProfileSource>>,
    /// Metadata about input models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_metadata: Option<Vec<InputModelMeta>>,
    /// Non-fatal warnings emitted during slicing.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    /// Build environment information.
    pub environment: EnvironmentInfo,
    /// Config diff from default profile.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_diff: Option<serde_json::Value>,
    /// Error message (only when status is `failed`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Per-object statistics in plate mode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub per_object_stats: Option<Vec<serde_json::Value>>,
}

impl Manifest {
    /// Create a new manifest in the `Running` state.
    pub fn new_running(input: Vec<PathBuf>) -> Self {
        let now = Utc::now().to_rfc3339();
        Self {
            schema_version: 1,
            slicecore_version: env!("CARGO_PKG_VERSION").to_string(),
            status: JobStatus::Running,
            created: now.clone(),
            started: Some(now),
            completed: None,
            duration_ms: None,
            input,
            output: None,
            config: "config.toml".to_string(),
            log: "slice.log".to_string(),
            thumbnail: None,
            reproduce_command: None,
            checksums: None,
            statistics: None,
            profile_provenance: None,
            input_metadata: None,
            warnings: Vec::new(),
            environment: EnvironmentInfo::current(),
            config_diff: None,
            error: None,
            per_object_stats: None,
        }
    }

    /// Transition this manifest to the `Success` state.
    #[must_use]
    pub fn into_success(
        mut self,
        output: String,
        checksums: Option<ArtifactChecksums>,
        statistics: Option<PrintStats>,
        duration_ms: u64,
    ) -> Self {
        self.status = JobStatus::Success;
        self.completed = Some(Utc::now().to_rfc3339());
        self.duration_ms = Some(duration_ms);
        self.output = Some(output);
        self.checksums = checksums;
        self.statistics = statistics;
        self
    }

    /// Transition this manifest to the `Failed` state.
    #[must_use]
    #[allow(dead_code)] // Part of the manifest lifecycle API; wired in future failure-path work
    pub fn into_failed(mut self, error: String) -> Self {
        self.status = JobStatus::Failed;
        self.completed = Some(Utc::now().to_rfc3339());
        self.error = Some(error);
        self
    }
}

// ---------------------------------------------------------------------------
// Supporting structs
// ---------------------------------------------------------------------------

/// SHA-256 checksums of output artifacts.
#[derive(Debug, Clone, Serialize)]
pub struct ArtifactChecksums {
    /// Checksum of the G-code file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcode: Option<String>,
    /// Checksum of the config snapshot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// Checksum of the thumbnail.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

/// Print statistics from the slicing result.
#[derive(Debug, Clone, Serialize)]
pub struct PrintStats {
    /// Total filament length in millimeters.
    pub filament_length_mm: f64,
    /// Total filament weight in grams.
    pub filament_weight_g: f64,
    /// Estimated filament cost.
    pub filament_cost: f64,
    /// Estimated print time in seconds.
    pub estimated_time_seconds: f64,
    /// Number of layers.
    pub layer_count: usize,
    /// Number of G-code lines.
    pub line_count: usize,
}

/// Source information for a print profile.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileSource {
    /// Role of the profile (e.g., "machine", "filament", "process").
    pub role: String,
    /// Name of the profile.
    pub name: String,
    /// Path to the profile file, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    /// Checksum of the profile file, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
}

/// Metadata about an input model.
#[derive(Debug, Clone, Serialize)]
pub struct InputModelMeta {
    /// Path to the model file.
    pub path: PathBuf,
    /// Number of triangles in the mesh.
    pub triangle_count: usize,
    /// Axis-aligned bounding box: `[min_x, min_y, min_z, max_x, max_y, max_z]`.
    pub bounding_box: [f64; 6],
    /// File size in bytes.
    pub file_size_bytes: u64,
}

/// Build environment information.
#[derive(Debug, Clone, Serialize)]
pub struct EnvironmentInfo {
    /// Operating system.
    pub os: String,
    /// CPU architecture.
    pub arch: String,
    /// Hostname.
    pub hostname: String,
}

impl EnvironmentInfo {
    /// Collect current environment information.
    fn current() -> Self {
        let hostname = std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        Self {
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
            hostname,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn create_makes_directory_and_lock_file() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("my-job");
        let job = JobDir::create(job_path.clone(), false).unwrap();
        assert!(job_path.exists());
        assert!(job_path.join(".lock").exists());
        drop(job);
    }

    #[test]
    fn create_errors_on_non_empty_directory_without_force() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("non-empty");
        std::fs::create_dir_all(&job_path).unwrap();
        std::fs::write(job_path.join("existing.txt"), "data").unwrap();
        let result = JobDir::create(job_path, false);
        assert!(matches!(result, Err(JobDirError::NotEmpty(_))));
    }

    #[test]
    fn create_succeeds_on_non_empty_directory_with_force() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("non-empty");
        std::fs::create_dir_all(&job_path).unwrap();
        std::fs::write(job_path.join("existing.txt"), "data").unwrap();
        let result = JobDir::create(job_path, true);
        assert!(result.is_ok());
    }

    #[test]
    fn create_auto_generates_uuid_directory() {
        let tmp = TempDir::new().unwrap();
        let job = JobDir::create_auto(tmp.path().to_path_buf(), false).unwrap();
        let dir_name = job.path().file_name().unwrap().to_str().unwrap();
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(dir_name.len(), 36);
        assert_eq!(dir_name.chars().filter(|c| *c == '-').count(), 4);
    }

    #[test]
    fn resolve_base_returns_job_base_when_provided() {
        let path = PathBuf::from("/custom/base");
        let result = JobDir::resolve_base(Some(path.as_path()));
        assert_eq!(result, path);
    }

    #[test]
    fn resolve_base_returns_env_var_when_no_flag() {
        // Save and restore env var
        let prev = std::env::var("SLICECORE_JOB_DIR").ok();
        std::env::set_var("SLICECORE_JOB_DIR", "/env/base");
        let result = JobDir::resolve_base(None);
        assert_eq!(result, PathBuf::from("/env/base"));
        match prev {
            Some(v) => std::env::set_var("SLICECORE_JOB_DIR", v),
            None => std::env::remove_var("SLICECORE_JOB_DIR"),
        }
    }

    #[test]
    fn resolve_base_returns_cwd_when_nothing_set() {
        let prev = std::env::var("SLICECORE_JOB_DIR").ok();
        std::env::remove_var("SLICECORE_JOB_DIR");
        let result = JobDir::resolve_base(None);
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        assert_eq!(result, cwd);
        if let Some(v) = prev {
            std::env::set_var("SLICECORE_JOB_DIR", v);
        }
    }

    #[test]
    fn artifact_paths_are_correct() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("artifacts-test");
        let job = JobDir::create(job_path.clone(), false).unwrap();
        assert_eq!(job.manifest_path(), job_path.join("manifest.json"));
        assert_eq!(job.config_path(), job_path.join("config.toml"));
        assert_eq!(job.log_path(), job_path.join("slice.log"));
        assert_eq!(job.thumbnail_path(), job_path.join("thumbnail.png"));
    }

    #[test]
    fn gcode_path_strips_extension() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("gcode-test");
        let job = JobDir::create(job_path.clone(), false).unwrap();
        assert_eq!(job.gcode_path("benchy.stl"), job_path.join("benchy.gcode"));
    }

    #[test]
    fn plate_gcode_path_returns_plate_gcode() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("plate-test");
        let job = JobDir::create(job_path.clone(), false).unwrap();
        assert_eq!(job.plate_gcode_path(), job_path.join("plate.gcode"));
    }

    #[test]
    fn manifest_serializes_with_schema_version_1() {
        let manifest = Manifest::new_running(vec![PathBuf::from("test.stl")]);
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["schema_version"], 1);
    }

    #[test]
    fn manifest_running_has_no_stats() {
        let manifest = Manifest::new_running(vec![PathBuf::from("test.stl")]);
        let json = serde_json::to_string(&manifest).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["status"], "running");
        assert!(value.get("checksums").is_none());
        assert!(value.get("statistics").is_none());
    }

    #[test]
    fn manifest_success_includes_stats_and_checksums() {
        let manifest = Manifest::new_running(vec![PathBuf::from("test.stl")]);
        let stats = PrintStats {
            filament_length_mm: 1000.0,
            filament_weight_g: 5.0,
            filament_cost: 0.50,
            estimated_time_seconds: 3600.0,
            layer_count: 100,
            line_count: 50000,
        };
        let checksums = ArtifactChecksums {
            gcode: Some("sha256:abc123".to_string()),
            config: Some("sha256:def456".to_string()),
            thumbnail: None,
        };
        let manifest = manifest.into_success(
            "benchy.gcode".to_string(),
            Some(checksums),
            Some(stats),
            100,
        );
        let json = serde_json::to_string(&manifest).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["status"], "success");
        assert!(value["checksums"].is_object());
        assert!(value["statistics"].is_object());
    }

    #[test]
    fn manifest_failed_includes_error() {
        let manifest = Manifest::new_running(vec![PathBuf::from("test.stl")]);
        let manifest = manifest.into_failed("Mesh has zero triangles".to_string());
        let json = serde_json::to_string(&manifest).unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["status"], "failed");
        assert_eq!(value["error"], "Mesh has zero triangles");
    }

    #[test]
    fn lock_file_contains_current_pid() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("lock-pid-test");
        let _job = JobDir::create(job_path.clone(), false).unwrap();
        let lock_content = std::fs::read_to_string(job_path.join(".lock")).unwrap();
        let pid: u32 = lock_content.trim().parse().unwrap();
        assert_eq!(pid, std::process::id());
    }

    #[test]
    fn dropping_job_dir_removes_lock_file() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("drop-test");
        let job = JobDir::create(job_path.clone(), false).unwrap();
        assert!(job_path.join(".lock").exists());
        drop(job);
        assert!(!job_path.join(".lock").exists());
    }

    #[test]
    fn second_create_on_locked_directory_returns_locked_error() {
        let tmp = TempDir::new().unwrap();
        let job_path = tmp.path().join("locked-test");
        let _job1 = JobDir::create(job_path.clone(), false).unwrap();
        let result = JobDir::create(job_path, true); // force=true but still locked
        assert!(matches!(result, Err(JobDirError::Locked { .. })));
    }
}
