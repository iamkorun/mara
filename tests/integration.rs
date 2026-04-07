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
