use std::process::Command;
use tempfile::TempDir;

fn git(dir: &std::path::Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "test")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "test")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .status()
        .expect("git");
    assert!(status.success(), "git {:?} failed", args);
}

fn bin() -> std::path::PathBuf {
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("mara")
}

fn setup_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    std::fs::write(p.join("small.txt"), "hello").unwrap();
    git(p, &["add", "small.txt"]);
    git(p, &["commit", "-q", "-m", "small"]);
    let big = vec![b'x'; 200 * 1024];
    std::fs::write(p.join("big.bin"), &big).unwrap();
    git(p, &["add", "big.bin"]);
    git(p, &["commit", "-q", "-m", "big"]);
    dir
}

#[test]
fn scan_finds_large_blob() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["scan", "--min-size", "100K", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("big.bin"),
        "missing big.bin in:\n{}",
        stdout
    );
    assert!(!stdout.contains("small.txt"));
}

#[test]
fn scan_no_results() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["scan", "--min-size", "10M", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("No blobs"));
}

#[test]
fn stat_works() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["stat", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains(".git total"));
}

#[test]
fn suggest_emits_filter_repo() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["suggest", "--min-size", "100K", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("git filter-repo"));
    assert!(s.contains("big.bin"));
}

#[test]
fn errors_outside_repo() {
    let dir = TempDir::new().unwrap();
    let out = Command::new(bin())
        .args(["scan", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("not inside a git repository"));
}

#[test]
fn scan_quiet_suppresses_header() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["--quiet", "scan", "--min-size", "100K", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Table header is suppressed; data row is still printed.
    assert!(!stdout.contains("SIZE      SHA"));
    assert!(stdout.contains("big.bin"));
}

#[test]
fn scan_verbose_writes_progress_to_stderr() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["--verbose", "scan", "--min-size", "100K", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("scanning"));
    assert!(stderr.contains("blob entries"));
}

#[test]
fn suggest_quiet_omits_warning_header() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["--quiet", "suggest", "--min-size", "100K", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("WARNING"));
    assert!(!stdout.contains("git-filter-repo"));
    assert!(stdout.contains("git filter-repo"));
}

#[test]
fn suggest_escapes_paths_with_spaces() {
    let dir = TempDir::new().unwrap();
    let p = dir.path();
    git(p, &["init", "-q", "-b", "main"]);
    let big = vec![b'x'; 200 * 1024];
    std::fs::write(p.join("big file name.bin"), &big).unwrap();
    git(p, &["add", "big file name.bin"]);
    git(p, &["commit", "-q", "-m", "big"]);

    let out = Command::new(bin())
        .args(["suggest", "--min-size", "100K", "--path"])
        .arg(p)
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Path with spaces must be quoted.
    assert!(
        stdout.contains("'big file name.bin'"),
        "path not escaped:\n{}",
        stdout
    );
}

#[test]
fn invalid_min_size_errors_cleanly() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["scan", "--min-size", "garbage", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("invalid size"));
}

#[test]
fn version_flag_prints_version() {
    let out = Command::new(bin())
        .arg("--version")
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("mara"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_flag_lists_subcommands() {
    let out = Command::new(bin())
        .arg("--help")
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("scan"));
    assert!(stdout.contains("stat"));
    assert!(stdout.contains("suggest"));
}

#[test]
fn stat_quiet_prints_only_total() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["--quiet", "stat", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    // Quiet output is just the total byte count line; no labels.
    assert!(!stdout.contains("Repository:"));
    assert!(!stdout.contains("loose objects"));
    // First non-empty line should be parseable as a number.
    let first = stdout.trim().lines().next().unwrap_or("");
    assert!(first.parse::<u64>().is_ok(), "expected number, got: {first}");
}

#[test]
fn scan_limit_zero_produces_no_rows() {
    let dir = setup_repo();
    let out = Command::new(bin())
        .args(["scan", "--min-size", "100K", "--limit", "0", "--path"])
        .arg(dir.path())
        .output()
        .expect("run mara");
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(!stdout.contains("big.bin"));
}
