use crate::cache_detector::CacheItem;
use crate::log_cleaner::LogFile;
use rayon::prelude::*;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Result of a file operation
#[derive(Debug, Clone)]
pub struct OperationResult {
    pub success: bool,
    pub error: Option<String>,
    pub bytes_freed: u64,
}

/// File operations manager
pub struct FileOperations {
    dry_run: bool,
}

impl FileOperations {
    pub fn new(dry_run: bool) -> Self {
        Self { dry_run }
    }

    /// Delete cache items with parallel processing
    pub fn delete_cache_items(
        &self,
        items: &[CacheItem],
    ) -> Result<Vec<OperationResult>, Box<dyn std::error::Error>> {
        if items.is_empty() {
            return Ok(Vec::new());
        }

        println!("Starting cleanup of {} cache items...", items.len());

        let total = items.len();
        let dry_run = self.dry_run;

        // Use rayon for parallel processing
        let results: Vec<OperationResult> = items
            .par_iter()
            .enumerate()
            .map(|(index, item)| {
                // Show progress with less frequent updates to avoid overwhelming output
                if index % 10 == 0 || index == total - 1 {
                    print!(
                        "  {} {} [{}/{}] ",
                        if dry_run { "DRY RUN" } else { "DELETING" },
                        item.path.display(),
                        index + 1,
                        total
                    );
                    io::stdout().flush().ok();
                }

                let result = if dry_run {
                    Self::simulate_deletion(item)
                } else {
                    Self::perform_deletion(item)
                };

                match &result {
                    Ok(op_result) => {
                        if op_result.success && (index % 10 == 0 || index == total - 1) {
                            println!(" SUCCESS ({})", format_bytes(op_result.bytes_freed));
                        } else if !op_result.success && (index % 10 == 0 || index == total - 1) {
                            println!(
                                " FAILED: {}",
                                op_result
                                    .error
                                    .as_ref()
                                    .unwrap_or(&"Unknown error".to_string())
                            );
                        }
                    }
                    Err(e) => {
                        if index % 10 == 0 || index == total - 1 {
                            println!(" ERROR: {}", e);
                        }
                    }
                }

                result.unwrap_or_else(|e| OperationResult {
                    success: false,
                    error: Some(e.to_string()),
                    bytes_freed: 0,
                })
            })
            .collect();

        Ok(results)
    }

    /// Delete log files with parallel processing
    pub fn delete_log_files(
        &self,
        logs: &[LogFile],
    ) -> Result<Vec<OperationResult>, Box<dyn std::error::Error>> {
        if logs.is_empty() {
            return Ok(Vec::new());
        }

        println!("Starting cleanup of {} log files...", logs.len());

        let total = logs.len();
        let dry_run = self.dry_run;

        // Use rayon for parallel processing
        let results: Vec<OperationResult> = logs
            .par_iter()
            .enumerate()
            .map(|(index, log)| {
                // Show progress with less frequent updates to avoid overwhelming output
                if index % 10 == 0 || index == total - 1 {
                    print!(
                        "  {} {} [{}/{}] ",
                        if dry_run { "DRY RUN" } else { "DELETING" },
                        log.path.display(),
                        index + 1,
                        total
                    );
                    io::stdout().flush().ok();
                }

                let result = if dry_run {
                    Self::simulate_log_deletion(log)
                } else {
                    Self::perform_log_deletion(log)
                };

                match &result {
                    Ok(op_result) => {
                        if op_result.success && (index % 10 == 0 || index == total - 1) {
                            println!(" SUCCESS ({})", format_bytes(op_result.bytes_freed));
                        } else if !op_result.success && (index % 10 == 0 || index == total - 1) {
                            println!(
                                " FAILED: {}",
                                op_result
                                    .error
                                    .as_ref()
                                    .unwrap_or(&"Unknown error".to_string())
                            );
                        }
                    }
                    Err(e) => {
                        if index % 10 == 0 || index == total - 1 {
                            println!(" ERROR: {}", e);
                        }
                    }
                }

                result.unwrap_or_else(|e| OperationResult {
                    success: false,
                    error: Some(e.to_string()),
                    bytes_freed: 0,
                })
            })
            .collect();

        Ok(results)
    }

    /// Simulate deletion of a cache item (dry run)
    fn simulate_deletion(item: &CacheItem) -> Result<OperationResult, Box<dyn std::error::Error>> {
        // Check if we can read the item
        if !item.path.exists() {
            return Ok(OperationResult {
                success: false,
                error: Some("Path does not exist".to_string()),
                bytes_freed: 0,
            });
        }

        let size = item.size_bytes.unwrap_or(0);

        Ok(OperationResult {
            success: true,
            error: None,
            bytes_freed: size,
        })
    }

    /// Perform actual deletion of a cache item
    fn perform_deletion(item: &CacheItem) -> Result<OperationResult, Box<dyn std::error::Error>> {
        let size = item.size_bytes.unwrap_or(0);

        // Check if path exists
        if !item.path.exists() {
            return Ok(OperationResult {
                success: false,
                error: Some("Path does not exist".to_string()),
                bytes_freed: 0,
            });
        }

        // Check permissions
        if !Self::is_deletable(&item.path)? {
            return Ok(OperationResult {
                success: false,
                error: Some("Permission denied".to_string()),
                bytes_freed: 0,
            });
        }

        // Perform deletion
        let result = if item.path.is_dir() {
            fs::remove_dir_all(&item.path)
        } else {
            fs::remove_file(&item.path)
        };

        match result {
            Ok(()) => Ok(OperationResult {
                success: true,
                error: None,
                bytes_freed: size,
            }),
            Err(e) => Ok(OperationResult {
                success: false,
                error: Some(e.to_string()),
                bytes_freed: 0,
            }),
        }
    }

    /// Simulate deletion of a log file (dry run)
    fn simulate_log_deletion(log: &LogFile) -> Result<OperationResult, Box<dyn std::error::Error>> {
        if !log.path.exists() {
            return Ok(OperationResult {
                success: false,
                error: Some("File does not exist".to_string()),
                bytes_freed: 0,
            });
        }

        Ok(OperationResult {
            success: true,
            error: None,
            bytes_freed: log.size_bytes,
        })
    }

    /// Perform actual deletion of a log file
    fn perform_log_deletion(log: &LogFile) -> Result<OperationResult, Box<dyn std::error::Error>> {
        // Check if file exists
        if !log.path.exists() {
            return Ok(OperationResult {
                success: false,
                error: Some("File does not exist".to_string()),
                bytes_freed: 0,
            });
        }

        // Check permissions
        if !Self::is_deletable(&log.path)? {
            return Ok(OperationResult {
                success: false,
                error: Some("Permission denied".to_string()),
                bytes_freed: 0,
            });
        }

        // Perform deletion
        match fs::remove_file(&log.path) {
            Ok(()) => Ok(OperationResult {
                success: true,
                error: None,
                bytes_freed: log.size_bytes,
            }),
            Err(e) => Ok(OperationResult {
                success: false,
                error: Some(e.to_string()),
                bytes_freed: 0,
            }),
        }
    }

    /// Check if a path can be deleted
    fn is_deletable(path: &Path) -> Result<bool, Box<dyn std::error::Error>> {
        // Try to access the parent directory
        if let Some(parent) = path.parent() {
            match fs::read_dir(parent) {
                Ok(_) => Ok(true),
                Err(e) => {
                    if e.kind() == io::ErrorKind::PermissionDenied {
                        Ok(false)
                    } else {
                        Ok(true) // Other errors might be temporary
                    }
                }
            }
        } else {
            Ok(false) // Can't delete root
        }
    }

    /// Create a backup list of items before deletion
    pub fn create_backup_list(
        &self,
        cache_items: &[CacheItem],
        log_files: &[LogFile],
    ) -> Result<(), Box<dyn std::error::Error>> {
        let backup_file = Self::get_backup_file_path()?;

        // Create backup directory if it doesn't exist
        if let Some(parent) = backup_file.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut content = String::new();
        content.push_str(&format!(
            "# Cleaner Backup List - {}\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));
        content.push_str("# This file contains a list of items that were cleaned\n\n");

        if !cache_items.is_empty() {
            content.push_str("## Cache Items\n");
            for item in cache_items {
                content.push_str(&format!(
                    "{} # {} - {}\n",
                    item.path.display(),
                    item.cache_type.description(),
                    item.size_bytes
                        .map(format_bytes)
                        .unwrap_or_else(|| "Unknown size".to_string())
                ));
            }
            content.push('\n');
        }

        if !log_files.is_empty() {
            content.push_str("## Log Files\n");
            for log in log_files {
                content.push_str(&format!(
                    "{} # {} - {} - {} old\n",
                    log.path.display(),
                    log.log_type.description(),
                    format_bytes(log.size_bytes),
                    format_duration(log.age)
                ));
            }
        }

        fs::write(&backup_file, content)?;
        println!("Backup list created: {}", backup_file.display());

        Ok(())
    }

    /// Get the backup file path
    fn get_backup_file_path() -> Result<std::path::PathBuf, Box<dyn std::error::Error>> {
        let config_home = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            format!("{}/.config", home)
        });

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        Ok(std::path::PathBuf::from(config_home)
            .join("cleaner")
            .join("backups")
            .join(format!("cleanup_{}.txt", timestamp)))
    }
}

/// Summary of operation results
#[derive(Debug)]
pub struct OperationSummary {
    pub total_items: usize,
    pub successful: usize,
    pub failed: usize,
    pub total_bytes_freed: u64,
    pub permission_denied: usize,
}

impl OperationSummary {
    pub fn from_results(results: &[OperationResult]) -> Self {
        let total_items = results.len();
        let successful = results.iter().filter(|r| r.success).count();
        let failed = total_items - successful;
        let total_bytes_freed = results.iter().map(|r| r.bytes_freed).sum();
        let permission_denied = results
            .iter()
            .filter(|r| {
                !r.success
                    && r.error
                        .as_ref()
                        .is_some_and(|e| e.contains("Permission denied"))
            })
            .count();

        Self {
            total_items,
            successful,
            failed,
            total_bytes_freed,
            permission_denied,
        }
    }
}

/// Format bytes into human-readable format
pub fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    const THRESHOLD: f64 = 1024.0;

    if bytes == 0 {
        return "0 B".to_string();
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;

    while unit_index < UNITS.len() - 1 && size >= THRESHOLD {
        size /= THRESHOLD;
        unit_index += 1;
    }

    format!("{:.2} {}", size, UNITS[unit_index])
}

/// Format duration into human-readable format
pub fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let days = total_seconds / (24 * 60 * 60);
    let hours = (total_seconds % (24 * 60 * 60)) / (60 * 60);
    let minutes = (total_seconds % (60 * 60)) / 60;

    if days > 0 {
        format!("{}d {}h", days, hours)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(512), "512.00 B");
        assert_eq!(format_bytes(1024), "1.00 KB");
        assert_eq!(format_bytes(1048576), "1.00 MB");
    }

    #[test]
    fn test_format_duration() {
        use std::time::Duration;

        assert_eq!(format_duration(Duration::from_secs(60)), "1m");
        assert_eq!(format_duration(Duration::from_secs(3600)), "1h 0m");
        assert_eq!(format_duration(Duration::from_secs(86400)), "1d 0h");
    }

    #[test]
    fn test_operation_summary() {
        let results = vec![
            OperationResult {
                success: true,
                error: None,
                bytes_freed: 1024,
            },
            OperationResult {
                success: false,
                error: Some("Permission denied".to_string()),
                bytes_freed: 0,
            },
        ];

        let summary = OperationSummary::from_results(&results);
        assert_eq!(summary.total_items, 2);
        assert_eq!(summary.successful, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.total_bytes_freed, 1024);
        assert_eq!(summary.permission_denied, 1);
    }
}
