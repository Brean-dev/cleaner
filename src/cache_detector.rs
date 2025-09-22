use crate::config::Config;
use glob::glob;
use jwalk::WalkDir;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Represents a detected cache directory or file
#[derive(Debug, Clone)]
pub struct CacheItem {
    pub path: PathBuf,
    pub cache_type: CacheType,
    pub size_bytes: Option<u64>,
    pub file_count: Option<usize>,
    pub last_modified: Option<SystemTime>,
}

/// Types of cache items
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum CacheType {
    UserCache,
    SystemCache,
    PackageManagerCache,
    ApplicationCache,
    BrowserCache,
    DevelopmentCache,
    BuildArtifact,
    TemporaryFile,
}

impl CacheType {
    pub fn description(&self) -> &'static str {
        match self {
            CacheType::UserCache => "User cache directory",
            CacheType::SystemCache => "System cache directory",
            CacheType::PackageManagerCache => "Package manager cache",
            CacheType::ApplicationCache => "Application cache",
            CacheType::BrowserCache => "Browser cache",
            CacheType::DevelopmentCache => "Development tool cache",
            CacheType::BuildArtifact => "Build artifact",
            CacheType::TemporaryFile => "Temporary file/directory",
        }
    }
}

/// Cache detection engine
pub struct CacheDetector {
    config: Config,
}

impl CacheDetector {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Detect all cache items under the given root path
    pub fn detect_cache_items<P: AsRef<Path>>(
        &self,
        root: P,
    ) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
        let root_path = root.as_ref();
        let mut cache_items = Vec::new();

        // Detect cache directories
        cache_items.extend(self.detect_cache_directories(root_path)?);

        // Detect build artifacts
        cache_items.extend(self.detect_build_artifacts(root_path)?);

        // Detect temporary files
        cache_items.extend(self.detect_temporary_files(root_path)?);

        // Remove duplicates and sort by type
        self.deduplicate_and_sort(cache_items)
    }

    /// Detect cache directories using various patterns
    fn detect_cache_directories(
        &self,
        root: &Path,
    ) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
        // Check if this is a user home directory scan
        let is_user_scan = self.is_user_directory(root);

        // Configure parallel walking with jwalk
        let max_threads = self
            .config
            .performance
            .max_threads
            .unwrap_or(rayon::current_num_threads());
        let parallelism = if max_threads == 1 {
            jwalk::Parallelism::Serial
        } else {
            jwalk::Parallelism::RayonNewPool(max_threads)
        };

        // Use parallel directory traversal with jwalk
        let entries: Result<Vec<_>, _> = WalkDir::new(root)
            .parallelism(parallelism)
            .max_depth(self.config.performance.max_depth.unwrap_or(10))
            .follow_links(!self.config.performance.skip_symlinks)
            .into_iter()
            .filter_map(|entry_result| match entry_result {
                Ok(entry) => {
                    if entry.file_type().is_dir() {
                        Some(Ok(entry))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            })
            .collect();

        let entries = entries?;

        // Use rayon for parallel processing of directory classification
        let items: Result<Vec<_>, _> = entries
            .into_par_iter()
            .filter_map(
                |entry| match self.classify_directory_entry(&entry, is_user_scan) {
                    Ok(Some(cache_item)) => Some(Ok(cache_item)),
                    Ok(None) => None,
                    Err(e) => Some(Err(format!("Classification error: {}", e))),
                },
            )
            .collect();

        match items {
            Ok(cache_items) => Ok(cache_items),
            Err(e) => Err(e.into()),
        }
    }

    /// Classify a directory entry as a cache item
    fn classify_directory_entry(
        &self,
        entry: &jwalk::DirEntry<((), ())>,
        is_user_scan: bool,
    ) -> Result<Option<CacheItem>, String> {
        let path = entry.path();
        let path_str = path.to_string_lossy().to_lowercase();

        // Skip excluded paths
        if self.config.is_excluded_path(&path) {
            return Ok(None);
        }

        // Determine cache type based on patterns
        let cache_type = if is_user_scan {
            self.classify_user_cache(&path_str)
        } else {
            self.classify_system_cache(&path_str)
        };

        if let Some(cache_type) = cache_type {
            let last_modified = std::fs::metadata(&path)
                .ok()
                .and_then(|m| m.modified().ok());

            let cache_item = CacheItem {
                path: path.to_path_buf(),
                cache_type,
                size_bytes: None, // Will be calculated later if needed
                file_count: None,
                last_modified,
            };
            Ok(Some(cache_item))
        } else {
            Ok(None)
        }
    }

    /// Classify user-level cache directories
    fn classify_user_cache(&self, path_str: &str) -> Option<CacheType> {
        // Browser caches
        for pattern in &self.config.cache_patterns.browser_caches {
            if self.matches_pattern(path_str, pattern) {
                return Some(CacheType::BrowserCache);
            }
        }

        // Development tool caches
        for pattern in &self.config.cache_patterns.dev_tool_caches {
            if self.matches_pattern(path_str, pattern) {
                return Some(CacheType::DevelopmentCache);
            }
        }

        // Package manager caches (user-level)
        for pattern in &self.config.cache_patterns.package_manager_caches {
            if pattern.starts_with('~') && self.matches_pattern(path_str, &pattern[2..]) {
                return Some(CacheType::PackageManagerCache);
            }
        }

        // User cache directories
        for pattern in &self.config.cache_patterns.user_cache_dirs {
            if self.matches_pattern(path_str, pattern) {
                return Some(CacheType::UserCache);
            }
        }

        // Application cache patterns
        for pattern in &self.config.cache_patterns.app_cache_patterns {
            if self.matches_pattern(path_str, pattern) {
                return Some(CacheType::ApplicationCache);
            }
        }

        None
    }

    /// Classify system-level cache directories
    fn classify_system_cache(&self, path_str: &str) -> Option<CacheType> {
        // System cache directories
        for pattern in &self.config.cache_patterns.system_cache_dirs {
            if self.matches_pattern(path_str, pattern) {
                return Some(CacheType::SystemCache);
            }
        }

        // Package manager caches (system-level)
        for pattern in &self.config.cache_patterns.package_manager_caches {
            if !pattern.starts_with('~') && self.matches_pattern(path_str, pattern) {
                return Some(CacheType::PackageManagerCache);
            }
        }

        // Check if it's a user cache under system scan
        if path_str.contains("/home/") {
            return self.classify_user_cache(path_str);
        }

        None
    }

    /// Detect build artifacts and temporary files
    fn detect_build_artifacts(
        &self,
        root: &Path,
    ) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
        let mut items = Vec::new();

        for pattern in &self.config.cache_patterns.build_artifacts {
            if let Ok(paths) = glob(&format!("{}/{}", root.display(), pattern)) {
                for path in paths.flatten() {
                    if path.exists() && !self.config.is_excluded_path(&path) {
                        items.push(CacheItem {
                            path,
                            cache_type: CacheType::BuildArtifact,
                            size_bytes: None,
                            file_count: None,
                            last_modified: None,
                        });
                    }
                }
            }
        }

        Ok(items)
    }

    /// Detect temporary files and directories
    fn detect_temporary_files(
        &self,
        root: &Path,
    ) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
        // Configure parallel walking with jwalk
        let max_threads = self
            .config
            .performance
            .max_threads
            .unwrap_or(rayon::current_num_threads());
        let parallelism = if max_threads == 1 {
            jwalk::Parallelism::Serial
        } else {
            jwalk::Parallelism::RayonNewPool(max_threads)
        };

        // Use parallel directory traversal with jwalk
        let entries: Result<Vec<_>, _> = WalkDir::new(root)
            .parallelism(parallelism)
            .max_depth(self.config.performance.max_depth.unwrap_or(10))
            .follow_links(!self.config.performance.skip_symlinks)
            .into_iter()
            .collect();

        let entries = entries?;

        // Use rayon for parallel processing of files
        let items: Result<Vec<_>, _> = entries
            .into_par_iter()
            .filter_map(|entry| {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_lowercase();

                if self.config.is_excluded_path(&path) {
                    return None;
                }

                // Check if this is a code file that should be excluded
                if let Some(extension) = path.extension()
                    && let Some(ext_str) = extension.to_str()
                {
                    let ext_str = format!(".{}", ext_str.to_lowercase());
                    let code_extensions = [
                        ".rs", ".go", ".js", ".ts", ".py", ".java", ".cpp", ".c", ".h", ".hpp",
                        ".cs", ".php", ".rb", ".swift", ".kt", ".scala", ".clj", ".hs", ".ml",
                        ".fs", ".vb", ".pl", ".sh", ".ps1", ".bat",
                    ];
                    if code_extensions.contains(&ext_str.as_str()) {
                        return None;
                    }
                }

                for pattern in &self.config.cache_patterns.temp_patterns {
                    if self.matches_pattern(&path_str, pattern) {
                        let last_modified = std::fs::metadata(&path)
                            .ok()
                            .and_then(|m| m.modified().ok());

                        return Some(Ok::<CacheItem, String>(CacheItem {
                            path: path.to_path_buf(),
                            cache_type: CacheType::TemporaryFile,
                            size_bytes: None,
                            file_count: None,
                            last_modified,
                        }));
                    }
                }
                None
            })
            .collect();

        match items {
            Ok(cache_items) => Ok(cache_items),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a path string matches a pattern (with simple wildcard support)
    fn matches_pattern(&self, path_str: &str, pattern: &str) -> bool {
        if pattern.contains('*') {
            // Simple glob-like matching
            let pattern_parts: Vec<&str> = pattern.split('*').collect();

            if pattern_parts.len() == 1 {
                return path_str.contains(pattern);
            }

            let mut current_pos = 0;
            for (i, part) in pattern_parts.iter().enumerate() {
                if part.is_empty() {
                    continue;
                }

                if i == 0 {
                    // First part must match from the beginning
                    if !path_str[current_pos..].starts_with(part) {
                        return false;
                    }
                    current_pos += part.len();
                } else if i == pattern_parts.len() - 1 {
                    // Last part must match at the end
                    return path_str[current_pos..].ends_with(part);
                } else {
                    // Middle parts can match anywhere
                    if let Some(pos) = path_str[current_pos..].find(part) {
                        current_pos += pos + part.len();
                    } else {
                        return false;
                    }
                }
            }
            true
        } else {
            path_str.contains(pattern)
        }
    }

    /// Check if a path is a user directory
    fn is_user_directory(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();
        path_str.starts_with("/home/") ||
        path_str.starts_with("/Users/") || // macOS compatibility
        path_str == std::env::var("HOME").unwrap_or_default()
    }

    /// Remove duplicates and sort cache items
    fn deduplicate_and_sort(
        &self,
        mut items: Vec<CacheItem>,
    ) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
        // Remove duplicates by path
        items.sort_by(|a, b| a.path.cmp(&b.path));
        items.dedup_by(|a, b| a.path == b.path);

        // Remove nested items (keep only top-level cache directories)
        let mut filtered_items = Vec::new();

        for item in items {
            let is_nested = filtered_items.iter().any(|existing: &CacheItem| {
                item.path.starts_with(&existing.path) && item.path != existing.path
            });

            if !is_nested {
                filtered_items.push(item);
            }
        }

        // Sort by cache type and then by path
        filtered_items.sort_by(|a, b| {
            a.cache_type
                .description()
                .cmp(b.cache_type.description())
                .then_with(|| a.path.cmp(&b.path))
        });

        Ok(filtered_items)
    }
}

/// Calculate size for cache items using parallel processing
pub fn calculate_sizes(
    items: Vec<CacheItem>,
    _max_threads: usize, // Parameter kept for API compatibility
) -> Result<Vec<CacheItem>, Box<dyn std::error::Error>> {
    let updated_items: Vec<CacheItem> = items
        .into_par_iter()
        .map(|mut item| {
            let (size, count) = calculate_directory_size(&item.path);
            item.size_bytes = Some(size);
            item.file_count = Some(count);
            item
        })
        .collect();

    Ok(updated_items)
}

/// Calculate the total size and file count of a directory
fn calculate_directory_size(path: &Path) -> (u64, usize) {
    let mut total_size = 0u64;
    let mut file_count = 0usize;

    for entry in WalkDir::new(path)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        if let Ok(metadata) = entry.metadata() {
            total_size += metadata.len();
            file_count += 1;
        }
    }

    (total_size, file_count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_cache_type_description() {
        assert_eq!(CacheType::UserCache.description(), "User cache directory");
        assert_eq!(CacheType::BrowserCache.description(), "Browser cache");
    }

    #[test]
    fn test_pattern_matching() {
        let config = Config::default();
        let detector = CacheDetector::new(config);

        assert!(detector.matches_pattern("home/user/.cache", ".cache"));
        assert!(detector.matches_pattern("home/user/.mozilla/firefox/profile/cache", "*/cache"));
        assert!(!detector.matches_pattern("home/user/documents", ".cache"));
    }

    #[test]
    fn test_cache_detection() {
        let temp_dir = TempDir::new().unwrap();
        let cache_dir = temp_dir.path().join(".cache");
        std::fs::create_dir(&cache_dir).unwrap();

        let config = Config::default();
        let detector = CacheDetector::new(config);

        let items = detector.detect_cache_items(temp_dir.path()).unwrap();
        assert!(!items.is_empty());
    }
}
