use clap::{Arg, Command};
use colored::*;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
};
use walkdir::{DirEntry, WalkDir};

// Version information
const VERSION: &str = env!("CARGO_PKG_VERSION");
const PKG_NAME: &str = env!("CARGO_PKG_NAME");

/// Build command line interface
fn build_cli() -> Command {
    Command::new(PKG_NAME)
        .version(VERSION)
        .about("A fast parallel cache directory cleaner")
        .author("Brean-dev")
        .arg(
            Arg::new("path")
                .help("Root path to scan for cache directories")
                .default_value("/")
                .index(1),
        )
        .arg(
            Arg::new("clean")
                .long("clean")
                .help("Actually delete the found cache directories")
                .action(clap::ArgAction::SetTrue),
        )
}

/// Check if running with root privileges
fn check_root_privileges() -> bool {
    // Check if running as root (UID 0)
    unsafe { libc::getuid() == 0 }
}

/// Check if a directory entry contains cache-related patterns in its path
fn has_cache_in_path(entry: &DirEntry) -> bool {
    const CACHE_PATTERNS: &[&str] = &[".cache", "tmp", "temp"];

    // Check if it's a directory first
    if !entry.file_type().is_dir() {
        return false;
    }

    // Get path components and check if any match our cache patterns exactly
    entry
        .path()
        .components()
        .filter_map(|comp| comp.as_os_str().to_str())
        .any(|component| CACHE_PATTERNS.contains(&component))
}

/// Try to access a directory and return if it's accessible
fn is_dir_accessible(path: &Path) -> bool {
    match fs::read_dir(path) {
        Ok(_) => true,
        Err(e) => {
            if e.kind() == io::ErrorKind::PermissionDenied {
                false
            } else {
                true // Other errors might be temporary, so we consider it accessible
            }
        }
    }
}

/// Collect all cache directories under the given root path using multiple threads
fn collect_cache_dirs<P: AsRef<Path>>(root: P) -> Vec<PathBuf> {
    let root_path = root.as_ref().to_path_buf();

    // Get available parallelism for optimal thread count
    let thread_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8); // Cap at 8 threads to avoid overwhelming the system

    // Collect top-level directories first
    let top_level_dirs: Vec<PathBuf> = fs::read_dir(&root_path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false)
                        && is_dir_accessible(&entry.path())
                })
                .map(|entry| entry.path())
                .collect()
        })
        .unwrap_or_default();

    if top_level_dirs.is_empty() {
        return Vec::new();
    }

    // Shared result collection using Arc<Mutex<Vec<PathBuf>>>
    let results = Arc::new(Mutex::new(Vec::new()));
    let inaccessible_dirs = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::new();

    // Distribute directories among threads
    let chunk_size = top_level_dirs.len().div_ceil(thread_count);

    for chunk in top_level_dirs.chunks(chunk_size) {
        let chunk_dirs = chunk.to_vec();
        let results_clone = Arc::clone(&results);
        let inaccessible_clone = Arc::clone(&inaccessible_dirs);

        let handle = thread::spawn(move || {
            let mut local_results = Vec::new();
            let mut local_inaccessible = Vec::new();

            for dir in chunk_dirs {
                // Walk each directory and collect cache dirs
                for entry in WalkDir::new(&dir)
                    .min_depth(1)
                    .into_iter()
                    .filter_map(|e| match e {
                        Ok(entry) => Some(entry),
                        Err(err) => {
                            // Log permission errors but continue
                            if err.io_error().map(|e| e.kind())
                                == Some(io::ErrorKind::PermissionDenied)
                                && let Some(path) = err.path()
                            {
                                local_inaccessible.push(path.to_path_buf());
                            }
                            None
                        }
                    })
                    .filter(has_cache_in_path)
                {
                    local_results.push(entry.into_path());
                }
            }

            // Lock and merge results
            if let Ok(mut global_results) = results_clone.lock() {
                global_results.extend(local_results);
            }

            if let Ok(mut global_inaccessible) = inaccessible_clone.lock() {
                global_inaccessible.extend(local_inaccessible);
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Thread panicked: {:?}", e);
        }
    }

    // Show permission warnings if not running as root
    if !check_root_privileges() {
        let inaccessible = Arc::try_unwrap(inaccessible_dirs)
            .unwrap_or_else(|_| panic!("Failed to unwrap inaccessible_dirs"))
            .into_inner()
            .unwrap_or_else(|_| panic!("Failed to acquire mutex"));

        if !inaccessible.is_empty() {
            println!(
                "\n{} {} directories were inaccessible due to permission restrictions:",
                "WARNING".bold().yellow(),
                inaccessible.len()
            );
            for dir in inaccessible.iter().take(5) {
                println!("  {}", dir.display().to_string().dimmed());
            }
            if inaccessible.len() > 5 {
                println!("  {} ({} more...)", "...".dimmed(), inaccessible.len() - 5);
            }
            println!(
                "{} Run with {} to access all directories.",
                "TIP:".bold().blue(),
                "sudo".green().bold()
            );
        }
    }

    // Extract final results
    Arc::try_unwrap(results)
        .unwrap_or_else(|_| panic!("Failed to unwrap results"))
        .into_inner()
        .unwrap_or_else(|_| panic!("Failed to acquire mutex"))
}

/// Filter to keep only top-level cache directories (not nested inside others)
fn top_level_cache_dirs(mut dirs: Vec<PathBuf>) -> Vec<PathBuf> {
    // Sort by path length for efficient parent checking
    dirs.sort_by_key(|path| path.as_os_str().len());

    let mut top_level = Vec::new();

    for dir in dirs {
        let is_nested = top_level
            .iter()
            .any(|parent: &PathBuf| dir.starts_with(parent) && dir != *parent);

        if !is_nested {
            top_level.push(dir);
        }
    }

    top_level
}

/// Calculate total size of files in the given paths using parallel processing
fn total_size<P: AsRef<Path>>(paths: &[P]) -> u64 {
    if paths.is_empty() {
        return 0;
    }

    let thread_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(paths.len().max(1));

    let total_size = Arc::new(Mutex::new(0u64));
    let mut handles = Vec::new();

    // Distribute paths among threads
    let chunk_size = paths.len().div_ceil(thread_count);

    for chunk in paths.chunks(chunk_size) {
        let chunk_paths: Vec<PathBuf> = chunk.iter().map(|p| p.as_ref().to_path_buf()).collect();
        let total_size_clone = Arc::clone(&total_size);

        let handle = thread::spawn(move || {
            let mut local_size = 0u64;

            for path in chunk_paths {
                for entry in WalkDir::new(path)
                    .into_iter()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_file())
                {
                    if let Ok(metadata) = entry.metadata() {
                        local_size += metadata.len();
                    }
                }
            }

            // Add to global total
            if let Ok(mut total) = total_size_clone.lock() {
                *total += local_size;
            }
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Size calculation thread panicked: {:?}", e);
        }
    }

    // Return final result
    Arc::try_unwrap(total_size)
        .unwrap_or_else(|_| panic!("Failed to unwrap total_size"))
        .into_inner()
        .unwrap_or_else(|_| panic!("Failed to acquire mutex"))
}

/// Format bytes into human-readable size
fn human_size(bytes: u64) -> String {
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

/// Prompt user for yes/no confirmation with enhanced formatting
fn prompt_yes_no(prompt: &str) -> io::Result<bool> {
    println!("{}", "WARNING".bold().red());
    print!("{} {} ", prompt, "[y/N]:".dimmed());
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(matches!(response.as_str(), "y" | "yes"))
}

/// Display cache directories with individual sizes (calculated in parallel)
fn display_cache_dirs(dirs: &[PathBuf]) {
    println!(
        "\n{} {}",
        "FOUND".bold().blue(),
        format!("{} top-level cache directories:", dirs.len()).bold()
    );

    // Calculate sizes in parallel for better performance
    let thread_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(dirs.len().max(1));

    let sizes = Arc::new(Mutex::new(vec![0u64; dirs.len()]));
    let mut handles = Vec::new();

    let chunk_size = dirs.len().div_ceil(thread_count);

    for (chunk_idx, chunk) in dirs.chunks(chunk_size).enumerate() {
        let chunk_dirs: Vec<PathBuf> = chunk.to_vec();
        let sizes_clone = Arc::clone(&sizes);
        let base_idx = chunk_idx * chunk_size;

        let handle = thread::spawn(move || {
            for (i, dir) in chunk_dirs.iter().enumerate() {
                let dir_size = total_size(&[dir]);

                if let Ok(mut sizes_vec) = sizes_clone.lock()
                    && base_idx + i < sizes_vec.len()
                {
                    sizes_vec[base_idx + i] = dir_size;
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all size calculations to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Display thread panicked: {:?}", e);
        }
    }

    // Display results
    let final_sizes = Arc::try_unwrap(sizes)
        .unwrap_or_else(|_| panic!("Failed to unwrap sizes"))
        .into_inner()
        .unwrap_or_else(|_| panic!("Failed to acquire mutex"));

    for (i, dir) in dirs.iter().enumerate() {
        let dir_size = final_sizes.get(i).copied().unwrap_or(0);
        println!(
            "  {}. {} {}",
            (i + 1).to_string().dimmed(),
            dir.display().to_string().white(),
            format!("({})", human_size(dir_size)).red()
        );
    }
}

/// Clean cache directories with progress indication using parallel processing
fn clean_cache_dirs(dirs: &[PathBuf]) -> Vec<(PathBuf, Result<(), io::Error>)> {
    let total = dirs.len();
    let results = Arc::new(Mutex::new(Vec::with_capacity(total)));
    let progress_counter = Arc::new(Mutex::new(0usize));

    // Use fewer threads for deletion to avoid overwhelming the filesystem
    let thread_count = thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(2)
        .min(4);

    let mut handles = Vec::new();
    let chunk_size = dirs.len().div_ceil(thread_count);

    for chunk in dirs.chunks(chunk_size) {
        let chunk_dirs: Vec<PathBuf> = chunk.to_vec();
        let results_clone = Arc::clone(&results);
        let progress_counter_clone = Arc::clone(&progress_counter);

        let handle = thread::spawn(move || {
            let mut local_results = Vec::new();

            for dir in chunk_dirs {
                // Update progress counter
                let current_progress = {
                    let mut counter = progress_counter_clone.lock().unwrap();
                    *counter += 1;
                    *counter
                };

                print!(
                    "  {} Removing {} [{}/{}]",
                    "DELETING".red(),
                    dir.display(),
                    current_progress,
                    total
                );
                io::stdout().flush().unwrap();

                // Check if we have permission to delete this directory
                let result = if is_dir_accessible(&dir) {
                    fs::remove_dir_all(&dir)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Permission denied - try running with sudo",
                    ))
                };

                match &result {
                    Ok(()) => println!(" {}", "SUCCESS".green()),
                    Err(e) => {
                        if e.kind() == io::ErrorKind::PermissionDenied {
                            println!(
                                " {} ({})",
                                "PERMISSION DENIED".yellow(),
                                "try sudo".dimmed()
                            );
                        } else {
                            println!(" {}", "FAILED".red());
                        }
                    }
                }

                local_results.push((dir.clone(), result));
            }

            // Merge results
            if let Ok(mut global_results) = results_clone.lock() {
                global_results.extend(local_results);
            }
        });

        handles.push(handle);
    }

    // Wait for all deletion threads to complete
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Deletion thread panicked: {:?}", e);
        }
    }

    // Return results in original order
    Arc::try_unwrap(results)
        .unwrap_or_else(|_| panic!("Failed to unwrap results"))
        .into_inner()
        .unwrap_or_else(|_| panic!("Failed to acquire mutex"))
}

/// Display cleaning results with better formatting
fn display_cleaning_results(results: &[(PathBuf, Result<(), io::Error>)]) {
    println!("\n{}", "CLEANING RESULTS:".bold().blue());

    let mut success_count = 0;
    let mut permission_denied_count = 0;
    let mut failure_count = 0;

    for (dir, result) in results {
        match result {
            Ok(()) => {
                success_count += 1;
                println!(
                    "  {} {}",
                    "SUCCESS".green(),
                    dir.display().to_string().dimmed()
                );
            }
            Err(e) => {
                if e.kind() == io::ErrorKind::PermissionDenied {
                    permission_denied_count += 1;
                    println!(
                        "  {} {} - {}",
                        "PERMISSION DENIED".yellow(),
                        dir.display(),
                        "requires elevated privileges".dimmed()
                    );
                } else {
                    failure_count += 1;
                    println!(
                        "  {} {} - {}",
                        "FAILED".red(),
                        dir.display(),
                        e.to_string().red()
                    );
                }
            }
        }
    }

    println!(
        "\n{} {} {} {} {} {}",
        "SUMMARY:".bold().blue(),
        format!("{} successful", success_count).green().bold(),
        "|".dimmed(),
        format!("{} permission denied", permission_denied_count)
            .yellow()
            .bold(),
        "|".dimmed(),
        format!("{} failed", failure_count).red().bold()
    );

    if permission_denied_count > 0 {
        println!(
            "\n{} Run {} to clean system-wide cache directories.",
            "TIP:".bold().blue(),
            "sudo ./cleaner / --clean".green().bold()
        );
    }
}

/// Display summary box with key information
fn display_summary(cache_dirs: &[PathBuf], total_size_bytes: u64, root: &str) {
    println!("\n");
    println!("Scan path: {}", root.green());
    println!(
        "Directories found: {}",
        cache_dirs.len().to_string().yellow().bold()
    );
    println!(
        "Total size: {}",
        human_size(total_size_bytes).yellow().bold()
    );
}

fn main() -> io::Result<()> {
    let matches = build_cli().get_matches();

    let root = matches.get_one::<String>("path").unwrap();
    let clean_mode = matches.get_flag("clean");

    // Check if scanning system-wide but not running as root
    if root == "/" && !check_root_privileges() {
        println!(
            "{} Scanning system-wide without root privileges.",
            "WARNING".bold().yellow()
        );
        println!(
            "Some directories may be inaccessible. Run {} for complete access.",
            "sudo ./cleaner / --clean".green().bold()
        );
        println!();
    }

    // Show privilege information
    if check_root_privileges() {
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

    println!(
        "{}",
        format!("Scanning for cache directories under '{}'...", root)
            .white()
            .dimmed()
    );

    // Show thread information
    let thread_count = thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    println!(
        "{}",
        format!("Using {} threads for parallel processing", thread_count)
            .white()
            .dimmed()
    );

    let found_dirs = collect_cache_dirs(root);
    let cache_dirs = top_level_cache_dirs(found_dirs);

    if cache_dirs.is_empty() {
        println!(
            "{}",
            format!("No accessible cache directories found under '{}'", root).green()
        );

        if !check_root_privileges() && root == "/" {
            println!(
                "{}",
                "Try running with sudo to access system-wide cache directories.".dimmed()
            );
        }
        return Ok(());
    }

    let total_size_bytes = total_size(&cache_dirs);

    // Display directories with individual sizes
    display_cache_dirs(&cache_dirs);

    // Display summary
    display_summary(&cache_dirs, total_size_bytes, root);

    if clean_mode {
        let prompt = format!(
            "\nAre you sure you want to delete all {} cache directories totaling {}?",
            cache_dirs.len(),
            human_size(total_size_bytes)
        );

        match prompt_yes_no(&prompt)? {
            true => {
                println!("\n{}", "Cleaning cache directories...".bold().yellow());
                let results = clean_cache_dirs(&cache_dirs);
                display_cleaning_results(&results);
            }
            false => println!("{}", "Cleaning aborted.".yellow()),
        }
    } else {
        println!(
            "\n{}",
            "Use --clean flag to delete these directories.".dimmed()
        );

        if !check_root_privileges() && root == "/" {
            println!(
                "{}",
                "For system-wide cleaning, run: sudo ./cleaner / --clean"
                    .green()
                    .bold()
            );
        }
    }

    Ok(())
}
