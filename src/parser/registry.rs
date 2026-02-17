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
//! Structure names follow these patterns:
//! - `NITF_02.10_FileHeader` → `nitf/nitf_02.10_file_header.ksy`
//! - `TRE_GEOLOB` → `tre/geolob.ksy`
//! - `DES_TRE_OVERFLOW` → `des/tre_overflow.ksy`

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
/// if let Some(def) = registry.get("NITF_02.10_FileHeader") {
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
    /// - `NITF_02.10_FileHeader` → `nitf/nitf_02.10_file_header.ksy`
    /// - `NSIF_01.00_FileHeader` → `nsif/nsif_01.00_file_header.ksy`
    /// - `TRE_GEOLOB` → `tre/geolob.ksy`
    /// - `DES_TRE_OVERFLOW` → `des/tre_overflow.ksy`
    pub fn name_to_filename(name: &str) -> PathBuf {
        // Handle TRE_ prefix
        if let Some(tre_name) = name.strip_prefix("TRE_") {
            let lower = tre_name.to_lowercase();
            return PathBuf::from("tre").join(format!("{}.ksy", lower));
        }

        // Handle DES_ prefix
        if let Some(des_name) = name.strip_prefix("DES_") {
            let lower = des_name.to_lowercase();
            return PathBuf::from("des").join(format!("{}.ksy", lower));
        }

        // Handle NITF_ prefix (e.g., NITF_02.10_FileHeader)
        if let Some(nitf_rest) = name.strip_prefix("NITF_") {
            // First convert CamelCase to snake_case, then lowercase
            let snake = Self::camel_to_snake(nitf_rest);
            let filename = format!("nitf_{}", snake);
            return PathBuf::from("nitf").join(format!("{}.ksy", filename));
        }

        // Handle NSIF_ prefix (e.g., NSIF_01.00_FileHeader)
        if let Some(nsif_rest) = name.strip_prefix("NSIF_") {
            let snake = Self::camel_to_snake(nsif_rest);
            let filename = format!("nsif_{}", snake);
            return PathBuf::from("nsif").join(format!("{}.ksy", filename));
        }

        // Default: convert to lowercase snake_case
        let snake = Self::camel_to_snake(name);
        PathBuf::from(format!("{}.ksy", snake))
    }

    /// Convert a filename path back to a structure name.
    fn filename_to_name(&self, path: &Path, base_dir: &Path) -> Option<String> {
        let relative = path.strip_prefix(base_dir).ok()?;
        let stem = path.file_stem()?.to_str()?;

        // Get the subdirectory if any
        let subdir = relative.parent().and_then(|p| p.to_str());

        match subdir {
            Some("tre") => {
                // tre/geolob.ksy → TRE_GEOLOB
                Some(format!("TRE_{}", stem.to_uppercase()))
            }
            Some("des") => {
                // des/tre_overflow.ksy → DES_TRE_OVERFLOW
                Some(format!("DES_{}", stem.to_uppercase()))
            }
            Some("nitf") => {
                // nitf/nitf_02.10_file_header.ksy → NITF_02.10_FileHeader
                let without_prefix = stem.strip_prefix("nitf_").unwrap_or(stem);
                Some(format!("NITF_{}", Self::snake_to_camel_preserve_version(without_prefix)))
            }
            Some("nsif") => {
                // nsif/nsif_01.00_file_header.ksy → NSIF_01.00_FileHeader
                let without_prefix = stem.strip_prefix("nsif_").unwrap_or(stem);
                Some(format!("NSIF_{}", Self::snake_to_camel_preserve_version(without_prefix)))
            }
            _ => {
                // Default: convert snake_case to CamelCase
                Some(Self::snake_to_camel(stem))
            }
        }
    }

    /// Convert CamelCase to snake_case.
    fn camel_to_snake(s: &str) -> String {
        let mut result = String::new();
        let mut prev_was_lower = false;
        let mut prev_was_underscore = false;

        for c in s.chars() {
            if c.is_uppercase() {
                // Add underscore before uppercase if:
                // - Previous char was lowercase (handles "FileHeader" → "file_header")
                // - Previous char wasn't underscore
                if prev_was_lower && !prev_was_underscore {
                    result.push('_');
                }
                result.push(c.to_lowercase().next().unwrap_or(c));
                prev_was_lower = false;
                prev_was_underscore = false;
            } else if c == '_' {
                result.push(c);
                prev_was_lower = false;
                prev_was_underscore = true;
            } else {
                result.push(c);
                prev_was_lower = c.is_lowercase();
                prev_was_underscore = false;
            }
        }

        result
    }

    /// Convert snake_case to CamelCase (preserving version numbers like 02.10).
    fn snake_to_camel(s: &str) -> String {
        let mut result = String::new();
        let mut capitalize_next = true;

        for c in s.chars() {
            if c == '_' {
                capitalize_next = true;
            } else if capitalize_next {
                result.push(c.to_uppercase().next().unwrap_or(c));
                capitalize_next = false;
            } else {
                result.push(c);
            }
        }

        result
    }

    /// Convert snake_case to CamelCase, preserving underscore before the CamelCase part.
    /// This handles version-prefixed names like "02.10_file_header" → "02.10_FileHeader"
    fn snake_to_camel_preserve_version(s: &str) -> String {
        // Find the first underscore that's followed by a letter (start of the name part)
        // e.g., "02.10_file_header" should split at the underscore before "file"
        // But only if the prefix looks like a version number (contains digits/dots)
        let mut split_pos = None;
        let chars: Vec<char> = s.chars().collect();
        
        for i in 0..chars.len() {
            if chars[i] == '_' && i + 1 < chars.len() && chars[i + 1].is_alphabetic() {
                // Check if the prefix looks like a version number (contains digits)
                let prefix: String = chars[..i].iter().collect();
                if prefix.chars().any(|c| c.is_ascii_digit()) {
                    split_pos = Some(i);
                    break;
                }
            }
        }

        match split_pos {
            Some(pos) => {
                // Keep the version part as-is, convert the rest to CamelCase
                let version_part = &s[..pos];
                let name_part = &s[pos + 1..]; // Skip the underscore
                format!("{}_{}", version_part, Self::snake_to_camel(name_part))
            }
            None => {
                // No version prefix, just convert normally
                Self::snake_to_camel(s)
            }
        }
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
        let path = StructureRegistry::name_to_filename("TRE_GEOLOB");
        assert_eq!(path, PathBuf::from("tre/geolob.ksy"));
    }

    #[test]
    fn name_to_filename_tre_with_underscore() {
        let path = StructureRegistry::name_to_filename("TRE_USE00A");
        assert_eq!(path, PathBuf::from("tre/use00a.ksy"));
    }

    #[test]
    fn name_to_filename_des() {
        let path = StructureRegistry::name_to_filename("DES_TRE_OVERFLOW");
        assert_eq!(path, PathBuf::from("des/tre_overflow.ksy"));
    }

    #[test]
    fn name_to_filename_nitf_file_header() {
        let path = StructureRegistry::name_to_filename("NITF_02.10_FileHeader");
        assert_eq!(path, PathBuf::from("nitf/nitf_02.10_file_header.ksy"));
    }

    #[test]
    fn name_to_filename_nitf_image_subheader() {
        let path = StructureRegistry::name_to_filename("NITF_02.10_ImageSubheader");
        assert_eq!(path, PathBuf::from("nitf/nitf_02.10_image_subheader.ksy"));
    }

    #[test]
    fn name_to_filename_nsif() {
        let path = StructureRegistry::name_to_filename("NSIF_01.00_FileHeader");
        assert_eq!(path, PathBuf::from("nsif/nsif_01.00_file_header.ksy"));
    }

    #[test]
    fn camel_to_snake_simple() {
        assert_eq!(StructureRegistry::camel_to_snake("FileHeader"), "file_header");
    }

    #[test]
    fn camel_to_snake_with_version() {
        assert_eq!(
            StructureRegistry::camel_to_snake("02.10_FileHeader"),
            "02.10_file_header"
        );
    }

    #[test]
    fn camel_to_snake_already_snake() {
        assert_eq!(
            StructureRegistry::camel_to_snake("file_header"),
            "file_header"
        );
    }

    #[test]
    fn snake_to_camel_simple() {
        assert_eq!(StructureRegistry::snake_to_camel("file_header"), "FileHeader");
    }

    #[test]
    fn snake_to_camel_with_version() {
        assert_eq!(
            StructureRegistry::snake_to_camel("02.10_file_header"),
            "02.10FileHeader"
        );
    }

    #[test]
    fn snake_to_camel_preserve_version_with_version() {
        assert_eq!(
            StructureRegistry::snake_to_camel_preserve_version("02.10_file_header"),
            "02.10_FileHeader"
        );
    }

    #[test]
    fn snake_to_camel_preserve_version_no_version() {
        assert_eq!(
            StructureRegistry::snake_to_camel_preserve_version("file_header"),
            "FileHeader"
        );
    }

    #[test]
    fn snake_to_camel_preserve_version_complex() {
        assert_eq!(
            StructureRegistry::snake_to_camel_preserve_version("01.00_image_subheader"),
            "01.00_ImageSubheader"
        );
    }

    // File-based tests using tempdir

    #[test]
    fn load_from_search_path() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        let ksy_content = create_test_ksy("geolob");
        fs::write(tre_dir.join("geolob.ksy"), ksy_content).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let def = registry.get("TRE_GEOLOB");
        assert!(def.is_some());
        assert_eq!(def.unwrap().id, "geolob");
    }

    #[test]
    fn list_includes_files_from_search_path() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        fs::write(tre_dir.join("geolob.ksy"), create_test_ksy("geolob")).unwrap();
        fs::write(tre_dir.join("use00a.ksy"), create_test_ksy("use00a")).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        assert!(names.contains(&"TRE_GEOLOB".to_string()));
        assert!(names.contains(&"TRE_USE00A".to_string()));
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
        fs::write(tre_dir1.join("geolob.ksy"), ksy1).unwrap();

        // Second path has definition with title "Second"
        let ksy2 = r#"meta:
  id: geolob
  title: Second
seq:
  - id: field1
    type: str
    size: 10
"#;
        fs::write(tre_dir2.join("geolob.ksy"), ksy2).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir1.path()); // Lower priority
        registry.add_search_path(temp_dir2.path()); // Higher priority

        let def = registry.get("TRE_GEOLOB").unwrap();
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
        fs::write(tre_dir.join("geolob.ksy"), ksy).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        // Register runtime definition
        let runtime_def = StructureDefinition::new("geolob").with_title("FromRuntime");
        registry.register("TRE_GEOLOB", runtime_def);

        let def = registry.get("TRE_GEOLOB").unwrap();
        assert_eq!(def.title, Some("FromRuntime".to_string()));
    }

    #[test]
    fn get_mut_caches_loaded_definition() {
        let temp_dir = TempDir::new().unwrap();
        let tre_dir = temp_dir.path().join("tre");
        fs::create_dir(&tre_dir).unwrap();

        fs::write(tre_dir.join("geolob.ksy"), create_test_ksy("geolob")).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        assert!(registry.file_cache.is_empty());

        // First call loads and caches
        let def1 = registry.get_mut("TRE_GEOLOB");
        assert!(def1.is_some());
        assert!(registry.file_cache.contains_key("TRE_GEOLOB"));

        // Second call uses cache
        let def2 = registry.get_mut("TRE_GEOLOB");
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
        assert!(names.contains(&"NITF_02.10_FileHeader".to_string()));
    }

    #[test]
    fn scan_des_directory() {
        let temp_dir = TempDir::new().unwrap();
        let des_dir = temp_dir.path().join("des");
        fs::create_dir(&des_dir).unwrap();

        fs::write(
            des_dir.join("tre_overflow.ksy"),
            create_test_ksy("tre_overflow"),
        )
        .unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        assert!(names.contains(&"DES_TRE_OVERFLOW".to_string()));
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

        // Create test files
        fs::write(
            nitf_dir.join("nitf_02.10_file_header.ksy"),
            create_test_ksy("nitf_file_header"),
        ).unwrap();
        fs::write(
            tre_dir.join("geolob.ksy"),
            create_test_ksy("geolob"),
        ).unwrap();
        fs::write(
            des_dir.join("tre_overflow.ksy"),
            create_test_ksy("tre_overflow"),
        ).unwrap();

        let mut registry = StructureRegistry::new();
        registry.add_search_path(temp_dir.path());

        let names = registry.list();
        
        // Verify expected names are present
        assert!(names.contains(&"NITF_02.10_FileHeader".to_string()));
        assert!(names.contains(&"TRE_GEOLOB".to_string()));
        assert!(names.contains(&"DES_TRE_OVERFLOW".to_string()));

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
        "[A-Z][A-Z0-9_]{2,15}".prop_map(|s| s.to_string())
    }

    /// Generate a valid structure id (lowercase with underscores)
    fn valid_struct_id() -> impl Strategy<Value = String> {
        "[a-z][a-z0-9_]{2,15}".prop_map(|s| s.to_string())
    }

    /// Generate a valid TRE name
    fn valid_tre_name() -> impl Strategy<Value = String> {
        "[A-Z][A-Z0-9]{2,6}".prop_map(|s| format!("TRE_{}", s))
    }

    /// Generate a valid DES name
    fn valid_des_name() -> impl Strategy<Value = String> {
        "[A-Z][A-Z0-9_]{2,10}".prop_map(|s| format!("DES_{}", s))
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
            tre_suffix in "[A-Z][A-Z0-9]{2,5}",
            id in valid_struct_id(),
        ) {
            let temp_dir1 = TempDir::new().unwrap();
            let temp_dir2 = TempDir::new().unwrap();

            let tre_dir1 = temp_dir1.path().join("tre");
            let tre_dir2 = temp_dir2.path().join("tre");
            fs::create_dir(&tre_dir1).unwrap();
            fs::create_dir(&tre_dir2).unwrap();

            let name = format!("TRE_{}", tre_suffix);
            let filename = format!("{}.ksy", tre_suffix.to_lowercase());

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
                let tre_suffix = name.strip_prefix("TRE_").unwrap();
                let filename = format!("{}.ksy", tre_suffix.to_lowercase());
                let ksy = create_ksy_content(&tre_suffix.to_lowercase(), None);
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
            tre_suffix in "[A-Z][A-Z0-9]{2,5}",
            file_id in valid_struct_id(),
            runtime_id in valid_struct_id(),
        ) {
            let temp_dir = TempDir::new().unwrap();
            let tre_dir = temp_dir.path().join("tre");
            fs::create_dir(&tre_dir).unwrap();

            let name = format!("TRE_{}", tre_suffix);
            let filename = format!("{}.ksy", tre_suffix.to_lowercase());

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
