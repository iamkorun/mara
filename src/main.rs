use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::time::Instant;

use mara::git::{enrich, list_blobs, top_blobs, BlobReport};
use mara::size::{format_size, parse_size};
use mara::stat;

const LONG_ABOUT: &str = "\
mara walks every object in a git repository, finds the largest blobs, and
tells you exactly which file they came from — along with a ready-to-run
`git filter-repo` command to remove them.

Examples:
  mara scan                          # top 20 blobs over 100 KB
  mara scan -m 1M -l 10              # top 10 blobs over 1 MB
  mara scan -p /path/to/repo         # scan another repo
  mara stat                          # .git size breakdown
  mara suggest -l 5                  # cleanup command for top 5 bloaters";

#[derive(Parser)]
#[command(
    name = "mara",
    version,
    about = "Find exactly what's bloating your git history",
    long_about = LONG_ABOUT,
)]
struct Cli {
    /// Verbose output (show progress on stderr).
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Suppress headers and summary lines.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scan the repo for large blobs.
    Scan(ScanArgs),
    /// Show .git directory size breakdown.
    Stat(PathArgs),
    /// Print a ready-to-run git filter-repo command for the top bloaters.
    Suggest(SuggestArgs),
}

#[derive(Parser)]
struct ScanArgs {
    /// Minimum blob size (e.g. 100K, 1M, 500KB).
    #[arg(short = 'm', long, default_value = "100K", value_name = "SIZE")]
    min_size: String,
    /// Maximum number of results.
    #[arg(short = 'l', long, default_value_t = 20, value_name = "N")]
    limit: usize,
    /// Path to the git repository.
    #[arg(short = 'p', long, default_value = ".", value_name = "PATH")]
    path: PathBuf,
}

#[derive(Parser)]
struct PathArgs {
    /// Path to the git repository.
    #[arg(short = 'p', long, default_value = ".", value_name = "PATH")]
    path: PathBuf,
}

#[derive(Parser)]
struct SuggestArgs {
    /// Minimum blob size (e.g. 100K, 1M).
    #[arg(short = 'm', long, default_value = "100K", value_name = "SIZE")]
    min_size: String,
    /// Number of bloaters to include in the cleanup command.
    #[arg(short = 'l', long, default_value_t = 5, value_name = "N")]
    limit: usize,
    /// Path to the git repository.
    #[arg(short = 'p', long, default_value = ".", value_name = "PATH")]
    path: PathBuf,
}

fn main() {
    if let Err(e) = run() {
        eprintln!("error: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Scan(args) => cmd_scan(args, cli.verbose, cli.quiet),
        Commands::Stat(args) => cmd_stat(args, cli.quiet),
        Commands::Suggest(args) => cmd_suggest(args, cli.verbose, cli.quiet),
    }
}

struct ScanSummary {
    reports: Vec<BlobReport>,
    total_blobs: usize,
    elapsed_ms: u128,
}

fn collect_reports(
    path: &Path,
    min_size: &str,
    limit: usize,
    verbose: bool,
) -> Result<ScanSummary> {
    let min = parse_size(min_size)?;
    let repo = mara::git::ensure_repo(path)?;
    if verbose {
        eprintln!(
            "scanning {} (min size {})",
            repo.display(),
            format_size(min)
        );
    }
    let start = Instant::now();
    let blobs = list_blobs(&repo)?;
    let total_blobs = blobs.len();
    if verbose {
        eprintln!("found {} blob entries total", total_blobs);
    }
    let top = top_blobs(blobs, min, limit);
    let reports = enrich(&repo, top);
    Ok(ScanSummary {
        reports,
        total_blobs,
        elapsed_ms: start.elapsed().as_millis(),
    })
}

fn cmd_scan(args: &ScanArgs, verbose: bool, quiet: bool) -> Result<()> {
    let summary = collect_reports(&args.path, &args.min_size, args.limit, verbose)?;
    if summary.reports.is_empty() {
        if !quiet {
            println!("No blobs >= {} found.", args.min_size);
        }
        return Ok(());
    }
    if !quiet {
        println!(
            "{:>10}  {:<8}  {:<10}  {:<20}  PATH",
            "SIZE", "SHA", "DATE", "AUTHOR"
        );
        println!("{}", "-".repeat(78));
    }
    for r in &summary.reports {
        let date = r
            .date
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(10)
            .collect::<String>();
        let author = r
            .author
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(20)
            .collect::<String>();
        let sha = r
            .commit
            .as_deref()
            .unwrap_or("-")
            .chars()
            .take(8)
            .collect::<String>();
        let path = if r.path.is_empty() {
            "(unreferenced)"
        } else {
            &r.path
        };
        println!(
            "{:>10}  {:<8}  {:<10}  {:<20}  {}",
            format_size(r.size),
            sha,
            date,
            author,
            path
        );
    }
    if !quiet {
        println!();
        println!(
            "Scanned {} objects in {}",
            summary.total_blobs,
            format_duration(summary.elapsed_ms)
        );
    }
    Ok(())
}

fn format_duration(ms: u128) -> String {
    if ms < 1000 {
        format!("{} ms", ms)
    } else {
        format!("{:.2} s", ms as f64 / 1000.0)
    }
}

fn cmd_stat(args: &PathArgs, quiet: bool) -> Result<()> {
    let repo = mara::git::ensure_repo(&args.path)?;
    let s = stat::collect(&repo)?;
    if quiet {
        println!("{}", s.total);
        return Ok(());
    }
    println!("Repository: {}", repo.display());
    println!("Git dir:    {}", s.git_dir.display());
    println!();
    println!("  .git total      {:>12}", format_size(s.total));
    println!("    loose objects {:>12}", format_size(s.loose_objects));
    println!("    pack files    {:>12}", format_size(s.pack_files));
    println!("    index         {:>12}", format_size(s.index));
    println!("  working tree    {:>12}", format_size(s.working_tree));
    Ok(())
}

fn cmd_suggest(args: &SuggestArgs, verbose: bool, quiet: bool) -> Result<()> {
    let summary = collect_reports(&args.path, &args.min_size, args.limit, verbose)?;
    if summary.reports.is_empty() {
        if !quiet {
            println!("No bloaters found above {}.", args.min_size);
        }
        return Ok(());
    }
    if !quiet {
        println!(
            "# WARNING: this rewrites git history. Coordinate with your team and back up first."
        );
        println!("# Requires: pip install git-filter-repo");
        println!();
    }
    let paths: Vec<&str> = summary
        .reports
        .iter()
        .filter_map(|r| {
            if r.path.is_empty() {
                None
            } else {
                Some(r.path.as_str())
            }
        })
        .collect();
    if paths.is_empty() {
        if !quiet {
            println!("# Top bloaters are unreferenced — try `git gc --prune=now --aggressive`.");
        }
        return Ok(());
    }
    print!("git filter-repo --invert-paths");
    for p in &paths {
        print!(" --path {}", shell_escape(p));
    }
    println!();
    Ok(())
}

fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_ascii_alphanumeric() || "/._-".contains(c))
    {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
