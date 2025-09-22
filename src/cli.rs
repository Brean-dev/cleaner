use clap::{Arg, ArgAction, Command};
use std::path::PathBuf;

/// Command line interface configuration
#[derive(Debug, Clone)]
pub struct CliArgs {
    /// Root path to scan for cache directories
    pub path: PathBuf,
    /// Actually delete the found cache and log files
    pub clean: bool,
    /// Show what would be deleted without actually deleting
    pub dry_run: bool,
    /// Enable verbose output
    pub verbose: bool,
    /// Configuration file path
    pub config: Option<PathBuf>,
    /// Enable log cleanup
    pub clean_logs: bool,
    /// Override log age threshold (in days)
    pub log_age_days: Option<u64>,
    /// Force cleanup without confirmation
    pub force: bool,
    /// Show detailed size information
    pub show_sizes: bool,
    /// Only show summary without listing individual items
    pub summary_only: bool,
}

impl Default for CliArgs {
    fn default() -> Self {
        Self {
            path: PathBuf::from("/"),
            clean: false,
            dry_run: false,
            verbose: false,
            config: None,
            clean_logs: false,
            log_age_days: None,
            force: false,
            show_sizes: true,
            summary_only: false,
        }
    }
}

/// Build command line interface
pub fn build_cli() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .about("A fast parallel cache and log file cleaner for Linux systems")
        .long_about(
            "A sophisticated cache and log file cleaner that follows XDG Base Directory \
             specifications and includes comprehensive safety checks. Supports parallel \
             processing for fast cleanup of cache directories, temporary files, and old log files."
        )
        .author("Brean-dev")
        .arg(
            Arg::new("path")
                .help("Root path to scan for cache directories and log files")
                .long_help(
                    "The root directory to scan for cache directories and log files. \
                     Use '/' for system-wide scanning or specify a user directory like '/home/user'. \
                     System-wide scanning requires root privileges for full access."
                )
                .default_value("/")
                .index(1),
        )
        .arg(
            Arg::new("clean")
                .long("clean")
                .short('c')
                .help("Actually delete the found cache directories and files")
                .long_help(
                    "Enable deletion mode. Without this flag, the tool will only scan and report \
                     what would be deleted. This is the recommended way to first understand \
                     what the tool would clean before actually running the cleanup."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .short('n')
                .help("Show what would be deleted without actually deleting")
                .long_help(
                    "Perform a dry run - scan and show what would be deleted but don't actually \
                     delete anything. This overrides the --clean flag and is useful for testing \
                     configuration changes."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("verbose")
                .long("verbose")
                .short('v')
                .help("Enable verbose output with detailed information")
                .long_help(
                    "Enable verbose output showing detailed information about the scanning process, \
                     thread usage, permission issues, and individual file operations."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("config")
                .long("config")
                .short('f')
                .help("Path to configuration file")
                .long_help(
                    "Specify a custom configuration file path. If not provided, the tool will \
                     look for config.toml in the XDG config directory (~/.config/cleaner/config.toml). \
                     If no config file exists, a default one will be created."
                )
                .value_name("FILE"),
        )
        .arg(
            Arg::new("clean-logs")
                .long("logs")
                .short('l')
                .help("Enable cleanup of old log files")
                .long_help(
                    "Enable cleanup of log files older than the configured threshold (default: 7 days). \
                     This will search for log files in standard locations like /var/log and user \
                     application log directories."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("log-age")
                .long("log-age")
                .help("Override log age threshold in days (default: 7)")
                .long_help(
                    "Override the maximum age for log files in days. Log files older than this \
                     threshold will be considered for deletion. This overrides the setting in \
                     the configuration file."
                )
                .value_name("DAYS")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .short('F')
                .help("Force cleanup without confirmation prompts")
                .long_help(
                    "Skip confirmation prompts and force cleanup. Use with caution as this \
                     bypasses safety checks that ask for user confirmation before large deletions."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-sizes")
                .long("no-sizes")
                .help("Skip calculating and displaying file sizes (faster)")
                .long_help(
                    "Skip size calculation for found files and directories. This makes the scan \
                     faster but you won't see how much space would be freed."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("summary-only")
                .long("summary")
                .short('s')
                .help("Show only summary without listing individual items")
                .long_help(
                    "Show only a summary of found cache directories and log files without \
                     listing each individual item. Useful for quick overview or scripting."
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("threads")
                .long("threads")
                .short('t')
                .help("Number of threads to use for parallel processing")
                .long_help(
                    "Override the number of threads used for parallel processing. By default, \
                     the tool uses the number of CPU cores available, capped at 8 threads. \
                     Use this to limit resource usage on busy systems."
                )
                .value_name("COUNT")
                .value_parser(clap::value_parser!(usize)),
        )
        .arg(
            Arg::new("max-depth")
                .long("max-depth")
                .help("Maximum directory depth to scan")
                .long_help(
                    "Limit the maximum depth of directory traversal. This can help avoid \
                     very deep directory structures that might cause performance issues. \
                     Default is 10 levels deep."
                )
                .value_name("DEPTH")
                .value_parser(clap::value_parser!(usize)),
        )
}

/// Parse command line arguments into CliArgs struct
pub fn parse_args() -> CliArgs {
    let matches = build_cli().get_matches();

    CliArgs {
        path: PathBuf::from(matches.get_one::<String>("path").unwrap()),
        clean: matches.get_flag("clean") && !matches.get_flag("dry-run"),
        dry_run: matches.get_flag("dry-run"),
        verbose: matches.get_flag("verbose"),
        config: matches.get_one::<String>("config").map(PathBuf::from),
        clean_logs: matches.get_flag("clean-logs"),
        log_age_days: matches.get_one::<u64>("log-age").copied(),
        force: matches.get_flag("force"),
        show_sizes: !matches.get_flag("no-sizes"),
        summary_only: matches.get_flag("summary-only"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_building() {
        let cmd = build_cli();
        assert_eq!(cmd.get_name(), env!("CARGO_PKG_NAME"));
    }

    #[test]
    fn test_default_args() {
        let args = CliArgs::default();
        assert_eq!(args.path, PathBuf::from("/"));
        assert!(!args.clean);
        assert!(!args.dry_run);
    }
}
