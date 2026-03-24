//! Job directory management for isolated slice execution.
//!
//! Provides `JobDir` for creating and managing structured output directories,
//! `Manifest` for job metadata serialization, and PID-based file locking.

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
