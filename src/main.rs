mod cache_detector;
mod cli;
mod config;
mod display;
mod file_operations;
mod log_cleaner;

use cache_detector::{CacheDetector, calculate_sizes};
use cli::parse_args;
use config::Config;
use display::Display;
use file_operations::FileOperations;
use log_cleaner::LogCleaner;
use std::io;
use std::process;

fn main() -> io::Result<()> {
    // Parse command line arguments
    let args = parse_args();

    // Load configuration
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(Config::default_config_path);
    let mut config = match Config::load_from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!(
                "Warning: Could not load config from {}: {}",
                config_path.display(),
                e
            );
            eprintln!("Using default configuration. A default config file will be created.");
            Config::default()
        }
    };

    // Override config with command line arguments
    if let Some(log_age_days) = args.log_age_days {
        config.log_cleanup.max_age_days = log_age_days;
    }

    if args.clean_logs {
        config.log_cleanup.enabled = true;
    }

    if args.dry_run {
        config.safety.dry_run = true;
    }

    if args.force {
        config.safety.confirm_threshold_bytes = u64::MAX; // Disable confirmation
    }

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration error: {}", e);
        process::exit(1);
    }

    // Save updated config if it was modified
    if config_path == Config::default_config_path()
        && let Err(e) = config.save_to_file(&config_path)
    {
        eprintln!("Warning: Could not save config: {}", e);
    }

    // Initialize display
    let display = Display::new(args.verbose, args.summary_only);

    // Show application header
    display.show_header();

    // Show privilege information
    display.show_privilege_info();

    // Check if scanning system-wide but not running as root
    if args.path.to_string_lossy() == "/" && unsafe { libc::getuid() != 0 } {
        println!(
            "{} Scanning system-wide without root privileges.",
            "WARNING".bold().yellow()
        );
        println!(
            "Some directories may be inaccessible. Run {} for complete access.",
            format!("sudo {} / --clean", env!("CARGO_PKG_NAME"))
                .green()
                .bold()
        );
        println!();
    }

    // Show scanning information
    let thread_count = config.effective_thread_count();
    display.show_scan_info(
        &args.path.to_string_lossy(),
        thread_count,
        config.log_cleanup.enabled,
    );

    // Initialize components
    let cache_detector = CacheDetector::new(config.clone());
    let log_cleaner = LogCleaner::new(config.clone());
    let file_ops = FileOperations::new(args.dry_run || config.safety.dry_run);

    // Detect cache items
    let mut cache_items = match cache_detector.detect_cache_items(&args.path) {
        Ok(items) => items,
        Err(e) => {
            eprintln!("Error detecting cache items: {}", e);
            process::exit(1);
        }
    };

    // Calculate cache sizes if enabled
    if args.show_sizes {
        if args.verbose {
            println!("Calculating cache sizes...");
        }
        match calculate_sizes(cache_items.clone(), thread_count) {
            Ok(updated_items) => cache_items = updated_items,
            Err(e) => eprintln!("Warning: Error calculating sizes: {}", e),
        }
    }

    // Find old log files if enabled
    let log_files = if config.log_cleanup.enabled {
        if args.verbose {
            println!("Scanning for old log files...");
        }
        match log_cleaner.find_old_log_files(&args.path) {
            Ok(logs) => logs,
            Err(e) => {
                eprintln!("Warning: Error finding log files: {}", e);
                Vec::new()
            }
        }
    } else {
        Vec::new()
    };

    // Display results
    display.show_cache_items(&cache_items);
    if config.log_cleanup.enabled {
        display.show_log_files(&log_files);
    }
    display.show_total_summary(&cache_items, &log_files, &args.path.to_string_lossy());

    // Exit if nothing to clean
    if cache_items.is_empty() && log_files.is_empty() {
        println!();
        if unsafe { libc::getuid() != 0 } && args.path.to_string_lossy() == "/" {
            println!(
                "{}",
                "Try running with sudo to access system-wide cache directories.".dimmed()
            );
        }
        return Ok(());
    }

    // Handle cleaning
    if args.clean || config.safety.dry_run {
        let total_size: u64 = cache_items
            .iter()
            .map(|i| i.size_bytes.unwrap_or(0))
            .sum::<u64>()
            + log_files.iter().map(|l| l.size_bytes).sum::<u64>();
        let total_items = cache_items.len() + log_files.len();

        // Check confirmation threshold
        if !args.force
            && !config.safety.dry_run
            && total_size > config.safety.confirm_threshold_bytes
        {
            let message = format!(
                "Are you sure you want to {} {} items totaling {}?",
                if args.dry_run {
                    "simulate cleaning"
                } else {
                    "delete"
                },
                total_items,
                file_operations::format_bytes(total_size)
            );

            if !display.prompt_confirmation(&message)? {
                println!("{}", "Operation cancelled.".yellow());
                return Ok(());
            }
        }

        // Create backup list if enabled
        if config.safety.create_backup_list
            && !args.dry_run
            && let Err(e) = file_ops.create_backup_list(&cache_items, &log_files)
        {
            eprintln!("Warning: Could not create backup list: {}", e);
        }

        println!();
        if args.dry_run || config.safety.dry_run {
            println!(
                "{}",
                "DRY RUN - Simulating cleanup operations...".cyan().bold()
            );
        } else {
            println!("{}", "Starting cleanup operations...".green().bold());
        }

        // Clean cache items
        let cache_results = if !cache_items.is_empty() {
            match file_ops.delete_cache_items(&cache_items) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Error cleaning cache items: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Clean log files
        let log_results = if !log_files.is_empty() {
            match file_ops.delete_log_files(&log_files) {
                Ok(results) => results,
                Err(e) => {
                    eprintln!("Error cleaning log files: {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        // Show results
        display.show_cleaning_results(
            &cache_results,
            &log_results,
            args.dry_run || config.safety.dry_run,
        );
    } else {
        println!();
        println!("{}", "Use --clean flag to delete these items.".dimmed());

        if unsafe { libc::getuid() != 0 } && args.path.to_string_lossy() == "/" {
            println!(
                "{}",
                format!(
                    "For system-wide cleaning, run: sudo {} / --clean",
                    env!("CARGO_PKG_NAME")
                )
                .green()
                .bold()
            );
        }

        println!();
    }

    Ok(())
}

// Import the colored trait for string coloring
use colored::*;
