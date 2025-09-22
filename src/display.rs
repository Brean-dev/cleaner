use crate::cache_detector::{CacheItem, CacheType};
use crate::file_operations::{OperationResult, OperationSummary, format_bytes, format_duration};
use crate::log_cleaner::{LogFile, LogType};
use colored::*;
use std::collections::HashMap;
use std::io::{self, Write};

/// Display utilities for formatting output
pub struct Display {
    verbose: bool,
    summary_only: bool,
}

impl Display {
    pub fn new(verbose: bool, summary_only: bool) -> Self {
        Self {
            verbose,
            summary_only,
        }
    }

    /// Display application header
    pub fn show_header(&self) {
        if self.verbose {
            println!("Version: {}", env!("CARGO_PKG_VERSION"));
            println!("Author: Brean-dev");
            println!();
        }
    }

    /// Display privilege information
    pub fn show_privilege_info(&self) {
        let is_root = unsafe { libc::getuid() == 0 };

        if is_root {
            println!(
                "{}",
                "Running with root privileges - full system access enabled."
                    .green()
                    .bold()
            );
        } else {
            println!(
                "{}",
                "Running with user privileges - limited to accessible directories.".yellow()
            );
        }
    }

    /// Display scanning information
    pub fn show_scan_info(&self, root: &str, thread_count: usize, enable_logs: bool) {
        println!(
            "Scanning: {} {}",
            root.white().bold(),
            if enable_logs {
                "(cache + logs)".dimmed()
            } else {
                "(cache only)".dimmed()
            }
        );

        if self.verbose {
            println!(
                "Using {} threads for parallel processing",
                thread_count.to_string().cyan()
            );
        }
        println!();
    }

    /// Display cache items found
    pub fn show_cache_items(&self, items: &[CacheItem]) {
        if items.is_empty() {
            println!("{}", "No cache directories found.".green());
            return;
        }

        println!(
            "{} {}",
            "FOUND".blue().bold(),
            format!("{} cache items:", items.len()).bold()
        );
        println!();

        if self.summary_only {
            self.show_cache_summary(items);
        } else {
            self.show_cache_details(items);
        }
    }

    /// Display cache summary grouped by type
    fn show_cache_summary(&self, items: &[CacheItem]) {
        let mut by_type: HashMap<CacheType, (usize, u64)> = HashMap::new();

        for item in items {
            let entry = by_type.entry(item.cache_type.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += item.size_bytes.unwrap_or(0);
        }

        for (cache_type, (count, total_size)) in by_type {
            println!(
                "  {} {} items, {}",
                cache_type.description().cyan(),
                count.to_string().yellow().bold(),
                format_bytes(total_size).red()
            );
        }
    }

    /// Display detailed cache items
    fn show_cache_details(&self, items: &[CacheItem]) {
        let mut current_type = None;

        for (i, item) in items.iter().enumerate() {
            // Group by type
            if current_type.as_ref() != Some(&item.cache_type) {
                if i > 0 {
                    println!();
                }
                println!(
                    "  {} {}:",
                    "●".cyan(),
                    item.cache_type.description().cyan().bold()
                );
                current_type = Some(item.cache_type.clone());
            }

            let size_info = if let Some(size) = item.size_bytes {
                format!(" ({})", format_bytes(size)).red()
            } else {
                " (calculating...)".dimmed()
            };

            println!(
                "    {} {}{}",
                "→".dimmed(),
                item.path.display().to_string().white(),
                size_info
            );

            if self.verbose {
                if let Some(count) = item.file_count {
                    println!(
                        "      {} {} files",
                        "•".dimmed(),
                        count.to_string().dimmed()
                    );
                }
                if let Some(modified) = item.last_modified
                    && let Ok(age) = std::time::SystemTime::now().duration_since(modified)
                {
                    println!(
                        "      {} {} old",
                        "•".dimmed(),
                        format_duration(age).dimmed()
                    );
                }
            }
        }
    }

    /// Display log files found
    pub fn show_log_files(&self, logs: &[LogFile]) {
        if logs.is_empty() {
            println!("{}", "No old log files found.".green());
            return;
        }

        println!(
            "{} {}",
            "LOG FILES".blue().bold(),
            format!("{} old log files:", logs.len()).bold()
        );
        println!();

        if self.summary_only {
            self.show_log_summary_details(logs);
        } else {
            self.show_log_details(logs);
        }
    }

    /// Display log summary
    fn show_log_summary_details(&self, logs: &[LogFile]) {
        let mut by_type: HashMap<LogType, (usize, u64)> = HashMap::new();

        for log in logs {
            let entry = by_type.entry(log.log_type.clone()).or_insert((0, 0));
            entry.0 += 1;
            entry.1 += log.size_bytes;
        }

        for (log_type, (count, total_size)) in by_type {
            println!(
                "  {} {} files, {}",
                log_type.description().cyan(),
                count.to_string().yellow().bold(),
                format_bytes(total_size).red()
            );
        }
    }

    /// Display detailed log files
    fn show_log_details(&self, logs: &[LogFile]) {
        let mut current_type = None;

        for (i, log) in logs.iter().enumerate() {
            // Group by type
            if current_type.as_ref() != Some(&log.log_type) {
                if i > 0 {
                    println!();
                }
                println!(
                    "  {} {}:",
                    "●".cyan(),
                    log.log_type.description().cyan().bold()
                );
                current_type = Some(log.log_type.clone());
            }

            println!(
                "    {} {} {} ({})",
                "→".dimmed(),
                log.path.display().to_string().white(),
                format_bytes(log.size_bytes).red(),
                format_duration(log.age).yellow()
            );

            if self.verbose {
                println!(
                    "      {} Modified: {}",
                    "•".dimmed(),
                    chrono::DateTime::<chrono::Utc>::from(log.last_modified)
                        .format("%Y-%m-%d %H:%M:%S UTC")
                        .to_string()
                        .dimmed()
                );
            }
        }
    }

    /// Display total summary
    pub fn show_total_summary(&self, cache_items: &[CacheItem], log_files: &[LogFile], root: &str) {
        let cache_size: u64 = cache_items.iter().map(|i| i.size_bytes.unwrap_or(0)).sum();
        let log_size: u64 = log_files.iter().map(|l| l.size_bytes).sum();
        let total_size = cache_size + log_size;

        println!();
        println!("{}", "SUMMARY".blue().bold());

        println!("Scan path: {}", root.green());

        if !cache_items.is_empty() {
            println!(
                "Cache items: {} ({})",
                cache_items.len().to_string().yellow().bold(),
                format_bytes(cache_size).red()
            );
        }

        if !log_files.is_empty() {
            println!(
                "Log files: {} ({})",
                log_files.len().to_string().yellow().bold(),
                format_bytes(log_size).red()
            );
        }

        println!("Total space: {}", format_bytes(total_size).red().bold());
    }

    /// Show cleaning results
    pub fn show_cleaning_results(
        &self,
        cache_results: &[OperationResult],
        log_results: &[OperationResult],
        dry_run: bool,
    ) {
        println!();
        println!(
            "{} {}",
            if dry_run {
                "DRY RUN RESULTS"
            } else {
                "CLEANING RESULTS"
            },
            "".blue().bold()
        );
        println!("{}", "━".repeat(50).dimmed());

        if !cache_results.is_empty() {
            let cache_summary = OperationSummary::from_results(cache_results);
            self.show_operation_summary("Cache Cleanup", &cache_summary, dry_run);
        }

        if !log_results.is_empty() {
            let log_summary = OperationSummary::from_results(log_results);
            self.show_operation_summary("Log Cleanup", &log_summary, dry_run);
        }

        // Combined summary
        let all_results: Vec<_> = cache_results.iter().chain(log_results.iter()).collect();
        if !all_results.is_empty() {
            let combined_summary = OperationSummary::from_results(
                &all_results.into_iter().cloned().collect::<Vec<_>>(),
            );
            println!();
            println!("{}", "TOTAL SUMMARY".green().bold());
            println!("{}", "─".repeat(30).dimmed());

            println!(
                "Items processed: {}",
                combined_summary.total_items.to_string().cyan().bold()
            );
            println!(
                "Successful: {}",
                combined_summary.successful.to_string().green().bold()
            );

            if combined_summary.failed > 0 {
                println!(
                    "Failed: {}",
                    combined_summary.failed.to_string().red().bold()
                );
            }

            if combined_summary.permission_denied > 0 {
                println!(
                    "Permission denied: {}",
                    combined_summary
                        .permission_denied
                        .to_string()
                        .yellow()
                        .bold()
                );
            }

            println!(
                "Space {}: {}",
                if dry_run {
                    "that would be freed"
                } else {
                    "freed"
                },
                format_bytes(combined_summary.total_bytes_freed)
                    .green()
                    .bold()
            );
        }
    }

    /// Show operation summary for a specific type
    fn show_operation_summary(&self, title: &str, summary: &OperationSummary, dry_run: bool) {
        println!("{} {}", "".cyan(), title.cyan().bold());
        println!(
            "  {} {}: {}",
            if dry_run {
                "Would process"
            } else {
                "Processed"
            },
            "items".dimmed(),
            summary.total_items.to_string().cyan()
        );
        println!(
            "  {} {}: {}",
            if dry_run {
                "Would succeed"
            } else {
                "Successful"
            },
            "".dimmed(),
            summary.successful.to_string().green()
        );

        if summary.failed > 0 {
            println!(
                "  {} {}: {}",
                "Failed".red(),
                "".dimmed(),
                summary.failed.to_string().red()
            );
        }

        if summary.permission_denied > 0 {
            println!(
                "  {} {}: {} {}",
                "Permission denied".yellow(),
                "".dimmed(),
                summary.permission_denied.to_string().yellow(),
                "(try sudo)".dimmed()
            );
        }

        println!(
            "  {} {}: {}",
            if dry_run { "Would free" } else { "Space freed" },
            "".dimmed(),
            format_bytes(summary.total_bytes_freed).green()
        );
    }

    /// Prompt for confirmation
    pub fn prompt_confirmation(&self, message: &str) -> io::Result<bool> {
        println!("{}", "CONFIRMATION REQUIRED".red().bold());
        print!("{} {} ", message, "[y/N]:".dimmed());
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        let response = input.trim().to_lowercase();
        Ok(matches!(response.as_str(), "y" | "yes"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache_detector::CacheType;
    use std::path::PathBuf;

    #[test]
    fn test_display_creation() {
        let display = Display::new(true, false);
        assert!(display.verbose);
        assert!(!display.summary_only);
    }

    #[test]
    fn test_cache_item_display() {
        let item = CacheItem {
            path: PathBuf::from("/tmp/test"),
            cache_type: CacheType::UserCache,
            size_bytes: Some(1024),
            file_count: Some(10),
            last_modified: None,
        };

        let display = Display::new(false, true);
        // We can't easily test the output, but we can ensure it doesn't panic
        display.show_cache_items(&[item]);
    }
}
