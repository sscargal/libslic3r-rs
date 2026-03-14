//! Profile name resolution engine.
//!
//! Translates user-provided profile names (e.g., `"PLA_Basic"`, `"Bambu_X1C"`)
//! into concrete TOML file paths by searching user profiles first, then the
//! library index, then filesystem scan.
//!
//! # Resolution Order
//!
//! 1. If the query contains `/` or ends with `.toml`, treat it as a file path.
//! 2. Search user profiles directory (`~/.slicecore/profiles/{type}/`).
//! 3. Search library index for exact ID match, then case-insensitive substring.
//! 4. If single match: return. If multiple: [`ProfileError::Ambiguous`]. If none:
//!    [`ProfileError::NotFound`] with "did you mean?" suggestions.
//!
//! # Examples
//!
//! ```no_run
//! use slicecore_engine::profile_resolve::ProfileResolver;
//!
//! let resolver = ProfileResolver::new(None);
//! let resolved = resolver.resolve("PLA_Basic", "filament").unwrap();
//! println!("Resolved to: {}", resolved.path.display());
//! ```

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::EngineError;
use crate::profile_library::{load_index, ProfileIndex, ProfileIndexEntry};

/// Where a resolved profile was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileSource {
    /// Found in the user profiles directory (`~/.slicecore/profiles/`).
    User,
    /// Found in the library index under a specific vendor.
    Library {
        /// Vendor name, e.g. `"BBL"`, `"Creality"`.
        vendor: String,
    },
    /// Built-in default profile.
    BuiltIn,
}

impl std::fmt::Display for ProfileSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::User => write!(f, "user"),
            Self::Library { vendor } => write!(f, "library/{vendor}"),
            Self::BuiltIn => write!(f, "built-in"),
        }
    }
}

/// A successfully resolved profile with metadata.
#[derive(Debug, Clone)]
pub struct ResolvedProfile {
    /// Absolute path to the TOML profile file.
    pub path: PathBuf,
    /// Where the profile was found.
    pub source: ProfileSource,
    /// Profile type: `"machine"`, `"filament"`, or `"process"`.
    pub profile_type: String,
    /// Human-readable profile name.
    pub name: String,
    /// SHA-256 hex digest of the file content.
    pub checksum: String,
}

/// Errors that can occur during profile resolution.
#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    /// No profile matched the query.
    #[error("profile not found: '{query}'{}", format_suggestions(.suggestions))]
    NotFound {
        /// The user-provided query string.
        query: String,
        /// Similar profile names for "did you mean?" hints.
        suggestions: Vec<String>,
    },

    /// Multiple profiles matched the query.
    #[error("ambiguous profile query '{query}', matches: {}", matches.join(", "))]
    Ambiguous {
        /// The user-provided query string.
        query: String,
        /// All matching profile names.
        matches: Vec<String>,
    },

    /// Profile found but wrong type.
    #[error("profile '{query}' is a {actual_type} profile, not {expected_type}{}", format_hint(.hint))]
    TypeMismatch {
        /// The user-provided query string.
        query: String,
        /// The type the user requested.
        expected_type: String,
        /// The actual type of the matched profile.
        actual_type: String,
        /// Hint like "did you mean --filament?".
        hint: Option<String>,
    },

    /// Circular inheritance detected.
    #[error("circular inheritance detected: {}", chain.join(" -> "))]
    CircularInheritance {
        /// The chain of profile names forming the cycle.
        chain: Vec<String>,
    },

    /// I/O error reading a profile file.
    #[error("IO error reading profile: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse a profile TOML file.
    #[error("failed to parse profile TOML: {0}")]
    Parse(String),
}

fn format_suggestions(suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        String::new()
    } else {
        format!(". Did you mean: {}?", suggestions.join(", "))
    }
}

fn format_hint(hint: &Option<String>) -> String {
    match hint {
        Some(h) => format!(" ({h})"),
        None => String::new(),
    }
}

impl From<ProfileError> for EngineError {
    fn from(e: ProfileError) -> Self {
        EngineError::ConfigError(e.to_string())
    }
}

/// Profile name resolver.
///
/// Searches user profiles directory, then library directories for matching
/// profiles by name, with type-constrained filtering.
#[derive(Debug)]
pub struct ProfileResolver {
    /// User profiles directory (e.g., `~/.slicecore/profiles/`).
    user_dir: Option<PathBuf>,
    /// Library directories containing converted profile trees.
    library_dirs: Vec<PathBuf>,
    /// Loaded library index (if found).
    index: Option<ProfileIndex>,
}

impl ProfileResolver {
    /// Creates a new resolver with optional profiles directory override.
    ///
    /// # Discovery Order for Library Directories
    ///
    /// 1. `$SLICECORE_PROFILES_DIR` environment variable
    /// 2. `profiles_dir_override` parameter (from CLI `--profiles-dir`)
    /// 3. `./profiles/` relative to working directory
    /// 4. `<binary-dir>/profiles/` relative to the executable
    /// 5. `~/.slicecore/library/`
    ///
    /// User profiles are always at `~/.slicecore/profiles/`.
    ///
    /// # Examples
    ///
    /// ```
    /// use slicecore_engine::profile_resolve::ProfileResolver;
    ///
    /// let resolver = ProfileResolver::new(None);
    /// ```
    #[must_use]
    pub fn new(profiles_dir_override: Option<&Path>) -> Self {
        let library_dirs = Self::find_library_dirs(profiles_dir_override);
        let user_dir = Self::find_user_dir();

        // Try to load index from the first library dir that has one
        let index = library_dirs.iter().find_map(|dir| load_index(dir).ok());

        Self {
            user_dir,
            library_dirs,
            index,
        }
    }

    /// Creates a resolver with explicit directories (for testing).
    #[must_use]
    pub fn with_dirs(
        user_dir: Option<PathBuf>,
        library_dirs: Vec<PathBuf>,
        index: Option<ProfileIndex>,
    ) -> Self {
        Self {
            user_dir,
            library_dirs,
            index,
        }
    }

    /// Resolves a profile query to a concrete file path.
    ///
    /// # Resolution Algorithm
    ///
    /// 1. If `query` contains `/` or ends with `.toml`, treat as file path.
    /// 2. Search user profiles directory for exact filename match, then substring.
    /// 3. Search library index for exact ID match, then case-insensitive substring.
    /// 4. Single match -> return. Multiple -> [`ProfileError::Ambiguous`].
    ///    None -> [`ProfileError::NotFound`] with suggestions.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError`] on not-found, ambiguous, type mismatch, or I/O errors.
    pub fn resolve(
        &self,
        query: &str,
        expected_type: &str,
    ) -> Result<ResolvedProfile, ProfileError> {
        // Step 1: File path shortcut
        if query.contains('/') || query.ends_with(".toml") {
            return self.resolve_file_path(query, expected_type);
        }

        // Step 2: Search user profiles (user shadows library for same name)
        let mut matches = Vec::new();
        if let Some(ref user_dir) = self.user_dir {
            let type_dir = user_dir.join(expected_type);
            if type_dir.is_dir() {
                self.search_directory(&type_dir, query, expected_type, ProfileSource::User, &mut matches);
            }
        }

        // If user search found an exact match, return it immediately (shadowing)
        let query_lower = query.to_lowercase();
        let user_exact: Vec<_> = matches
            .iter()
            .filter(|m| {
                m.name.to_lowercase() == query_lower
                    || m.path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .is_some_and(|s| s.to_lowercase() == query_lower)
            })
            .cloned()
            .collect();
        if user_exact.len() == 1 {
            return Ok(user_exact.into_iter().next().expect("checked len == 1"));
        }

        // Step 3: Search library index
        if let Some(ref index) = self.index {
            self.search_index(index, query, expected_type, &mut matches);
        }

        // Also search library dirs for TOML files not in index
        for lib_dir in &self.library_dirs {
            let type_dir = lib_dir.join(expected_type);
            if type_dir.is_dir() {
                // Only add matches not already found
                let existing_paths: HashSet<PathBuf> =
                    matches.iter().map(|m: &ResolvedProfile| m.path.clone()).collect();
                let mut dir_matches = Vec::new();
                self.search_directory(
                    &type_dir,
                    query,
                    expected_type,
                    ProfileSource::Library {
                        vendor: String::new(),
                    },
                    &mut dir_matches,
                );
                for m in dir_matches {
                    if !existing_paths.contains(&m.path) {
                        matches.push(m);
                    }
                }
            }
        }

        // Step 4: Evaluate results
        match matches.len() {
            0 => {
                // Check for type mismatch (found in different type)
                if let Some(mismatch) = self.find_type_mismatch(query, expected_type) {
                    return Err(mismatch);
                }
                let suggestions = self.suggest_similar(query, expected_type);
                Err(ProfileError::NotFound {
                    query: query.to_string(),
                    suggestions,
                })
            }
            1 => Ok(matches.into_iter().next().expect("checked len == 1")),
            _ => {
                // Check for exact match among multiple substring matches
                let exact: Vec<_> = matches
                    .iter()
                    .filter(|m| {
                        m.name.to_lowercase() == query_lower
                            || m.path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .is_some_and(|s| s.to_lowercase() == query_lower)
                    })
                    .cloned()
                    .collect();
                if exact.len() == 1 {
                    return Ok(exact.into_iter().next().expect("checked len == 1"));
                }
                Err(ProfileError::Ambiguous {
                    query: query.to_string(),
                    matches: matches.iter().map(|m| m.name.clone()).collect(),
                })
            }
        }
    }

    /// Resolves inheritance chain for a profile.
    ///
    /// Reads the `inherits` field from the TOML and recursively resolves
    /// parent profiles. Returns the chain `[root_ancestor, ..., profile]`.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::CircularInheritance`] if a cycle is detected.
    /// Depth is limited to 5 levels.
    pub fn resolve_inheritance(
        &self,
        profile_path: &Path,
    ) -> Result<Vec<ResolvedProfile>, ProfileError> {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        self.resolve_inheritance_inner(profile_path, &mut chain, &mut visited, 0)?;
        chain.reverse(); // root ancestor first
        Ok(chain)
    }

    /// Searches for profiles matching a query, returning all matches.
    ///
    /// Useful for list/search commands. Optionally filters by type.
    #[must_use]
    pub fn search(
        &self,
        query: &str,
        type_filter: Option<&str>,
        limit: usize,
    ) -> Vec<ResolvedProfile> {
        let types = if let Some(t) = type_filter {
            vec![t.to_string()]
        } else {
            vec![
                "machine".to_string(),
                "filament".to_string(),
                "process".to_string(),
            ]
        };

        let mut results = Vec::new();
        for profile_type in &types {
            if let Some(ref user_dir) = self.user_dir {
                let type_dir = user_dir.join(profile_type);
                if type_dir.is_dir() {
                    self.search_directory(
                        &type_dir,
                        query,
                        profile_type,
                        ProfileSource::User,
                        &mut results,
                    );
                }
            }
            if let Some(ref index) = self.index {
                self.search_index(index, query, profile_type, &mut results);
            }
            for lib_dir in &self.library_dirs {
                let type_dir = lib_dir.join(profile_type);
                if type_dir.is_dir() {
                    let existing: HashSet<PathBuf> =
                        results.iter().map(|m| m.path.clone()).collect();
                    let mut dir_matches = Vec::new();
                    self.search_directory(
                        &type_dir,
                        query,
                        profile_type,
                        ProfileSource::Library {
                            vendor: String::new(),
                        },
                        &mut dir_matches,
                    );
                    for m in dir_matches {
                        if !existing.contains(&m.path) {
                            results.push(m);
                        }
                    }
                }
            }
        }
        results.truncate(limit);
        results
    }

    /// Discovers library directories in priority order.
    ///
    /// 1. `$SLICECORE_PROFILES_DIR` environment variable
    /// 2. `profiles_dir_override` (from CLI)
    /// 3. `./profiles/`
    /// 4. `<binary-dir>/profiles/`
    /// 5. `~/.slicecore/library/`
    #[must_use]
    pub fn find_library_dirs(profiles_dir_override: Option<&Path>) -> Vec<PathBuf> {
        let mut dirs = Vec::new();

        // 1. Environment variable
        if let Ok(env_dir) = std::env::var("SLICECORE_PROFILES_DIR") {
            let p = PathBuf::from(&env_dir);
            if p.is_dir() {
                dirs.push(p);
            }
        }

        // 2. CLI override
        if let Some(override_dir) = profiles_dir_override {
            if override_dir.is_dir() && !dirs.contains(&override_dir.to_path_buf()) {
                dirs.push(override_dir.to_path_buf());
            }
        }

        // 3. ./profiles/
        let cwd_profiles = PathBuf::from("profiles");
        if cwd_profiles.is_dir() && !dirs.contains(&cwd_profiles) {
            dirs.push(cwd_profiles);
        }

        // 4. <binary-dir>/profiles/
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                let bin_profiles = exe_dir.join("profiles");
                if bin_profiles.is_dir() && !dirs.contains(&bin_profiles) {
                    dirs.push(bin_profiles);
                }
            }
        }

        // 5. ~/.slicecore/library/
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Some(home) = home_dir() {
                let lib_dir = home.join(".slicecore").join("library");
                if lib_dir.is_dir() && !dirs.contains(&lib_dir) {
                    dirs.push(lib_dir);
                }
            }
        }

        dirs
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn find_user_dir() -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            home_dir().map(|h| h.join(".slicecore").join("profiles"))
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    fn resolve_file_path(
        &self,
        path_str: &str,
        expected_type: &str,
    ) -> Result<ResolvedProfile, ProfileError> {
        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(ProfileError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("profile file not found: {path_str}"),
            )));
        }
        let content = std::fs::read_to_string(&path)?;
        let profile_type = extract_profile_type(&content).unwrap_or_else(|| expected_type.to_string());

        if profile_type != expected_type {
            return Err(ProfileError::TypeMismatch {
                query: path_str.to_string(),
                expected_type: expected_type.to_string(),
                actual_type: profile_type,
                hint: Some(format!("did you mean --{expected_type}?")),
            });
        }

        let checksum = compute_checksum(&content);
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        Ok(ResolvedProfile {
            path,
            source: ProfileSource::User,
            profile_type,
            name,
            checksum,
        })
    }

    fn search_directory(
        &self,
        dir: &Path,
        query: &str,
        profile_type: &str,
        source: ProfileSource,
        matches: &mut Vec<ResolvedProfile>,
    ) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        let query_lower = query.to_lowercase();

        // Collect TOML files
        let mut toml_files: Vec<(String, PathBuf)> = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    toml_files.push((stem.to_string(), path));
                }
            }
        }

        // Check exact match first
        for (stem, path) in &toml_files {
            if stem.to_lowercase() == query_lower {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let checksum = compute_checksum(&content);
                    matches.push(ResolvedProfile {
                        path: path.clone(),
                        source: source.clone(),
                        profile_type: profile_type.to_string(),
                        name: stem.clone(),
                        checksum,
                    });
                    return; // exact match found, skip substring
                }
            }
        }

        // Substring match
        for (stem, path) in &toml_files {
            if stem.to_lowercase().contains(&query_lower) {
                if let Ok(content) = std::fs::read_to_string(path) {
                    let checksum = compute_checksum(&content);
                    matches.push(ResolvedProfile {
                        path: path.clone(),
                        source: source.clone(),
                        profile_type: profile_type.to_string(),
                        name: stem.clone(),
                        checksum,
                    });
                }
            }
        }
    }

    fn search_index(
        &self,
        index: &ProfileIndex,
        query: &str,
        expected_type: &str,
        matches: &mut Vec<ResolvedProfile>,
    ) {
        let query_lower = query.to_lowercase();

        // Exact ID match first
        for entry in &index.profiles {
            if entry.profile_type != expected_type {
                continue;
            }
            if entry.id.to_lowercase() == query_lower || entry.name.to_lowercase() == query_lower {
                let path = self.resolve_index_path(entry);
                if let Some(checksum) = self.compute_checksum_for_path(&path) {
                    matches.push(ResolvedProfile {
                        path,
                        source: ProfileSource::Library {
                            vendor: entry.vendor.clone(),
                        },
                        profile_type: entry.profile_type.clone(),
                        name: entry.name.clone(),
                        checksum,
                    });
                    return; // exact match, skip substring
                }
            }
        }

        // Substring match on name
        for entry in &index.profiles {
            if entry.profile_type != expected_type {
                continue;
            }
            if entry.name.to_lowercase().contains(&query_lower) {
                let path = self.resolve_index_path(entry);
                if let Some(checksum) = self.compute_checksum_for_path(&path) {
                    matches.push(ResolvedProfile {
                        path,
                        source: ProfileSource::Library {
                            vendor: entry.vendor.clone(),
                        },
                        profile_type: entry.profile_type.clone(),
                        name: entry.name.clone(),
                        checksum,
                    });
                }
            }
        }
    }

    fn resolve_index_path(&self, entry: &ProfileIndexEntry) -> PathBuf {
        // Try each library dir to find the actual file
        for lib_dir in &self.library_dirs {
            let full = lib_dir.join(&entry.path);
            if full.exists() {
                return full;
            }
        }
        // Fallback: return relative path from first library dir
        self.library_dirs
            .first()
            .map(|d| d.join(&entry.path))
            .unwrap_or_else(|| PathBuf::from(&entry.path))
    }

    fn compute_checksum_for_path(&self, path: &Path) -> Option<String> {
        std::fs::read_to_string(path)
            .ok()
            .map(|c| compute_checksum(&c))
    }

    fn find_type_mismatch(&self, query: &str, expected_type: &str) -> Option<ProfileError> {
        let other_types = ["machine", "filament", "process"];
        for otype in &other_types {
            if *otype == expected_type {
                continue;
            }
            // Check user dir
            if let Some(ref user_dir) = self.user_dir {
                let type_dir = user_dir.join(otype);
                if type_dir.is_dir() {
                    let mut temp = Vec::new();
                    self.search_directory(
                        &type_dir,
                        query,
                        otype,
                        ProfileSource::User,
                        &mut temp,
                    );
                    if !temp.is_empty() {
                        return Some(ProfileError::TypeMismatch {
                            query: query.to_string(),
                            expected_type: expected_type.to_string(),
                            actual_type: (*otype).to_string(),
                            hint: Some(format!("did you mean --{otype}?")),
                        });
                    }
                }
            }
            // Check library dirs
            for lib_dir in &self.library_dirs {
                let type_dir = lib_dir.join(otype);
                if type_dir.is_dir() {
                    let mut temp = Vec::new();
                    self.search_directory(
                        &type_dir,
                        query,
                        otype,
                        ProfileSource::Library {
                            vendor: String::new(),
                        },
                        &mut temp,
                    );
                    if !temp.is_empty() {
                        return Some(ProfileError::TypeMismatch {
                            query: query.to_string(),
                            expected_type: expected_type.to_string(),
                            actual_type: (*otype).to_string(),
                            hint: Some(format!("did you mean --{otype}?")),
                        });
                    }
                }
            }
            // Check index
            if let Some(ref index) = self.index {
                let query_lower = query.to_lowercase();
                for entry in &index.profiles {
                    if entry.profile_type == *otype
                        && entry.name.to_lowercase().contains(&query_lower)
                    {
                        return Some(ProfileError::TypeMismatch {
                            query: query.to_string(),
                            expected_type: expected_type.to_string(),
                            actual_type: (*otype).to_string(),
                            hint: Some(format!("did you mean --{otype}?")),
                        });
                    }
                }
            }
        }
        None
    }

    fn suggest_similar(&self, query: &str, expected_type: &str) -> Vec<String> {
        let mut candidates: Vec<String> = Vec::new();

        // Collect names from user dir
        if let Some(ref user_dir) = self.user_dir {
            let type_dir = user_dir.join(expected_type);
            if type_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&type_dir) {
                    for entry in entries.flatten() {
                        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                            candidates.push(stem.to_string());
                        }
                    }
                }
            }
        }

        // Collect names from library dirs
        for lib_dir in &self.library_dirs {
            let type_dir = lib_dir.join(expected_type);
            if type_dir.is_dir() {
                if let Ok(entries) = std::fs::read_dir(&type_dir) {
                    for entry in entries.flatten() {
                        if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                            if !candidates.contains(&stem.to_string()) {
                                candidates.push(stem.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Collect names from index
        if let Some(ref index) = self.index {
            for entry in &index.profiles {
                if entry.profile_type == expected_type
                    && !candidates.contains(&entry.name)
                {
                    candidates.push(entry.name.clone());
                }
            }
        }

        // Score using strsim
        let mut scored: Vec<(f64, String)> = candidates
            .into_iter()
            .map(|name| {
                let dist = strsim::jaro_winkler(&query.to_lowercase(), &name.to_lowercase());
                (dist, name)
            })
            .filter(|(score, _)| *score > 0.6)
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(3);
        scored.into_iter().map(|(_, name)| name).collect()
    }

    fn resolve_inheritance_inner(
        &self,
        profile_path: &Path,
        chain: &mut Vec<ResolvedProfile>,
        visited: &mut HashSet<PathBuf>,
        depth: usize,
    ) -> Result<(), ProfileError> {
        const MAX_DEPTH: usize = 5;
        if depth > MAX_DEPTH {
            return Err(ProfileError::CircularInheritance {
                chain: chain.iter().map(|p| p.name.clone()).collect(),
            });
        }

        let canonical = profile_path
            .canonicalize()
            .unwrap_or_else(|_| profile_path.to_path_buf());
        if !visited.insert(canonical.clone()) {
            let mut cycle_chain: Vec<String> = chain.iter().map(|p| p.name.clone()).collect();
            cycle_chain.push(
                profile_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            );
            return Err(ProfileError::CircularInheritance { chain: cycle_chain });
        }

        let content = std::fs::read_to_string(profile_path)?;
        let checksum = compute_checksum(&content);
        let name = profile_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let profile_type =
            extract_profile_type(&content).unwrap_or_else(|| "process".to_string());

        chain.push(ResolvedProfile {
            path: profile_path.to_path_buf(),
            source: ProfileSource::User,
            profile_type: profile_type.clone(),
            name,
            checksum,
        });

        // Check for inherits field
        let table: toml::Table = toml::from_str(&content).map_err(|e| {
            ProfileError::Parse(format!(
                "failed to parse '{}': {e}",
                profile_path.display()
            ))
        })?;

        if let Some(inherits) = table.get("inherits").and_then(|v| v.as_str()) {
            // Resolve parent using the same resolver
            let parent_path = if inherits.contains('/') || inherits.ends_with(".toml") {
                PathBuf::from(inherits)
            } else {
                // Look in the same directory
                let parent_file = format!("{inherits}.toml");
                profile_path
                    .parent()
                    .map(|d| d.join(&parent_file))
                    .unwrap_or_else(|| PathBuf::from(&parent_file))
            };

            if parent_path.exists() {
                self.resolve_inheritance_inner(&parent_path, chain, visited, depth + 1)?;
            }
        }

        Ok(())
    }
}

/// Extracts `profile_type` from TOML content.
fn extract_profile_type(content: &str) -> Option<String> {
    let table: toml::Table = toml::from_str(content).ok()?;
    table
        .get("profile_type")
        .and_then(|v| v.as_str())
        .map(String::from)
}

/// Computes SHA-256 hex digest of content.
fn compute_checksum(content: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(content.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Platform-aware home directory lookup.
#[cfg(not(target_arch = "wasm32"))]
fn home_dir() -> Option<PathBuf> {
    // Use $HOME on Unix, %USERPROFILE% on Windows
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a test profile TOML file.
    fn write_profile(dir: &Path, name: &str, profile_type: &str) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{name}.toml"));
        let content = format!("profile_type = \"{profile_type}\"\nname = \"{name}\"\n");
        fs::write(&path, content).unwrap();
        path
    }

    fn write_profile_with_inherits(
        dir: &Path,
        name: &str,
        profile_type: &str,
        inherits: &str,
    ) -> PathBuf {
        fs::create_dir_all(dir).unwrap();
        let path = dir.join(format!("{name}.toml"));
        let content = format!(
            "profile_type = \"{profile_type}\"\nname = \"{name}\"\ninherits = \"{inherits}\"\n"
        );
        fs::write(&path, content).unwrap();
        path
    }

    fn make_test_resolver(user_dir: Option<PathBuf>, library_dirs: Vec<PathBuf>) -> ProfileResolver {
        let index = library_dirs
            .iter()
            .find_map(|d| load_index(d).ok());
        ProfileResolver::with_dirs(user_dir, library_dirs, index)
    }

    fn make_test_index(entries: Vec<ProfileIndexEntry>) -> ProfileIndex {
        ProfileIndex {
            version: 1,
            generated: String::new(),
            profiles: entries,
        }
    }

    #[test]
    fn file_path_returns_directly() {
        let tmp = TempDir::new().unwrap();
        let path = write_profile(tmp.path(), "test_profile", "filament");

        let resolver = make_test_resolver(None, vec![]);
        let result = resolver.resolve(path.to_str().unwrap(), "filament").unwrap();
        assert_eq!(result.path, path);
        assert_eq!(result.profile_type, "filament");
    }

    #[test]
    fn file_path_with_slash_bypasses_search() {
        let tmp = TempDir::new().unwrap();
        let subdir = tmp.path().join("sub");
        let path = write_profile(&subdir, "test", "machine");

        let resolver = make_test_resolver(None, vec![]);
        // Query contains '/' so should be treated as file path
        let result = resolver
            .resolve(path.to_str().unwrap(), "machine")
            .unwrap();
        assert_eq!(result.path, path);
    }

    #[test]
    fn toml_extension_bypasses_search() {
        let tmp = TempDir::new().unwrap();
        let path = write_profile(tmp.path(), "direct", "process");

        let resolver = make_test_resolver(None, vec![]);
        let result = resolver
            .resolve(path.to_str().unwrap(), "process")
            .unwrap();
        assert_eq!(result.name, "direct");
    }

    #[test]
    fn exact_id_match_priority() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        // Create two profiles: one exact match, one substring match
        write_profile(&lib_dir.join("filament"), "PLA", "filament");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let result = resolver.resolve("PLA", "filament").unwrap();
        assert_eq!(result.name, "PLA");
    }

    #[test]
    fn substring_match_when_no_exact() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let result = resolver.resolve("basic", "filament").unwrap();
        assert_eq!(result.name, "PLA_Basic");
    }

    #[test]
    fn case_insensitive_matching() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let result = resolver.resolve("pla_basic", "filament").unwrap();
        assert_eq!(result.name, "PLA_Basic");
    }

    #[test]
    fn type_constraint_filters_results() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("machine"), "Bambu_X1C", "machine");
        write_profile(&lib_dir.join("filament"), "Bambu_PLA", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        // Searching for machine type should only find machine profiles
        let result = resolver.resolve("Bambu_X1C", "machine").unwrap();
        assert_eq!(result.profile_type, "machine");

        // Searching for filament should not find machine profiles
        let result = resolver.resolve("Bambu_X1C", "filament");
        assert!(result.is_err());
    }

    #[test]
    fn ambiguous_query_error() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Silk_Red", "filament");
        write_profile(&lib_dir.join("filament"), "PLA_Silk_Blue", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let result = resolver.resolve("Silk", "filament");
        assert!(result.is_err());
        let err = result.unwrap_err();
        match err {
            ProfileError::Ambiguous { query, matches } => {
                assert_eq!(query, "Silk");
                assert_eq!(matches.len(), 2);
            }
            other => panic!("expected Ambiguous, got: {other}"),
        }
    }

    #[test]
    fn user_profiles_searched_before_library() {
        let tmp = TempDir::new().unwrap();
        let user_dir = tmp.path().join("user");
        let lib_dir = tmp.path().join("lib");

        write_profile(&user_dir.join("filament"), "MyPLA", "filament");
        write_profile(&lib_dir.join("filament"), "MyPLA", "filament");

        let resolver = make_test_resolver(Some(user_dir.clone()), vec![lib_dir]);
        let result = resolver.resolve("MyPLA", "filament").unwrap();
        assert_eq!(result.source, ProfileSource::User);
        assert!(result.path.starts_with(&user_dir));
    }

    #[test]
    fn user_profile_shadows_library() {
        let tmp = TempDir::new().unwrap();
        let user_dir = tmp.path().join("user");
        let lib_dir = tmp.path().join("lib");

        let user_path = write_profile(&user_dir.join("filament"), "PLA_Basic", "filament");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");

        let resolver = make_test_resolver(Some(user_dir), vec![lib_dir]);
        let result = resolver.resolve("PLA_Basic", "filament").unwrap();
        // Should find user version, not library
        assert_eq!(result.source, ProfileSource::User);
        assert_eq!(result.path, user_path);
    }

    #[test]
    fn not_found_suggests_similar() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");
        write_profile(&lib_dir.join("filament"), "PLA_Silk", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let result = resolver.resolve("PLB_Basic", "filament");
        assert!(result.is_err());
        match result.unwrap_err() {
            ProfileError::NotFound { suggestions, .. } => {
                assert!(
                    !suggestions.is_empty(),
                    "should have suggestions for similar names"
                );
            }
            other => panic!("expected NotFound, got: {other}"),
        }
    }

    #[test]
    fn wrong_type_hints_correct_flag() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        // Try to find as machine - should get type mismatch hint
        let result = resolver.resolve("PLA_Basic", "machine");
        assert!(result.is_err());
        match result.unwrap_err() {
            ProfileError::TypeMismatch {
                hint,
                actual_type,
                ..
            } => {
                assert_eq!(actual_type, "filament");
                assert!(hint.unwrap().contains("--filament"));
            }
            other => panic!("expected TypeMismatch, got: {other}"),
        }
    }

    #[test]
    fn inherits_resolved_through_chain() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("filament");

        write_profile(&dir, "PLA_Base", "filament");
        write_profile_with_inherits(&dir, "PLA_Basic", "filament", "PLA_Base");

        let resolver = make_test_resolver(None, vec![]);
        let child_path = dir.join("PLA_Basic.toml");
        let chain = resolver.resolve_inheritance(&child_path).unwrap();

        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].name, "PLA_Base"); // root ancestor first
        assert_eq!(chain[1].name, "PLA_Basic"); // child last
    }

    #[test]
    fn circular_inheritance_detected() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("filament");

        // A -> B -> A (cycle)
        write_profile_with_inherits(&dir, "A", "filament", "B");
        write_profile_with_inherits(&dir, "B", "filament", "A");

        let resolver = make_test_resolver(None, vec![]);
        let result = resolver.resolve_inheritance(&dir.join("A.toml"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ProfileError::CircularInheritance { chain } => {
                assert!(chain.len() >= 2, "cycle chain should have at least 2 entries");
            }
            other => panic!("expected CircularInheritance, got: {other}"),
        }
    }

    #[test]
    fn library_directory_env_var_detection() {
        let tmp = TempDir::new().unwrap();
        let env_dir = tmp.path().join("env_profiles");
        fs::create_dir_all(&env_dir).unwrap();

        // Temporarily set env var
        std::env::set_var("SLICECORE_PROFILES_DIR", env_dir.to_str().unwrap());
        let dirs = ProfileResolver::find_library_dirs(None);
        std::env::remove_var("SLICECORE_PROFILES_DIR");

        assert!(
            dirs.contains(&env_dir),
            "should include env var directory"
        );
    }

    #[test]
    fn library_directory_override_detection() {
        let tmp = TempDir::new().unwrap();
        let override_dir = tmp.path().join("override_profiles");
        fs::create_dir_all(&override_dir).unwrap();

        // Clear env var to avoid interference
        std::env::remove_var("SLICECORE_PROFILES_DIR");
        let dirs = ProfileResolver::find_library_dirs(Some(&override_dir));
        assert!(
            dirs.contains(&override_dir),
            "should include CLI override directory"
        );
    }

    #[test]
    fn search_returns_all_matches() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        write_profile(&lib_dir.join("filament"), "PLA_Basic", "filament");
        write_profile(&lib_dir.join("filament"), "PLA_Silk", "filament");
        write_profile(&lib_dir.join("filament"), "PETG_Basic", "filament");

        let resolver = make_test_resolver(None, vec![lib_dir]);
        let results = resolver.search("PLA", Some("filament"), 10);
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn checksum_is_consistent() {
        let tmp = TempDir::new().unwrap();
        let path = write_profile(tmp.path(), "test", "filament");

        let resolver = make_test_resolver(None, vec![]);
        let r1 = resolver.resolve(path.to_str().unwrap(), "filament").unwrap();
        let r2 = resolver.resolve(path.to_str().unwrap(), "filament").unwrap();
        assert_eq!(r1.checksum, r2.checksum);
        assert!(!r1.checksum.is_empty());
    }

    #[test]
    fn index_based_search() {
        let tmp = TempDir::new().unwrap();
        let lib_dir = tmp.path().join("lib");
        fs::create_dir_all(&lib_dir).unwrap();

        // Write a profile file that the index references
        let filament_dir = lib_dir.join("orcaslicer").join("BBL").join("filament");
        fs::create_dir_all(&filament_dir).unwrap();
        let profile_path = filament_dir.join("PLA_Basic.toml");
        fs::write(&profile_path, "profile_type = \"filament\"\nname = \"PLA_Basic\"\n").unwrap();

        let index = make_test_index(vec![ProfileIndexEntry {
            id: "orcaslicer/BBL/filament/PLA_Basic".to_string(),
            name: "PLA_Basic".to_string(),
            source: "orcaslicer".to_string(),
            vendor: "BBL".to_string(),
            profile_type: "filament".to_string(),
            material: Some("PLA".to_string()),
            nozzle_size: None,
            printer_model: None,
            path: "orcaslicer/BBL/filament/PLA_Basic.toml".to_string(),
            layer_height: None,
            quality: None,
        }]);

        let resolver = ProfileResolver::with_dirs(None, vec![lib_dir], Some(index));
        let result = resolver.resolve("PLA_Basic", "filament").unwrap();
        assert_eq!(result.name, "PLA_Basic");
        assert!(matches!(result.source, ProfileSource::Library { .. }));
    }
}
