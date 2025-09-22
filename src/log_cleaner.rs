use crate::config::Config;
use jwalk::WalkDir;
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Represents a detected log file
#[derive(Debug, Clone)]
pub struct LogFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub last_modified: SystemTime,
    pub age: Duration,
    pub log_type: LogType,
}

/// Types of log files
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum LogType {
    System,
    Application,
    User,
    Debug,
    Error,
    Access,
    Security,
    Developer,
}

impl LogType {
    pub fn description(&self) -> &'static str {
        match self {
            LogType::System => "System log",
            LogType::Application => "Application log",
            LogType::User => "User application log",
            LogType::Debug => "Debug log",
            LogType::Error => "Error log",
            LogType::Access => "Access log",
            LogType::Security => "Security log",
            LogType::Developer => "Development log",
        }
    }
}

/// Log file detection and cleanup engine
pub struct LogCleaner {
    config: Config,
}

impl LogCleaner {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Find all log files that are older than the configured threshold
    pub fn find_old_log_files<P: AsRef<Path>>(
        &self,
        root: P,
    ) -> Result<Vec<LogFile>, Box<dyn std::error::Error>> {
        if !self.config.log_cleanup.enabled {
            return Ok(Vec::new());
        }

        let root_path = root.as_ref();
        let now = SystemTime::now();
        let age_threshold = self.config.log_age_threshold();
        let mut log_files = Vec::new();

        // Search in configured log patterns
        for pattern in &self.config.log_cleanup.log_patterns {
            log_files.extend(self.scan_log_pattern(pattern, now, age_threshold)?);
        }

        // Scan the root directory if it's not covered by patterns
        if !self.is_path_covered_by_patterns(root_path) {
            log_files.extend(self.scan_directory_for_logs(root_path, now, age_threshold)?);
        }

        // Filter and sort
        self.filter_and_sort_logs(log_files)
    }

    /// Scan a specific pattern for log files
    fn scan_log_pattern(
        &self,
        pattern: &str,
        now: SystemTime,
        age_threshold: Duration,
    ) -> Result<Vec<LogFile>, Box<dyn std::error::Error>> {
        let mut logs = Vec::new();

        // Expand ~ to home directory
        let expanded_pattern = if pattern.starts_with('~') {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            pattern.replacen('~', &home, 1)
        } else {
            pattern.to_string()
        };

        // Handle glob patterns
        if expanded_pattern.contains('*') {
            logs.extend(self.scan_glob_pattern(&expanded_pattern, now, age_threshold)?);
        } else {
            // Direct directory scan
            let path = PathBuf::from(expanded_pattern);
            if path.exists() && path.is_dir() {
                logs.extend(self.scan_directory_for_logs(&path, now, age_threshold)?);
            }
        }

        Ok(logs)
    }

    /// Scan using glob patterns
    fn scan_glob_pattern(
        &self,
        pattern: &str,
        now: SystemTime,
        age_threshold: Duration,
    ) -> Result<Vec<LogFile>, Box<dyn std::error::Error>> {
        use glob::glob;
        let mut logs = Vec::new();

        for entry in glob(pattern)? {
            match entry {
                Ok(path) => {
                    if path.is_file() {
                        if let Some(log_file) = self.check_log_file(&path, now, age_threshold)? {
                            logs.push(log_file);
                        }
                    } else if path.is_dir() {
                        logs.extend(self.scan_directory_for_logs(&path, now, age_threshold)?);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Error processing glob pattern {}: {}", pattern, e);
                }
            }
        }

        Ok(logs)
    }

    /// Scan a directory for log files using parallel processing
    fn scan_directory_for_logs(
        &self,
        dir: &Path,
        now: SystemTime,
        age_threshold: Duration,
    ) -> Result<Vec<LogFile>, Box<dyn std::error::Error>> {
        if self.config.is_excluded_path(dir) {
            return Ok(Vec::new());
        }

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
        let entries: Result<Vec<_>, _> = WalkDir::new(dir)
            .parallelism(parallelism)
            .max_depth(self.config.performance.max_depth.unwrap_or(10))
            .follow_links(!self.config.performance.skip_symlinks)
            .into_iter()
            .filter_map(|entry_result| match entry_result {
                Ok(entry) => {
                    if entry.file_type().is_file() {
                        Some(Ok(entry))
                    } else {
                        None
                    }
                }
                Err(e) => Some(Err(e)),
            })
            .collect();

        let entries = entries?;

        // Use rayon for parallel processing of file classification
        let logs: Result<Vec<_>, _> = entries
            .into_par_iter()
            .filter_map(
                |entry| match self.check_log_file(&entry.path(), now, age_threshold) {
                    Ok(Some(log_file)) => Some(Ok(log_file)),
                    Ok(None) => None,
                    Err(e) => Some(Err(format!("Error checking log file: {}", e))),
                },
            )
            .collect();

        match logs {
            Ok(log_files) => Ok(log_files),
            Err(e) => Err(e.into()),
        }
    }

    /// Check if a file is a log file and meets age criteria
    fn check_log_file(
        &self,
        path: &Path,
        now: SystemTime,
        age_threshold: Duration,
    ) -> Result<Option<LogFile>, Box<dyn std::error::Error>> {
        // Check if it's a log file by extension
        if !self.is_log_file(path) {
            return Ok(None);
        }

        // Check if path is excluded
        if self.config.is_excluded_path(path) {
            return Ok(None);
        }

        // Get file metadata
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(_) => return Ok(None), // Skip files we can't read
        };

        // Check minimum size
        if metadata.len() < self.config.log_cleanup.min_size_bytes {
            return Ok(None);
        }

        // Check age
        let modified = metadata.modified()?;
        let age = now
            .duration_since(modified)
            .unwrap_or(Duration::from_secs(0));

        if age < age_threshold {
            return Ok(None);
        }

        // Classify log type
        let log_type = self.classify_log_file(path);

        Ok(Some(LogFile {
            path: path.to_path_buf(),
            size_bytes: metadata.len(),
            last_modified: modified,
            age,
            log_type,
        }))
    }

    /// Check if a file is a log file based on extension and location
    fn is_log_file(&self, path: &Path) -> bool {
        // Check extension
        if let Some(extension) = path.extension() {
            let ext_str = extension.to_string_lossy().to_lowercase();
            if self
                .config
                .log_cleanup
                .log_extensions
                .iter()
                .any(|e| e.to_lowercase() == ext_str)
            {
                return true;
            }
        }

        // Check filename patterns
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        // Common log file patterns
        if filename.contains("log")
            || filename.ends_with(".log")
            || filename.ends_with(".out")
            || filename.ends_with(".err")
            || filename.contains("debug")
            || filename.contains("trace")
            || filename.contains("audit")
        {
            // Don't treat files with "logger" in the name as log files
            // Don't treat source code files as log files
            if let Some(extension) = path.extension() {
                let ext_str = extension.to_string_lossy().to_lowercase();
                let code_extensions = [
                    "rs", "go", "js", "ts", "py", "java", "cpp", "c", "h", "hpp", "cs", "php",
                    "rb", "swift", "kt", "scala", "clj", "hs", "ml", "fs", "vb", "pl", "sh", "ps1",
                    "bat",
                ];
                if code_extensions.contains(&ext_str.as_str()) {
                    return false;
                }
            }

            // Don't treat files with "logger" in the name as log files
            if filename.contains("logger") {
                return false;
            }
            return true;
        }

        // Check if it's in a logs directory
        if let Some(parent) = path.parent() {
            let parent_name = parent
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if parent_name.contains("log") {
                return true;
            }
        }

        false
    }

    /// Classify the type of log file
    fn classify_log_file(&self, path: &Path) -> LogType {
        let path_str = path.to_string_lossy().to_lowercase();
        let filename = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();

        // System logs
        if path_str.starts_with("/var/log")
            || path_str.contains("/syslog")
            || filename.contains("kern")
            || filename.contains("auth")
            || filename.contains("mail")
            || filename.contains("cron")
        {
            return LogType::System;
        }

        // Security logs
        if filename.contains("auth")
            || filename.contains("security")
            || filename.contains("audit")
            || filename.contains("access")
        {
            return LogType::Security;
        }

        // Error logs
        if filename.contains("error") || filename.contains("err") || filename.contains("exception")
        {
            return LogType::Error;
        }

        // Debug logs
        if filename.contains("debug") || filename.contains("trace") || filename.contains("verbose")
        {
            return LogType::Debug;
        }

        // Access logs
        if filename.contains("access") || filename.contains("request") || filename.contains("http")
        {
            return LogType::Access;
        }

        // User logs
        if path_str.contains("/home/")
            || path_str.contains("/.config/")
            || path_str.contains("/.local/")
        {
            return LogType::User;
        }

        // Development logs
        if path_str.contains("node_modules")
            || path_str.contains("target/")
            || path_str.contains("build/")
            || path_str.contains(".git/")
            || filename.contains("npm")
            || filename.contains("cargo")
            || filename.contains("gradle")
        {
            return LogType::Developer;
        }

        // Default to application log
        LogType::Application
    }

    /// Check if a path is covered by the configured log patterns
    fn is_path_covered_by_patterns(&self, path: &Path) -> bool {
        let path_str = path.to_string_lossy();

        for pattern in &self.config.log_cleanup.log_patterns {
            let expanded_pattern = if pattern.starts_with('~') {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                pattern.replacen('~', &home, 1)
            } else {
                pattern.to_string()
            };

            if path_str.starts_with(&expanded_pattern) {
                return true;
            }
        }

        false
    }

    /// Filter and sort log files
    fn filter_and_sort_logs(
        &self,
        mut logs: Vec<LogFile>,
    ) -> Result<Vec<LogFile>, Box<dyn std::error::Error>> {
        // Remove duplicates
        logs.sort_by(|a, b| a.path.cmp(&b.path));
        logs.dedup_by(|a, b| a.path == b.path);

        // Sort by age (oldest first) and then by size (largest first)
        logs.sort_by(|a, b| {
            b.age
                .cmp(&a.age)
                .then_with(|| b.size_bytes.cmp(&a.size_bytes))
        });

        Ok(logs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_type_description() {
        assert_eq!(LogType::System.description(), "System log");
        assert_eq!(LogType::Error.description(), "Error log");
    }

    #[test]
    fn test_is_log_file() {
        let config = Config::default();
        let cleaner = LogCleaner::new(config);

        assert!(cleaner.is_log_file(Path::new("test.log")));
        assert!(cleaner.is_log_file(Path::new("application.out")));
        assert!(cleaner.is_log_file(Path::new("debug.trace")));
        assert!(!cleaner.is_log_file(Path::new("test.txt")));
    }

    #[test]
    fn test_classify_log_file() {
        let config = Config::default();
        let cleaner = LogCleaner::new(config);

        assert_eq!(
            cleaner.classify_log_file(Path::new("/var/log/syslog")),
            LogType::System
        );
        assert_eq!(
            cleaner.classify_log_file(Path::new("error.log")),
            LogType::Error
        );
        assert_eq!(
            cleaner.classify_log_file(Path::new("debug.log")),
            LogType::Debug
        );
        assert_eq!(
            cleaner.classify_log_file(Path::new("/home/user/app.log")),
            LogType::User
        );
    }

    #[test]
    fn test_log_detection() {
        let temp_dir = TempDir::new().unwrap();
        let log_file = temp_dir.path().join("test.log");
        std::fs::write(&log_file, "test log content").unwrap();

        let config = Config::default();
        let cleaner = LogCleaner::new(config);

        // This test would need the log file to be old enough, so we can't easily test the actual detection
        // But we can test that the method doesn't crash
        let result = cleaner.find_old_log_files(temp_dir.path());
        assert!(result.is_ok());
    }
}
