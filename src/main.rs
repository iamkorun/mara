use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};

use mara::git::{enrich, list_blobs, top_blobs, BlobReport};
use mara::size::{format_size, parse_size};
use mara::stat;

#[derive(Parser)]
#[command(name = "mara", version, about = "Find exactly what's bloating your git history", long_about = None)]
struct Cli {
    /// Verbose output (show progress).
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Suppress non-essential output.
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
    #[arg(long, default_value = "100K")]
    min_size: String,
    /// Maximum number of results.
    #[arg(long, default_value_t = 20)]
    limit: usize,
    /// Path to the git repository.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[derive(Parser)]
struct PathArgs {
    /// Path to the git repository.
    #[arg(long, default_value = ".")]
    path: PathBuf,
}

#[derive(Parser)]
struct SuggestArgs {
    /// Minimum blob size (e.g. 100K, 1M).
    #[arg(long, default_value = "100K")]
    min_size: String,
    /// Number of bloaters to suggest cleanup for.
    #[arg(long, default_value_t = 5)]
    limit: usize,
    /// Path to the git repository.
    #[arg(long, default_value = ".")]
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

fn collect_reports(
    path: &Path,
    min_size: &str,
    limit: usize,
    verbose: bool,
) -> Result<Vec<BlobReport>> {
    let min = parse_size(min_size)?;
    let repo = mara::git::ensure_repo(path)?;
    if verbose {
        eprintln!(
            "scanning {} (min size {})",
            repo.display(),
            format_size(min)
        );
    }
    let blobs = list_blobs(&repo)?;
    if verbose {
        eprintln!("found {} blob entries total", blobs.len());
    }
    let top = top_blobs(blobs, min, limit);
    Ok(enrich(&repo, top))
}

fn cmd_scan(args: &ScanArgs, verbose: bool, quiet: bool) -> Result<()> {
    let reports = collect_reports(&args.path, &args.min_size, args.limit, verbose)?;
    if reports.is_empty() {
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
    for r in &reports {
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
    Ok(())
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
    let reports = collect_reports(&args.path, &args.min_size, args.limit, verbose)?;
    if reports.is_empty() {
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
    let paths: Vec<&str> = reports
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
