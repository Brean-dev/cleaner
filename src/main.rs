use colored::*;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
};
use walkdir::{DirEntry, WalkDir};

/// Check if a directory entry contains cache-related patterns in its path
fn has_cache_in_path(entry: &DirEntry) -> bool {
    const CACHE_PATTERNS: &[&str] = &[".cache"];

    entry.file_type().is_dir()
        && CACHE_PATTERNS
            .iter()
            .any(|pattern| entry.path().to_string_lossy().contains(pattern))
}

/// Collect all cache directories under the given root path
fn collect_cache_dirs<P: AsRef<Path>>(root: P) -> Vec<PathBuf> {
    WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(Result::ok)
        .filter(has_cache_in_path)
        .map(|entry| entry.into_path())
        .collect()
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

/// Calculate total size of files in the given paths
fn total_size<P: AsRef<Path>>(paths: &[P]) -> u64 {
    paths
        .iter()
        .flat_map(|path| {
            WalkDir::new(path)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .filter_map(|entry| entry.metadata().ok())
                .map(|metadata| metadata.len())
        })
        .sum()
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

/// Display cache directories with individual sizes
fn display_cache_dirs(dirs: &[PathBuf]) {
    println!(
        "\n{} {}",
        "FOUND".bold().blue(),
        format!("{} top-level cache directories:", dirs.len()).bold()
    );

    for (i, dir) in dirs.iter().enumerate() {
        let dir_size = total_size(&[dir]);
        println!(
            "  {}. {} {}",
            (i + 1).to_string().dimmed(),
            dir.display().to_string().white(),
            format!("({})", human_size(dir_size)).red()
        );
    }
}

/// Clean cache directories with progress indication
fn clean_cache_dirs(dirs: &[PathBuf]) -> Vec<(PathBuf, Result<(), io::Error>)> {
    let total = dirs.len();
    dirs.iter()
        .enumerate()
        .map(|(i, dir)| {
            print!(
                "  {} Removing {} [{}/{}]",
                "DELETING".red(),
                dir.display(),
                i + 1,
                total
            );
            io::stdout().flush().unwrap();

            let result = fs::remove_dir_all(dir);

            match &result {
                Ok(()) => println!(" {}", "SUCCESS".green()),
                Err(_) => println!(" {}", "FAILED".red()),
            }

            (dir.clone(), result)
        })
        .collect()
}

/// Display cleaning results with better formatting
fn display_cleaning_results(results: &[(PathBuf, Result<(), io::Error>)]) {
    println!("\n{}", "CLEANING RESULTS:".bold().blue());

    let mut success_count = 0;
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

    println!(
        "\n{} {} {} {}",
        "SUMMARY:".bold().blue(),
        format!("{} successful", success_count).green().bold(),
        "|".dimmed(),
        format!("{} failed", failure_count).red().bold()
    );
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
    let args: Vec<String> = env::args().collect();
    let root = args.get(1).map(String::as_str).unwrap_or("/");
    let clean_mode = args.iter().any(|arg| arg == "--clean");

    println!(
        "{}",
        format!("Scanning for cache directories under '{}'...", root)
            .white()
            .dimmed()
    );

    let found_dirs = collect_cache_dirs(root);
    let cache_dirs = top_level_cache_dirs(found_dirs);

    if cache_dirs.is_empty() {
        println!(
            "{}",
            format!("No directories containing '.cache' found under '{}'", root).green()
        );
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
    }

    Ok(())
}
