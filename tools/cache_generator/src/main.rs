use rand::distr::Alphanumeric;
use rand::{Rng, RngCore, SeedableRng};
use rand_chacha::ChaCha8Rng;
use std::{
    env, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    thread,
    time::Instant,
};

const MAX_TOTAL_SIZE: u64 = 1024 * 1024 * 1024; // 1GB
const MIN_FILE_SIZE: u64 = 1024; // 1KB
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10MB
const PROGRESS_UPDATE_INTERVAL: u64 = 10 * 1024 * 1024; // 10MB
const FILES_PER_BATCH: usize = 50; // Process files in batches for better thread utilization

struct CacheGenerator {
    cache_dir: PathBuf,
    /// Using AtomicU64 instead of Mutex for better performance on progress tracking
    total_generated: Arc<AtomicU64>,
    target_size: u64,
    /// Number of worker threads for file generation
    num_threads: usize,
}

#[derive(Clone)]
enum FileType {
    Binary,
    Json,
    Log,
    Temp,
    Database,
}

/// Represents a file generation task that can be sent between threads
#[derive(Clone)]
struct FileTask {
    dir: PathBuf,
    file_type: FileType,
    target_size: u64,
}

impl CacheGenerator {
    fn new() -> io::Result<Self> {
        let home = env::var("HOME").map_err(|_| {
            io::Error::new(io::ErrorKind::NotFound, "HOME environment variable not set")
        })?;

        let cache_dir = PathBuf::from(home).join(".cache");

        // Use available CPU cores for optimal threading
        let num_threads = num_cpus::get().max(1);

        Ok(Self {
            cache_dir,
            total_generated: Arc::new(AtomicU64::new(0)),
            target_size: MAX_TOTAL_SIZE,
            num_threads,
        })
    }

    fn ensure_cache_dir(&self) -> io::Result<()> {
        if !self.cache_dir.exists() {
            fs::create_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    fn create_app_directories(&self) -> io::Result<Vec<PathBuf>> {
        let app_names = [
            "firefox",
            "chrome",
            "chromium",
            "brave",
            "opera",
            "vscode",
            "atom",
            "sublime-text",
            "vim",
            "emacs",
            "spotify",
            "vlc",
            "gimp",
            "inkscape",
            "blender",
            "discord",
            "slack",
            "teams",
            "zoom",
            "skype",
            "steam",
            "lutris",
            "wine",
            "bottles",
            "heroic",
            "npm",
            "pip",
            "cargo",
            "composer",
            "yarn",
            "docker",
            "podman",
            "flatpak",
            "snap",
            "appimage",
            "gnome",
            "kde",
            "xfce",
            "i3",
            "awesome",
            "thumbnails",
            "fontconfig",
            "mesa_shader_cache",
        ];

        let mut system_rng = rand::rng();
        let mut rng = ChaCha8Rng::from_rng(&mut system_rng);
        let num_apps = rng.random_range(8..=15);
        let mut created_dirs = Vec::new();

        for _ in 0..num_apps {
            let app_name = app_names[rng.random_range(0..app_names.len())];
            let mut app_dir = self.cache_dir.join(app_name);

            // Add version subdirectory sometimes
            if rng.random_bool(0.33) {
                let version = format!("v{}.{}", rng.random_range(1..10), rng.random_range(0..20));
                app_dir = app_dir.join(version);
            }

            // Add cache subdirectory sometimes
            if rng.random_bool(0.5) {
                let subdirs = ["cache", "tmp", "data", "logs", "session", "storage"];
                let subdir = subdirs[rng.random_range(0..subdirs.len())];
                app_dir = app_dir.join(subdir);
            }

            if let Ok(()) = fs::create_dir_all(&app_dir) {
                created_dirs.push(app_dir);
            }
        }

        Ok(created_dirs)
    }

    /// Optimized random string generation with thread-local RNG
    fn generate_random_string_with_rng(rng: &mut ChaCha8Rng, length: usize) -> String {
        (0..length)
            .map(|_| rng.sample(Alphanumeric) as char)
            .collect()
    }

    /// Optimized hex generation with thread-local RNG
    fn generate_random_hex_with_rng(rng: &mut ChaCha8Rng, length: usize) -> String {
        const HEX_CHARS: &[u8] = b"abcdef0123456789";
        (0..length)
            .map(|_| HEX_CHARS[rng.random_range(0..HEX_CHARS.len())] as char)
            .collect()
    }

    /// Create file content with improved efficiency using provided RNG
    fn create_file_content_with_rng(
        rng: &mut ChaCha8Rng,
        file_type: &FileType,
        size: u64,
    ) -> Vec<u8> {
        match file_type {
            FileType::Binary => {
                let mut data = vec![0u8; size as usize];
                rng.fill_bytes(&mut data);
                data
            }
            FileType::Json => {
                let data_size = if size > 200 { size - 200 } else { 100 };
                let data_content = Self::generate_random_string_with_rng(rng, data_size as usize);
                let json = format!(
                    r#"{{"timestamp":{},"user":"{}","session_id":"{}","data":"{}"}}"#,
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_secs(),
                    env::var("USER").unwrap_or_else(|_| "unknown".to_string()),
                    Self::generate_random_hex_with_rng(rng, 32),
                    data_content
                );
                json.into_bytes()
            }
            FileType::Log => {
                let lines = (size / 100).max(1);
                let mut content = String::with_capacity(size as usize);
                for _ in 0..lines {
                    content.push_str(&format!(
                        "{} [INFO] Cache operation {}\n",
                        chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                        Self::generate_random_string_with_rng(rng, 50)
                    ));
                }
                content.into_bytes()
            }
            FileType::Temp => {
                Self::generate_random_string_with_rng(rng, size as usize).into_bytes()
            }
            FileType::Database => {
                let data_size = if size > 100 { size - 100 } else { 100 };
                let content = format!(
                    "CACHE_DB_VERSION=1.0\nCREATED={}\nDATA={}",
                    chrono::Local::now(),
                    Self::generate_random_string_with_rng(rng, data_size as usize)
                );
                content.into_bytes()
            }
        }
    }

    /// Generate a single file with provided RNG for better performance
    fn generate_file_with_rng(
        &self,
        rng: &mut ChaCha8Rng,
        dir: &Path,
        file_type: FileType,
        target_size: u64,
    ) -> io::Result<u64> {
        let (filename, extension) = match file_type {
            FileType::Binary => (
                format!("cache_{}", Self::generate_random_hex_with_rng(rng, 16)),
                "bin",
            ),
            FileType::Json => (
                format!("session_{}", Self::generate_random_hex_with_rng(rng, 8)),
                "json",
            ),
            FileType::Log => (
                format!("app_{}", chrono::Local::now().format("%Y%m%d")),
                "log",
            ),
            FileType::Temp => (
                format!("tmp_{}", Self::generate_random_hex_with_rng(rng, 12)),
                "tmp",
            ),
            FileType::Database => ("cache".to_string(), "db"),
        };

        let filepath = dir.join(format!("{}.{}", filename, extension));
        let content = Self::create_file_content_with_rng(rng, &file_type, target_size);

        fs::write(&filepath, &content)?;
        Ok(content.len() as u64)
    }

    /// Worker thread function that processes file generation tasks
    fn worker_thread(
        &self,
        tasks: Arc<Mutex<Vec<FileTask>>>,
        progress_counter: Arc<AtomicU64>,
    ) -> u64 {
        let mut total_generated = 0u64;
        // Use seed_from_u64 with a random seed for thread-local RNG
        let mut rng = ChaCha8Rng::seed_from_u64(rand::random());

        loop {
            // Get a batch of tasks to process
            let batch = {
                let mut tasks_guard = tasks.lock().unwrap();
                if tasks_guard.is_empty() {
                    break; // No more tasks
                }

                // Take up to FILES_PER_BATCH tasks at once to reduce lock contention
                let take_count = tasks_guard.len().min(FILES_PER_BATCH);
                tasks_guard.drain(0..take_count).collect::<Vec<_>>()
            };

            // Process the batch without holding the lock
            for task in batch {
                if let Ok(file_size) = self.generate_file_with_rng(
                    &mut rng,
                    &task.dir,
                    task.file_type,
                    task.target_size,
                ) {
                    total_generated += file_size;

                    // Update progress atomically (much faster than mutex)
                    let current_total = progress_counter.fetch_add(file_size, Ordering::Relaxed);

                    // Reduced frequency progress updates to minimize overhead
                    if current_total % PROGRESS_UPDATE_INTERVAL < file_size {
                        let progress = (current_total * 100) / self.target_size;
                        let progress_bar = "#".repeat((progress / 5) as usize);
                        print!(
                            "\rProgress: [{:<20}] {}% ({})",
                            progress_bar,
                            progress,
                            human_readable_size(current_total)
                        );
                        let _ = io::stdout().flush();
                    }
                }
            }
        }

        total_generated
    }

    /// Generate tasks for file creation (pre-compute what files to create)
    fn generate_file_tasks(&self, directories: &[PathBuf]) -> Vec<FileTask> {
        let mut rng = ChaCha8Rng::seed_from_u64(rand::random());
        let mut tasks = Vec::new();
        let size_per_dir = self.target_size / directories.len() as u64;

        let file_types = [
            FileType::Binary,
            FileType::Json,
            FileType::Log,
            FileType::Temp,
            FileType::Database,
        ];

        for (i, dir) in directories.iter().enumerate() {
            let mut target_size = size_per_dir;

            // Give the last directory any remaining size
            if i == directories.len() - 1 {
                let used_size = size_per_dir * (directories.len() - 1) as u64;
                target_size = self.target_size - used_size;
            }

            let mut current_size = 0u64;

            // Pre-generate all file tasks for this directory
            while current_size < target_size {
                let remaining = target_size - current_size;
                if remaining < MIN_FILE_SIZE {
                    break;
                }

                let file_size = rng.random_range(MIN_FILE_SIZE..=remaining.min(MAX_FILE_SIZE));
                let file_type = file_types[rng.random_range(0..file_types.len())].clone();

                tasks.push(FileTask {
                    dir: dir.clone(),
                    file_type,
                    target_size: file_size,
                });

                current_size += file_size;
            }
        }

        tasks
    }

    fn generate(&self) -> io::Result<()> {
        println!(
            "Generating fake cache files using {} threads...",
            self.num_threads
        );
        let start_time = Instant::now();

        self.ensure_cache_dir()?;
        let directories = self.create_app_directories()?;

        if directories.is_empty() {
            return Err(io::Error::other("No cache directories were created"));
        }

        // Pre-generate all file tasks to distribute work evenly across threads
        let file_tasks = self.generate_file_tasks(&directories);
        let tasks = Arc::new(Mutex::new(file_tasks));
        let progress_counter = Arc::new(AtomicU64::new(0));

        // Spawn worker threads
        let mut handles = Vec::new();
        for _ in 0..self.num_threads {
            let generator = self.clone();
            let tasks = Arc::clone(&tasks);
            let progress_counter = Arc::clone(&progress_counter);

            let handle = thread::spawn(move || generator.worker_thread(tasks, progress_counter));
            handles.push(handle);
        }

        // Wait for all threads to complete and collect results
        let mut total_actual = 0u64;
        for handle in handles {
            match handle.join() {
                Ok(size) => total_actual += size,
                Err(_) => eprintln!("Thread panicked during file generation"),
            }
        }

        println!(); // New line after progress bar
        let duration = start_time.elapsed();
        let throughput = total_actual as f64 / duration.as_secs_f64() / (1024.0 * 1024.0);

        println!(
            "\x1b[32m[SUCCESS]\x1b[0m Generated {} in {} directories",
            human_readable_size(total_actual),
            directories.len()
        );
        println!(
            "\x1b[32m[SUCCESS]\x1b[0m Cache generation completed in {:.2}s ({:.1} MB/s) - ready for testing",
            duration.as_secs_f64(),
            throughput
        );

        Ok(())
    }

    fn clean(&self) -> io::Result<()> {
        println!("Cleaning up generated cache files...");

        if self.cache_dir.exists() {
            print!(
                "Delete all contents of {}? (y/N): ",
                self.cache_dir.display()
            );
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" || input.trim().to_lowercase() == "yes" {
                if let Ok(entries) = fs::read_dir(&self.cache_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_dir() {
                            fs::remove_dir_all(&path)?;
                        } else {
                            fs::remove_file(&path)?;
                        }
                    }
                }
                println!("\x1b[32m[SUCCESS]\x1b[0m Cache directory cleaned");
            } else {
                println!("Cleanup cancelled");
            }
        } else {
            println!("No cache directory found to clean");
        }

        Ok(())
    }
}

// Clone implementation for sharing between threads
impl Clone for CacheGenerator {
    fn clone(&self) -> Self {
        Self {
            cache_dir: self.cache_dir.clone(),
            total_generated: Arc::clone(&self.total_generated),
            target_size: self.target_size,
            num_threads: self.num_threads,
        }
    }
}

fn human_readable_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
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

    format!("{:.1} {}", size, UNITS[unit_index])
}

fn show_help() {
    println!(
        r#"
Usage: cache_generator [OPTIONS]

Generate fake cache entries in ~/.cache for testing cache cleaning tools.

OPTIONS:
    -h, --help      Show this help message
    -c, --clean     Clean up generated cache files
    -g, --generate  Generate fake cache files (default action)

EXAMPLES:
    cache_generator                 # Generate fake cache files
    cache_generator --generate      # Same as above
    cache_generator --clean         # Clean up generated files
    cache_generator --help          # Show this help

NOTES:
    - Maximum total size: {}
    - Files are created only in the current user's ~/.cache directory
    - Uses {} threads for optimal performance
    - Generated files have realistic names and content types
"#,
        human_readable_size(MAX_TOTAL_SIZE),
        num_cpus::get()
    );
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let action = if args.len() > 1 {
        match args[1].as_str() {
            "-h" | "--help" => {
                show_help();
                return Ok(());
            }
            "-c" | "--clean" => "clean",
            "-g" | "--generate" => "generate",
            _ => {
                eprintln!("\x1b[31m[ERROR]\x1b[0m Unknown option: {}", args[1]);
                eprintln!("Use --help for usage information");
                std::process::exit(1);
            }
        }
    } else {
        "generate"
    };

    let generator = CacheGenerator::new()?;

    match action {
        "generate" => {
            if let Err(e) = generator.generate() {
                eprintln!("\x1b[31m[ERROR]\x1b[0m Cache generation failed: {}", e);
                std::process::exit(1);
            }
        }
        "clean" => generator.clean()?,
        _ => unreachable!(),
    }

    Ok(())
}
