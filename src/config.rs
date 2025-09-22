use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuration for the cache cleaner
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    /// Cache directory patterns to detect
    pub cache_patterns: CachePatterns,
    /// Log cleanup configuration
    pub log_cleanup: LogCleanupConfig,
    /// Safety settings
    pub safety: SafetyConfig,
    /// Performance settings
    pub performance: PerformanceConfig,
}

/// Comprehensive cache detection patterns
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachePatterns {
    /// User-level cache directories (under $HOME)
    pub user_cache_dirs: Vec<String>,
    /// System-wide cache directories
    pub system_cache_dirs: Vec<String>,
    /// Application-specific cache patterns
    pub app_cache_patterns: Vec<String>,
    /// Package manager cache directories
    pub package_manager_caches: Vec<String>,
    /// Development tool caches
    pub dev_tool_caches: Vec<String>,
    /// Browser cache patterns
    pub browser_caches: Vec<String>,
    /// Temporary directory patterns
    pub temp_patterns: Vec<String>,
    /// Build artifact patterns
    pub build_artifacts: Vec<String>,
}

/// Log file cleanup configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogCleanupConfig {
    /// Enable log cleanup
    pub enabled: bool,
    /// Maximum age for log files (in days)
    pub max_age_days: u64,
    /// Log directory patterns to search
    pub log_patterns: Vec<String>,
    /// Log file extensions to consider
    pub log_extensions: Vec<String>,
    /// Minimum size threshold for log files (in bytes)
    pub min_size_bytes: u64,
}

/// Safety configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SafetyConfig {
    /// Directories to always exclude from cleaning
    pub exclude_paths: Vec<String>,
    /// Require confirmation for large deletions (in bytes)
    pub confirm_threshold_bytes: u64,
    /// Maximum number of files to delete in one operation
    pub max_files_per_operation: usize,
    /// Dry run mode (show what would be deleted without deleting)
    pub dry_run: bool,
    /// Create backup list before deletion
    pub create_backup_list: bool,
}

/// Performance configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// Maximum number of threads to use
    pub max_threads: Option<usize>,
    /// Timeout for directory access (in seconds)
    pub access_timeout_secs: u64,
    /// Skip symbolic links
    pub skip_symlinks: bool,
    /// Maximum depth for directory traversal
    pub max_depth: Option<usize>,
}

impl Default for CachePatterns {
    fn default() -> Self {
        Self {
            // XDG Base Directory compliant user cache directories
            user_cache_dirs: vec![
                ".cache".to_string(),
                ".local/share/Trash".to_string(),
                ".thumbnails".to_string(),
                ".mozilla/firefox/*/Cache".to_string(),
                ".config/google-chrome/*/Cache".to_string(),
                ".config/chromium/*/Cache".to_string(),
                ".vscode/CachedExtensions".to_string(),
                ".vscode/logs".to_string(),
            ],

            // System-wide cache directories
            system_cache_dirs: vec![
                "/var/cache".to_string(),
                "/var/tmp".to_string(),
                "/tmp".to_string(),
                "/var/lib/apt/lists".to_string(),
                "/var/cache/apt".to_string(),
                "/var/cache/fontconfig".to_string(),
                "/var/cache/man".to_string(),
            ],

            // Application-specific patterns
            app_cache_patterns: vec![
                "*/.cache".to_string(),
                "*/cache".to_string(),
                "*/Cache".to_string(),
                "*/.thumbnails".to_string(),
                "*/thumbnails".to_string(),
            ],

            // Package manager caches
            package_manager_caches: vec![
                "/var/cache/pacman/pkg".to_string(),   // Arch Linux
                "/var/cache/apt/archives".to_string(), // Debian/Ubuntu
                "/var/cache/yum".to_string(),          // RHEL/CentOS
                "/var/cache/dnf".to_string(),          // Fedora
                "/var/cache/zypper".to_string(),       // openSUSE
                "~/.cache/pip".to_string(),            // Python pip
                "~/.npm/_cacache".to_string(),         // Node.js npm
                "~/.cargo/registry/cache".to_string(), // Rust cargo
                "~/.gradle/caches".to_string(),        // Gradle
                "~/.m2/repository".to_string(),        // Maven
            ],

            // Development tool caches
            dev_tool_caches: vec![
                "node_modules/.cache".to_string(),
                "target/debug".to_string(), // Rust debug builds
                "build".to_string(),
                "dist".to_string(),
                ".pytest_cache".to_string(),
                "__pycache__".to_string(),
                ".mypy_cache".to_string(),
                ".tox".to_string(),
                ".coverage".to_string(),
            ],

            // Browser caches
            browser_caches: vec![
                ".mozilla/firefox/*/cache2".to_string(),
                ".config/google-chrome/*/Cache".to_string(),
                ".config/chromium/*/Cache".to_string(),
                ".opera/cache".to_string(),
                ".config/BraveSoftware/*/Cache".to_string(),
            ],

            // Temporary patterns
            temp_patterns: vec![
                "tmp".to_string(),
                "temp".to_string(),
                "temporary".to_string(),
                ".tmp".to_string(),
                ".temp".to_string(),
            ],

            // Build artifacts
            build_artifacts: vec![
                "*.o".to_string(),
                "*.so".to_string(),
                "*.a".to_string(),
                "*.pyc".to_string(),
                "*.pyo".to_string(),
                "*.class".to_string(),
                "*.dSYM".to_string(),
            ],
        }
    }
}

impl Default for LogCleanupConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_age_days: 7, // 1 week as requested
            log_patterns: vec![
                "/var/log".to_string(),
                "~/.local/share/*/logs".to_string(),
                "~/.config/*/logs".to_string(),
                "/tmp/*.log".to_string(),
                "/var/tmp/*.log".to_string(),
            ],
            log_extensions: vec![
                "log".to_string(),
                "LOG".to_string(),
                "logs".to_string(),
                "out".to_string(),
                "err".to_string(),
                "debug".to_string(),
                "trace".to_string(),
            ],
            min_size_bytes: 1024, // Only clean logs > 1KB
        }
    }
}

impl Default for SafetyConfig {
    fn default() -> Self {
        Self {
            exclude_paths: vec![
                "/.git".to_string(),
                "/.svn".to_string(),
                "/.hg".to_string(),
                "/proc".to_string(),
                "/sys".to_string(),
                "/dev".to_string(),
                "/run".to_string(),
                "/boot".to_string(),
                "/etc".to_string(),
                "/usr".to_string(),
                "/lib".to_string(),
                "/lib64".to_string(),
                "/bin".to_string(),
                "/sbin".to_string(),
            ],
            confirm_threshold_bytes: 100 * 1024 * 1024, // 100MB
            max_files_per_operation: 10000,
            dry_run: false,
            create_backup_list: true,
        }
    }
}

impl Default for PerformanceConfig {
    fn default() -> Self {
        Self {
            max_threads: None, // Use system default
            access_timeout_secs: 5,
            skip_symlinks: true,
            max_depth: Some(10), // Reasonable depth limit
        }
    }
}

impl Config {
    /// Load configuration from file, falling back to default if not found
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let path = path.as_ref();

        if !path.exists() {
            // Create default config file
            let default_config = Self::default();
            default_config.save_to_file(path)?;
            return Ok(default_config);
        }

        let content = fs::read_to_string(path)?;
        let config: Self = toml::from_str(&content)?;
        Ok(config)
    }

    /// Save configuration to file
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn std::error::Error>> {
        let path = path.as_ref();

        // Create parent directories if they don't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    /// Get the default config file path (XDG compliant)
    pub fn default_config_path() -> PathBuf {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            format!("{}/.config", home)
        });

        PathBuf::from(config_home)
            .join("cleaner")
            .join("config.toml")
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.log_cleanup.max_age_days == 0 {
            return Err("Log max age cannot be zero".to_string());
        }

        if self.safety.max_files_per_operation == 0 {
            return Err("Max files per operation cannot be zero".to_string());
        }

        if let Some(max_threads) = self.performance.max_threads
            && max_threads == 0
        {
            return Err("Max threads cannot be zero".to_string());
        }

        if let Some(max_depth) = self.performance.max_depth
            && max_depth == 0
        {
            return Err("Max depth cannot be zero".to_string());
        }

        Ok(())
    }

    /// Get log file age threshold as Duration
    pub fn log_age_threshold(&self) -> Duration {
        Duration::from_secs(self.log_cleanup.max_age_days * 24 * 60 * 60)
    }

    /// Check if a path should be excluded from cleaning
    pub fn is_excluded_path(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for exclude_pattern in &self.safety.exclude_paths {
            if path_str.contains(exclude_pattern) {
                return true;
            }
        }

        false
    }

    /// Get effective thread count
    pub fn effective_thread_count(&self) -> usize {
        self.performance.max_threads.unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4)
                .min(8) // Cap at 8 threads to avoid overwhelming the system
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert!(config.validate().is_ok());
        assert!(!config.cache_patterns.user_cache_dirs.is_empty());
        assert!(config.log_cleanup.enabled);
        assert_eq!(config.log_cleanup.max_age_days, 7);
    }

    #[test]
    fn test_config_serialization() {
        let config = Config::default();
        let toml_str = toml::to_string(&config).unwrap();
        let deserialized: Config = toml::from_str(&toml_str).unwrap();
        assert_eq!(
            config.log_cleanup.max_age_days,
            deserialized.log_cleanup.max_age_days
        );
    }
}
#[test]
fn test_config_serialization() {
    let config = Config::default();
    let toml_str = toml::to_string(&config).unwrap();
    let deserialized: Config = toml::from_str(&toml_str).unwrap();
    assert_eq!(
        config.log_cleanup.max_age_days,
        deserialized.log_cleanup.max_age_days
    );
}
