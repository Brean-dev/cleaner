use std::{env, fs, io, io::Write, path::Path};
use walkdir::{DirEntry, WalkDir};

fn has_cache_in_path(entry: &DirEntry) -> bool {
    entry.file_type().is_dir() && entry.path().to_string_lossy().contains(".cache")
}

fn collect_cache_dirs<P: AsRef<Path>>(root: P) -> Vec<String> {
    let mut dirs = Vec::new();
    for entry in WalkDir::new(root)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if has_cache_in_path(&entry) {
            dirs.push(entry.path().to_string_lossy().into_owned());
        }
    }
    dirs
}

// Only keep the top-most directories (not nested inside another .cache dir)
fn top_level_cache_dirs(mut dirs: Vec<String>) -> Vec<String> {
    dirs.sort_by(|a, b| a.len().cmp(&b.len()));
    let mut top_level = Vec::new();
    for dir in dirs {
        if !top_level
            .iter()
            .any(|parent| dir.starts_with(parent) && dir != *parent)
        {
            top_level.push(dir);
        }
    }
    top_level
}

fn total_size<P: AsRef<Path>>(paths: &[P]) -> u64 {
    let mut total = 0u64;
    for p in paths {
        for entry in WalkDir::new(p).into_iter().filter_map(|e| e.ok()) {
            if entry.file_type().is_file() {
                if let Ok(meta) = entry.metadata() {
                    total += meta.len();
                }
            }
        }
    }
    total
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1024.0 && unit < UNITS.len() - 1 {
        size /= 1024.0;
        unit += 1;
    }
    format!("{:.2} {}", size, UNITS[unit])
}

fn prompt_yes_no(prompt: &str) -> bool {
    print!("{}", prompt);
    io::stdout().flush().unwrap();
    let mut input = String::new();
    match io::stdin().read_line(&mut input) {
        Ok(_) => {
            let resp = input.trim().to_lowercase();
            resp == "y" || resp == "yes"
        }
        Err(_) => false,
    }
}

fn clean_cache_dirs(dirs: &[String]) {
    for dir in dirs {
        match fs::remove_dir_all(dir) {
            Ok(_) => println!("Removed: {}", dir),
            Err(e) => println!("Failed to remove {}: {}", dir, e),
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let root = if args.len() > 1 { &args[1] } else { "/" };
    let clean = args.iter().any(|a| a == "--clean");

    let found_dirs = collect_cache_dirs(root);
    let cache_dirs = top_level_cache_dirs(found_dirs);

    if cache_dirs.is_empty() {
        println!("No directories containing '.cache' found under '{}'", root);
        return;
    }

    let size = total_size(&cache_dirs);
    println!(
        "Found {} top-level directories containing '.cache' under '{}':",
        cache_dirs.len(),
        root
    );
    for dir in &cache_dirs {
        println!("  {}", dir);
    }
    println!("Total size: {}", human_size(size));

    if clean {
        let prompt = format!(
            "\nAre you sure you want to clean (delete) all top-level directories containing '.cache' under '{}' totaling {}? (y/N): ",
            root,
            human_size(size),
        );
        if prompt_yes_no(&prompt) {
            clean_cache_dirs(&cache_dirs);
        } else {
            println!("Aborted cleaning.");
        }
    }
}
