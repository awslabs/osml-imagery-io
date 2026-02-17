//! Structure definition registry with search path resolution.
//!
//! The [`StructureRegistry`] manages loading, caching, and lookup of structure
//! definitions from multiple search paths with priority ordering.
//!
//! # Search Path Priority
//!
//! Definitions are searched in the following order (later overrides earlier):
//! 1. Built-in definitions compiled into the library
//! 2. Package data directory: `$CARGO_MANIFEST_DIR/data/structures/`
//! 3. Paths from `OSML_IO_STRUCTURE_PATH` environment variable
//! 4. Runtime-registered definitions (highest priority)
//!
//! # Naming Convention
//!
//! Structure names match the filename stem directly (no case conversion):
//! - `nitf_02.10_file_header` → `nitf/nitf_02.10_file_header.ksy`
//! - `nsif_01.00_file_header` → `nsif/nsif_01.00_file_header.ksy`
//! - `tre_geolob` → `tre/tre_geolob.ksy`
//! - `des_tre_overflow` → `des/des_tre_overflow.ksy`
//!
//! The subdirectory is determined by the prefix: `nitf_`, `nsif_`, `tre_`, `des_`.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::definition::DefinitionLoader;
use super::error::LoadError;
use super::types::StructureDefinition;

/// Environment variable for additional structure search paths.
const STRUCTURE_PATH_ENV: &str = "OSML_IO_STRUCTURE_PATH";

/// Registry for structure definitions with search path resolution.
///
/// The registry manages loading, caching, and lookup of structure definitions
/// from multiple search paths. Definitions can be loaded from KSY files on disk
/// or registered at runtime.
///
/// # Example
///
/// ```no_run
/// use osml_io::parser::StructureRegistry;
///
/// let mut registry = StructureRegistry::new();
/// registry.add_search_path("/custom/structures");
///
/// if let Some(def) = registry.get("nitf_02.10_file_header") {
///     println!("Found definition: {}", def.id);
/// }
///
/// for name in registry.list() {
///     println!("Available: {}", name);
/// }
/// ```
pub struct StructureRegistry {
    /// Runtime-registered definitions (highest priority)
    runtime_definitions: HashMap<String, Arc<StructureDefinition>>,
    /// Cached definitions loaded from files
    file_cache: HashMap<String, Arc<StructureDefinition>>,
    /// Search paths in priority order (later overrides earlier)
    search_paths: Vec<PathBuf>,
}

impl StructureRegistry {
    /// Create registry with default search paths.
    ///
    /// Default search paths include:
    /// 1. Package data directory (`data/structures/` relative to crate root)
    /// 2. Paths from `OSML_IO_STRUCTURE_PATH` environment variable
    pub fn new() -> Self {
        let mut registry = Self {
            runtime_definitions: HashMap::new(),
            file_cache: HashMap::new(),
            search_paths: Vec::new(),
        };

        // Add package data directory (relative to crate root)
        if let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") {
            let package_data = PathBuf::from(manifest_dir).join("data").join("structures");
            if package_data.exists() {
                registry.search_paths.push(package_data);
            }
        }

        // Also check relative to current directory for runtime usage
        let local_data = PathBuf::from("data").join("structures");
        if local_data.exists() && !registry.search_paths.contains(&local_data) {
            registry.search_paths.push(local_data);
        }

        // Add paths from environment variable
        if let Ok(env_paths) = env::var(STRUCTURE_PATH_ENV) {
            for path_str in env_paths.split(':') {
                let path = PathBuf::from(path_str);
                if path.exists() && !registry.search_paths.contains(&path) {
                    registry.search_paths.push(path);
                }
            }
        }

        registry
    }

    /// Add a search path (higher priority than existing paths).
    ///
    /// The new path will be searched before previously added paths.
    /// If the path doesn't exist, it will still be added but won't
    /// contribute any definitions until it exists.
    pub fn add_search_path(&mut self, path: impl AsRef<Path>) {
        let path_buf = path.as_ref().to_path_buf();
        if !self.search_paths.contains(&path_buf) {
            self.search_paths.push(path_buf);
        }
    }

    /// Get a structure definition by name.
    ///
    /// Searches in priority order:
    /// 1. Runtime-registered definitions
    /// 2. File cache
    /// 3. Search paths (loading and caching if found)
    ///
    /// Returns `None` if the definition is not found.
    pub fn get(&self, name: &str) -> Option<Arc<StructureDefinition>> {
        // Check runtime definitions first (highest priority)
        if let Some(def) = self.runtime_definitions.get(name) {
            return Some(Arc::clone(def));
        }

        // Check file cache
        if let Some(def) = self.file_cache.get(name) {
            return Some(Arc::clone(def));
        }

        // Try to load from search paths
        // Note: We need interior mutability for caching, but for now
        // we'll just load without caching in the immutable get()
        self.load_from_paths(name)
    }

    /// Get a structure definition by name, loading and caching if necessary.
    ///
    /// This is the mutable version that updates the cache.
    pub fn get_mut(&mut self, name: &str) -> Option<Arc<StructureDefinition>> {
        // Check runtime definitions first (highest priority)
        if let Some(def) = self.runtime_definitions.get(name) {
            return Some(Arc::clone(def));
        }

        // Check file cache
        if let Some(def) = self.file_cache.get(name) {
            return Some(Arc::clone(def));
        }

        // Try to load from search paths and cache
        if let Some(def) = self.load_from_paths(name) {
            self.file_cache.insert(name.to_string(), Arc::clone(&def));
            return Some(def);
        }

        None
    }

    /// List all available structure names.
    ///
    /// Returns names from:
    /// - Runtime-registered definitions
    /// - Cached definitions
    /// - All KSY files found in search paths
    pub fn list(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();

        // Add runtime definitions
        names.extend(self.runtime_definitions.keys().cloned());

        // Add cached definitions
        for name in self.file_cache.keys() {
            if !names.contains(name) {
                names.push(name.clone());
            }
        }

        // Scan search paths for KSY files
        for path in &self.search_paths {
            if let Ok(entries) = self.scan_directory(path) {
                for name in entries {
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
            }
        }

        names.sort();
        names
    }

    /// Reload all definitions from disk.
    ///
    /// Clears the file cache and re-scans search paths.
    /// Runtime-registered definitions are preserved.
    pub fn reload(&mut self) -> Result<(), LoadError> {
        self.file_cache.clear();
        Ok(())
    }

    /// Register a definition at runtime (highest priority).
    ///
    /// Runtime-registered definitions take priority over file-based
    /// definitions with the same name.
    pub fn register(&mut self, name: &str, def: StructureDefinition) {
        self.runtime_definitions
            .insert(name.to_string(), Arc::new(def));
    }

    /// Unregister a runtime definition.
    ///
    /// Returns the definition if it was registered, or `None` if not found.
    pub fn unregister(&mut self, name: &str) -> Option<Arc<StructureDefinition>> {
        self.runtime_definitions.remove(name)
    }

    /// Get the current search paths.
    pub fn search_paths(&self) -> &[PathBuf] {
        &self.search_paths
    }

    /// Clear the file cache.
    pub fn clear_cache(&mut self) {
        self.file_cache.clear();
    }

    /// Load a definition from search paths.
    fn load_from_paths(&self, name: &str) -> Option<Arc<StructureDefinition>> {
        let filename = Self::name_to_filename(name);

        // Search in reverse order (later paths have higher priority)
        for path in self.search_paths.iter().rev() {
            let full_path = path.join(&filename);
            if full_path.exists() {
                if let Ok(def) = DefinitionLoader::load_file(&full_path) {
                    return Some(Arc::new(def));
                }
            }
        }

        None
    }

    /// Scan a directory for KSY files and return structure names.
    fn scan_directory(&self, dir: &Path) -> Result<Vec<String>, std::io::Error> {
        let mut names = Vec::new();

        if !dir.exists() {
            return Ok(names);
        }

        // Scan subdirectories (nitf/, tre/, des/, etc.)
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Scan KSY files in subdirectory
                if let Ok(subdir_entries) = fs::read_dir(&path) {
                    for sub_entry in subdir_entries.flatten() {
                        let sub_path = sub_entry.path();
                        if sub_path.extension().map_or(false, |ext| ext == "ksy") {
                            if let Some(name) = self.filename_to_name(&sub_path, dir) {
                                names.push(name);
                            }
                        }
                    }
                }
            } else if path.extension().map_or(false, |ext| ext == "ksy") {
                // KSY file directly in the root
                if let Some(name) = self.filename_to_name(&path, dir) {
                    names.push(name);
                }
            }
        }

        Ok(names)
    }

    /// Convert a structure name to a filename path.
    ///
    /// # Naming Convention
    ///
    /// Structure names match the filename stem directly. The subdirectory
    /// is determined by the prefix:
    /// - `nitf_02.10_file_header` → `nitf/nitf_02.10_file_header.ksy`
    /// - `nsif_01.00_file_header` → `nsif/nsif_01.00_file_header.ksy`
    /// - `tre_geolob` → `tre/tre_geolob.ksy`
    /// - `des_tre_overflow` → `des/des_tre_overflow.ksy`
    pub fn name_to_filename(name: &str) -> PathBuf {
        let subdir = if name.starts_with("nitf_") {
            "nitf"
        } else if name.starts_with("nsif_") {
            "nsif"
        } else if name.starts_with("tre_") {
            "tre"
        } else if name.starts_with("des_") {
            "des"
        } else {
            ""
        };

        if subdir.is_empty() {
            PathBuf::from(format!("{}.ksy", name))
        } else {
            PathBuf::from(subdir).join(format!("{}.ksy", name))
        }
    }

    /// Convert a filename path back to a structure name.
    ///
    /// Simply returns the file stem (filename without extension).
    fn filename_to_name(&self, path: &Path, _base_dir: &Path) -> Option<String> {
        path.file_stem()?.to_str().map(|s| s.to_string())
    }
}

impl Default for StructureRegistry {
    fn default() -> Self {
        Self::new()
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::types::{Endian, StructureDefinition};
    use std::fs;
    use tempfile::TempDir;

    fn create_test_ksy(id: &str) -> String {
        format!(
            r#"meta:
  id: {}
seq:
  - id: field1
    type: str
    size: 10
"#,
            id
        )
    }

    #[test]
    fn new_creates_empty_registry() {
        let registry = StructureRegistry::new();
        assert!(registry.runtime_definitions.is_empty());
        assert!(registry.file_cache.is_empty());
    }

    #[test]
    fn add_search_path_adds_path() {
        let mut registry = StructureRegistry::new();
        let initial_count = registry.search_paths.len();
        registry.add_search_path("/test/path");
        assert_eq!(registry.search_paths.len(), initial_count + 1);
        assert!(registry.search_paths.contains(&PathBuf::from("/test/path")));
    }

    #[test]
    fn add_search_path_no_duplicates() {
        let mut registry = StructureRegistry::new();
        registry.add_search_path("/test/path");
        let count = registry.search_paths.len();
        registry.add_search_path("/test/path");
        assert_eq!(registry.search_paths.len(), count);
    }

    #[test]
    fn register_adds_runtime_definition() {
        let mut registry = StructureRegistry::new();
        let def = StructureDefinition::new("test_struct");
        registry.register("TEST", def);
        assert!(registry.runtime_definitions.contains_key("TEST"));
    }

    #[test]
    fn get_returns_runtime_definition() {
        let mut registry = StructureRegistry::new();
        let def = StructureDefinition::new("test_struct")
            .with_title("Test Structure")
            .with_endian(Endian::Little);
        registry.register("TEST", def);

        let result = registry.get("TEST");
        assert!(result.is_some());
        let retrieved = result.unwrap();
        assert_eq!(retrieved.id, "test_struct");
        assert_eq!(retrieved.title, Some("Test Structure".to_string()));
        assert_eq!(retrieved.endian, Endian::Little);
    }

    #[test]
    fn get_returns_none_for_unknown() {
        let registry = StructureRegistry::new();
        assert!(registry.get("UNKNOWN").is_none());
    }

    #[test]
    fn unregister_removes_definition() {
        let mut registry = StructureRegistry::new();
        let def = StructureDefinition::new("test_struct");
        registry.register("TEST", def);
        assert!(registry.get("TEST").is_some());

        let removed = registry.unregister("TEST");
        assert!(removed.is_some());
        assert!(registry.get("TEST").is_none());
    }

    #[test]
    fn list_includes_runtime_definitions() {
        let mut registry = StructureRegistry::new();
        registry.register("TEST_A", StructureDefinition::new("test_a"));
        registry.register("TEST_B", StructureDefinition::new("test_b"));

        let names = registry.list();
        assert!(names.contains(&"TEST_A".to_string()));
        assert!(names.contains(&"TEST_B".to_string()));
    }

    #[test]
    fn list_returns_sorted_names() {
        let mut registry = StructureRegistry::new();
        registry.register("ZZZ", StructureDefinition::new("zzz"));
        registry.register("AAA", StructureDefinition::new("aaa"));
        registry.register("MMM", StructureDefinition::new("mmm"));

        let names = registry.list();
        let sorted: Vec<_> = names.iter().filter(|n| n.len() == 3).cloned().collect();
        assert_eq!(sorted, vec!["AAA", "MMM", "ZZZ"]);
    }

    #[test]
    fn reload_clears_file_cache() {
        let mut registry = StructureRegistry::new();
        registry
            .file_cache
            .insert("TEST".to_string(), Arc::new(StructureDefinition::new("test")));
        assert!(!registry.file_cache.is_empty());

        registry.reload().unwrap();
        assert!(registry.file_cache.is_empty());
    }

    #[test]
    fn reload_preserves_runtime_definitions() {
        let mut registry = StructureRegistry::new();
        registry.register("TEST", StructureDefinition::new("test"));
        registry.reload().unwrap();
        assert!(registry.runtime_definitions.contains_key("TEST"));
    }

    #[test]
    fn clear_cache_clears_file_cache() {
        let mut registry = StructureRegistry::new();
        registry
            .file_cache
            .insert("TEST".to_string(), Arc::new(StructureDefinition::new("test")));
        registry.clear_cache();
        assert!(registry.file_cache.is_empty());
    }

    // Naming convention tests

    #[test]
    fn name_to_filename_tre() {
        let path = StructureRegistry::name_to_filename("tre_geolob");
        assert_eq!(path, PathBuf::from("tre/tre_geolob.ksy"));
    }

    #[test]
    fn name_to_filename_tre_with_underscore() {
        let path = StructureRegistry::name_to_filename("tre_use00a");
        assert_eq!(path, PathBuf::from("tre/tre_use00a.ksy"));
    }

    #[test]
    fn name_to_filename_des() {
        let path = StructureRegistry::name_to_filename("des_tre_overflow");
        assert_eq!(path, PathBuf::from("des/des_tre_overflow.ksy"));
    }

    #[test]
    fn name_to_filename_nitf_file_header() {
        let path = StructureRegistry::name_to_filename("nitf_02.10_file_header");
        assert_eq!(path, PathBuf::from("nitf/nitf_02.10_file_header.ksy"));
    }

    #[test]
    fn name_to_filename_nitf_image_subheader() {
        let path = StructureRegistry::name_to_filename("nitf_02.10_image_subheader");
        assert_eq!(path, PathBuf::from("nitf/nitf_02.10_image_subheader.ksy"));
    }

    #[test]
    fn name_to_filename_nsif() {
        let path = StructureRegistry::name_to_filename("nsif_01.00_file_header");
        assert_eq!(path, PathBuf::from("nsif/nsif_01.00_file_header.ksy"));
    }

    #[test]
    fn name_to_filename_unknown_prefix() {
        // Names without recognized prefix go to root
        let path = StructureRegistry::name_to_filename("custom_structure");
        assert_eq!(path, PathBuf::from("custom_structure.ksy"));
    }

    // File-based tests using tempdir

    #[test]
    fn load_from_search_path() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        let ksy_content = create_test_ksy("geolob");
        fs::write(tre_dir.join("tre_geolob.ksy"), ksy_content).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let def = registry.get("tre_geolob");
        assert!(def.is_some());
        assert_eq!(def.unwrap().id, "geolob");
    }

    #[test]
    fn list_includes_files_from_search_path() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        fs::write(tre_dir.join("tre_geolob.ksy"), create_test_ksy("geolob")).unwrap();
        fs::write(tre_dir.join("tre_use00a.ksy"), create_test_ksy("use00a")).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        assert!(names.contains(&"tre_geolob".to_string()));
        assert!(names.contains(&"tre_use00a".to_string()));
    }

    #[test]
    fn search_path_priority_later_wins() {
        let temp_dir1 = TempDir::new().unwrap();
        let temp_dir2 = TempDir::new().unwrap();

        let tre_dir1 = temp_dir1.path().join("tre");
        let tre_dir2 = temp_dir2.path().join("tre");
        fs::create_dir(&tre_dir1).unwrap();
        fs::create_dir(&tre_dir2).unwrap();

        // First path has definition with title "First"
        let ksy1 = r#"meta:
  id: geolob
  title: First
seq:
  - id: field1
    type: str
    size: 10
"#;
        fs::write(tre_dir1.join("tre_geolob.ksy"), ksy1).unwrap();

        // Second path has definition with title "Second"
        let ksy2 = r#"meta:
  id: geolob
  title: Second
seq:
  - id: field1
    type: str
    size: 10
"#;
        fs::write(tre_dir2.join("tre_geolob.ksy"), ksy2).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir1.path()); // Lower priority
        registry.add_search_path(temp_dir2.path()); // Higher priority

        let def = registry.get("tre_geolob").unwrap();
        assert_eq!(def.title, Some("Second".to_string()));
    }

    #[test]
    fn runtime_definition_takes_priority_over_file() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        let ksy = r#"meta:
  id: geolob
  title: FromFile
seq:
  - id: field1
    type: str
    size: 10
"#;
        fs::write(tre_dir.join("tre_geolob.ksy"), ksy).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        // Register runtime definition
        let runtime_def = StructureDefinition::new("geolob").with_title("FromRuntime");
        registry.register("tre_geolob", runtime_def);

        let def = registry.get("tre_geolob").unwrap();
        assert_eq!(def.title, Some("FromRuntime".to_string()));
    }

    #[test]
    fn get_mut_caches_loaded_definition() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        fs::write(tre_dir.join("tre_geolob.ksy"), create_test_ksy("geolob")).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        assert!(registry.file_cache.is_empty());

        // First call loads and caches
        let def1 = registry.get_mut("tre_geolob");
        assert!(def1.is_some());
        assert!(registry.file_cache.contains_key("tre_geolob"));

        // Second call uses cache
        let def2 = registry.get_mut("tre_geolob");
        assert!(def2.is_some());
        assert!(Arc::ptr_eq(&def1.unwrap(), &def2.unwrap()));
    }

    #[test]
    fn scan_nitf_directory() {
        let temp_dir = TempDir::new().unwrap();
        let nitf_dir = temp_dir.path().join("nitf");
        fs::create_dir(&nitf_dir).unwrap();

        fs::write(
            nitf_dir.join("nitf_02.10_file_header.ksy"),
            create_test_ksy("nitf_02_10_file_header"),
        )
        .unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        assert!(names.contains(&"nitf_02.10_file_header".to_string()));
    }

    #[test]
    fn scan_des_directory() {
        let temp_dir = TempDir::new().unwrap();
        let des_dir = temp_dir.path().join("des");
        fs::create_dir(&des_dir).unwrap();

        fs::write(
            des_dir.join("des_tre_overflow.ksy"),
            create_test_ksy("tre_overflow"),
        )
        .unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        assert!(names.contains(&"des_tre_overflow".to_string()));
    }

    #[test]
    fn list_get_consistency_nitf() {
        let temp_dir = TempDir::new().unwrap();
        let nitf_dir = temp_dir.path().join("nitf");
        fs::create_dir(&nitf_dir).unwrap();

        fs::write(
            nitf_dir.join("nitf_02.10_file_header.ksy"),
            create_test_ksy("nitf_02_10_file_header"),
        )
        .unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        // Every name returned by list() should be resolvable via get()
        // Note: list() may include names from default search paths too
        let names = registry.list();
        for name in &names {
            let def = registry.get(name);
            assert!(
                def.is_some(),
                "Name '{}' from list() should be resolvable via get()",
                name
            );
        }
    }

    #[test]
    fn list_get_consistency_all_types() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create directories for all types
        let nitf_dir = temp_dir.path().join("nitf");
        let tre_dir = temp_dir.path().join("tre");
        let des_dir = temp_dir.path().join("des");
        fs::create_dir(&nitf_dir).unwrap();
        fs::create_dir(&tre_dir).unwrap();
        fs::create_dir(&des_dir).unwrap();

        // Create test files with correct naming convention
        fs::write(
            nitf_dir.join("nitf_02.10_file_header.ksy"),
            create_test_ksy("nitf_file_header"),
        ).unwrap();
        fs::write(
            tre_dir.join("tre_geolob.ksy"),
            create_test_ksy("geolob"),
        ).unwrap();
        fs::write(
            des_dir.join("des_tre_overflow.ksy"),
            create_test_ksy("tre_overflow"),
        ).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        
        // Verify expected names are present
        assert!(names.contains(&"nitf_02.10_file_header".to_string()));
        assert!(names.contains(&"tre_geolob".to_string()));
        assert!(names.contains(&"des_tre_overflow".to_string()));

        // Every name should be resolvable
        for name in &names {
            let def = registry.get(name);
            assert!(
                def.is_some(),
                "Name '{}' from list() should be resolvable via get()",
                name
            );
        }
    }
}


/// Property-based tests for the structure registry.
#[cfg(test)]
mod proptests {
    use super::*;
    use crate::parser::types::StructureDefinition;
    use proptest::prelude::*;
    use std::fs;
    use tempfile::TempDir;

    /// Generate a valid structure name (alphanumeric with underscores)
    fn valid_struct_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{2,15}".prop_map(|s| s.to_string())
    }

    /// Generate a valid structure id (lowercase with underscores)
    fn valid_struct_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{2,15}".prop_map(|s| s.to_string())
    }

    /// Generate a valid TRE name (now lowercase with tre_ prefix)
    fn valid_tre_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9]{2,6}".prop_map(|s| format!("tre_{}", s))
    }

    /// Generate a valid DES name (now lowercase with des_ prefix)
    fn valid_des_name() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{2,10}".prop_map(|s| format!("des_{}", s))
    }

    /// Create a minimal valid KSY file content
    fn create_ksy_content(id: &str, title: Option<&str>) -> String {
        let title_line = title
            .map(|t| format!("  title: {}\n", t))
            .unwrap_or_default();
        format!(
            r#"meta:
  id: {}
{}seq:
  - id: field1
    type: str
    size: 10
"#,
            id, title_line
        )
    }

    /// Property 25: Registry Search Path Priority
    /// For any structure name with definitions in multiple search paths,
    /// get() SHALL return the definition from the highest-priority path.
    /// **Validates: Requirements 11.2, 11.4, 11.5**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn prop_25_search_path_priority(
            tre_suffix in "[a-z][a-z0-9]{2,5}",
            id in valid_struct_id(),
        ) {
            let temp_dir1 = TempDir::new().unwrap();
            let temp_dir2 = TempDir::new().unwrap();

            let tre_dir1 = temp_dir1.path().join("tre");
            let tre_dir2 = temp_dir2.path().join("tre");
            fs::create_dir(&tre_dir1).unwrap();
            fs::create_dir(&tre_dir2).unwrap();

            let name = format!("tre_{}", tre_suffix);
            let filename = format!("{}.ksy", name);

            // First path (lower priority) has title "LowPriority"
            let ksy1 = create_ksy_content(&id, Some("LowPriority"));
            fs::write(tre_dir1.join(&filename), &ksy1).unwrap();

            // Second path (higher priority) has title "HighPriority"
            let ksy2 = create_ksy_content(&id, Some("HighPriority"));
            fs::write(tre_dir2.join(&filename), &ksy2).unwrap();

            let mut registry = StructureRegistry::new();
            registry.add_search_path(temp_dir1.path()); // Lower priority
            registry.add_search_path(temp_dir2.path()); // Higher priority

            let def = registry.get(&name);
            prop_assert!(def.is_some(), "Definition should be found for {}", name);
            prop_assert_eq!(
                def.unwrap().title.clone(),
                Some("HighPriority".to_string()),
                "Higher priority path should win"
            );
        }
    }

    /// Property 26: Registry List Completeness
    /// For any registry, list() SHALL return all structure names that are
    /// resolvable via get().
    /// **Validates: Requirements 11.6**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(30))]
        #[test]
        fn prop_26_list_completeness(
            tre_names in prop::collection::vec(valid_tre_name(), 1..5),
            runtime_names in prop::collection::vec(valid_struct_name(), 0..3),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let tre_dir = temp_dir.path().join("tre");
            fs::create_dir(&tre_dir).unwrap();

            // Create KSY files for TRE names
            let mut expected_names: Vec<String> = Vec::new();
            for name in &tre_names {
                // Name is already in correct format: tre_xxx
                let filename = format!("{}.ksy", name);
                let id = name.strip_prefix("tre_").unwrap_or(name);
                let ksy = create_ksy_content(id, None);
                fs::write(tre_dir.join(&filename), &ksy).unwrap();
                if !expected_names.contains(name) {
                    expected_names.push(name.clone());
                }
            }

            let mut registry = StructureRegistry::new();
            registry.add_search_path(temp_dir.path());

            // Register runtime definitions
            for name in &runtime_names {
                let def = StructureDefinition::new(name.to_lowercase());
                registry.register(name, def);
                if !expected_names.contains(name) {
                    expected_names.push(name.clone());
                }
            }

            let listed = registry.list();

            // Every name in list() should be resolvable via get()
            for name in &listed {
                if expected_names.contains(name) {
                    let def = registry.get(name);
                    prop_assert!(
                        def.is_some(),
                        "Listed name '{}' should be resolvable via get()",
                        name
                    );
                }
            }

            // Every expected name should be in list()
            for name in &expected_names {
                prop_assert!(
                    listed.contains(name),
                    "Expected name '{}' should be in list()",
                    name
                );
            }
        }
    }

    /// Property 27: Runtime Registration Priority
    /// For any structure name, a runtime-registered definition SHALL take
    /// priority over file-based definitions.
    /// **Validates: Requirements 11.8**
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        #[test]
        fn prop_27_runtime_registration_priority(
            tre_suffix in "[a-z][a-z0-9]{2,5}",
            file_id in valid_struct_id(),
            runtime_id in valid_struct_id(),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let tre_dir = temp_dir.path().join("tre");
            fs::create_dir(&tre_dir).unwrap();

            let name = format!("tre_{}", tre_suffix);
            let filename = format!("{}.ksy", name);

            // Create file-based definition
            let ksy = create_ksy_content(&file_id, Some("FromFile"));
            fs::write(tre_dir.join(&filename), &ksy).unwrap();

            let mut registry = StructureRegistry::new();
            registry.add_search_path(temp_dir.path());

            // Verify file-based definition is found first
            let file_def = registry.get(&name);
            prop_assert!(file_def.is_some(), "File definition should be found");
            prop_assert_eq!(
                file_def.unwrap().title.clone(),
                Some("FromFile".to_string()),
                "Should get file definition before runtime registration"
            );

            // Register runtime definition
            let runtime_def = StructureDefinition::new(&runtime_id).with_title("FromRuntime");
            registry.register(&name, runtime_def);

            // Verify runtime definition takes priority
            let result = registry.get(&name);
            prop_assert!(result.is_some(), "Definition should still be found");
            prop_assert_eq!(
                result.unwrap().title.clone(),
                Some("FromRuntime".to_string()),
                "Runtime definition should take priority over file"
            );

            // Unregister and verify file definition is used again
            registry.unregister(&name);
            let after_unregister = registry.get(&name);
            prop_assert!(after_unregister.is_some(), "File definition should be found after unregister");
            prop_assert_eq!(
                after_unregister.unwrap().title.clone(),
                Some("FromFile".to_string()),
                "Should fall back to file definition after unregister"
            );
        }
    }
}
