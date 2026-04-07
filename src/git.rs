use anyhow::{anyhow, Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub struct Blob {
    pub oid: String,
    pub size: u64,
    pub path: String,
}

#[derive(Debug, Clone)]
pub struct BlobReport {
    pub oid: String,
    pub size: u64,
    pub path: String,
    pub commit: Option<String>,
    pub date: Option<String>,
    pub author: Option<String>,
}

/// Verify the path is inside a git repository.
pub fn ensure_repo(path: &Path) -> Result<PathBuf> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow!("git command not found — please install git")
            } else {
                anyhow!("failed to run git: {}", e)
            }
        })?;
    if !output.status.success() {
        return Err(anyhow!("not inside a git repository: {}", path.display()));
    }
    let top = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(PathBuf::from(top))
}

/// Walk all reachable git objects and return blob entries (oid, size, path).
pub fn list_blobs(repo: &Path) -> Result<Vec<Blob>> {
    // git rev-list --objects --all → "<oid> [path]"
    let mut rev = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-list", "--objects", "--all"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("spawning git rev-list")?;

    let rev_stdout = rev
        .stdout
        .take()
        .ok_or_else(|| anyhow!("could not capture git rev-list output"))?;

    let cat = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "cat-file",
            "--batch-check=%(objecttype) %(objectname) %(objectsize) %(rest)",
        ])
        .stdin(Stdio::from(rev_stdout))
        .output()
        .context("running git cat-file")?;

    // Reap rev-list so it doesn't linger as a zombie, and surface its errors.
    let rev_status = rev.wait().context("waiting on git rev-list")?;
    if !rev_status.success() {
        let mut err = String::new();
        if let Some(mut stderr) = rev.stderr.take() {
            stderr.read_to_string(&mut err).ok();
        }
        let err = err.trim();
        if err.is_empty() {
            return Err(anyhow!("git rev-list failed"));
        }
        return Err(anyhow!("git rev-list failed: {}", err));
    }

    if !cat.status.success() {
        return Err(anyhow!(
            "git cat-file failed: {}",
            String::from_utf8_lossy(&cat.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&cat.stdout);
    Ok(parse_cat_file(&stdout))
}

/// Parse `git cat-file --batch-check` output.
pub fn parse_cat_file(text: &str) -> Vec<Blob> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut parts = line.splitn(4, ' ');
        let kind = match parts.next() {
            Some(k) => k,
            None => continue,
        };
        if kind != "blob" {
            continue;
        }
        let oid = match parts.next() {
            Some(o) => o.to_string(),
            None => continue,
        };
        let size: u64 = match parts.next().and_then(|s| s.parse().ok()) {
            Some(n) => n,
            None => continue,
        };
        let path = parts.next().unwrap_or("").to_string();
        out.push(Blob { oid, size, path });
    }
    out
}

/// Filter blobs by min size, sort descending, dedupe by oid keeping largest path,
/// and limit to top N.
pub fn top_blobs(mut blobs: Vec<Blob>, min_size: u64, limit: usize) -> Vec<Blob> {
    blobs.retain(|b| b.size >= min_size);
    // Dedupe by oid: keep first occurrence with a non-empty path if possible.
    blobs.sort_by(|a, b| b.size.cmp(&a.size));
    let mut seen = std::collections::HashSet::new();
    let mut deduped = Vec::new();
    for b in blobs.into_iter() {
        if seen.insert(b.oid.clone()) {
            deduped.push(b);
        }
    }
    deduped.truncate(limit);
    deduped
}

/// Look up the first commit that introduced a given path.
pub fn first_commit_for_path(repo: &Path, path: &str) -> Option<(String, String, String)> {
    if path.is_empty() {
        return None;
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "log",
            "--all",
            "--diff-filter=A",
            "--format=%h%x09%ai%x09%an",
            "--",
            path,
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let last = text.lines().last()?;
    let mut parts = last.splitn(3, '\t');
    let sha = parts.next()?.to_string();
    let date = parts.next()?.to_string();
    let author = parts.next()?.to_string();
    Some((sha, date, author))
}

pub fn enrich(repo: &Path, blobs: Vec<Blob>) -> Vec<BlobReport> {
    blobs
        .into_iter()
        .map(|b| {
            let info = first_commit_for_path(repo, &b.path);
            BlobReport {
                oid: b.oid,
                size: b.size,
                path: b.path,
                commit: info.as_ref().map(|i| i.0.clone()),
                date: info.as_ref().map(|i| i.1.clone()),
                author: info.as_ref().map(|i| i.2.clone()),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_blob_lines() {
        let text = "blob abc123 1024 path/to/file.bin\nblob def456 2048 other.dat\ncommit aaa 100 \ntree bbb 50 \n";
        let blobs = parse_cat_file(text);
        assert_eq!(blobs.len(), 2);
        assert_eq!(blobs[0].oid, "abc123");
        assert_eq!(blobs[0].size, 1024);
        assert_eq!(blobs[0].path, "path/to/file.bin");
        assert_eq!(blobs[1].path, "other.dat");
    }

    #[test]
    fn ignores_non_blobs() {
        let text = "commit abc 100 something\ntree def 50 \n";
        assert!(parse_cat_file(text).is_empty());
    }

    #[test]
    fn filters_and_sorts() {
        let blobs = vec![
            Blob {
                oid: "a".into(),
                size: 100,
                path: "a".into(),
            },
            Blob {
                oid: "b".into(),
                size: 5000,
                path: "b".into(),
            },
            Blob {
                oid: "c".into(),
                size: 200,
                path: "c".into(),
            },
            Blob {
                oid: "b".into(),
                size: 5000,
                path: "b2".into(),
            },
        ];
        let top = top_blobs(blobs, 150, 10);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].oid, "b");
        assert_eq!(top[0].size, 5000);
        assert_eq!(top[1].oid, "c");
    }

    #[test]
    fn applies_limit() {
        let blobs = (0..10)
            .map(|i| Blob {
                oid: format!("o{i}"),
                size: 1000 + i,
                path: format!("p{i}"),
            })
            .collect();
        let top = top_blobs(blobs, 0, 3);
        assert_eq!(top.len(), 3);
        assert_eq!(top[0].size, 1009);
    }

    #[test]
    fn limit_larger_than_len() {
        let blobs = vec![Blob {
            oid: "a".into(),
            size: 100,
            path: "a".into(),
        }];
        let top = top_blobs(blobs, 0, 50);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn parses_blob_paths_with_spaces() {
        let text = "blob abc123 1024 path with spaces/file name.bin\n";
        let blobs = parse_cat_file(text);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].path, "path with spaces/file name.bin");
    }

    #[test]
    fn parses_empty_path_blob() {
        let text = "blob abc123 1024 \n";
        let blobs = parse_cat_file(text);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].path, "");
    }

    #[test]
    fn ignores_missing_object_lines() {
        // cat-file emits "<input> missing" for unknown objects
        let text = "deadbeef missing\nblob abc 100 f\n";
        let blobs = parse_cat_file(text);
        assert_eq!(blobs.len(), 1);
        assert_eq!(blobs[0].oid, "abc");
    }

    #[test]
    fn dedupes_by_oid_keeping_largest_first() {
        let blobs = vec![
            Blob {
                oid: "x".into(),
                size: 100,
                path: "short".into(),
            },
            Blob {
                oid: "x".into(),
                size: 100,
                path: "longer-path".into(),
            },
        ];
        let top = top_blobs(blobs, 0, 10);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn first_commit_for_empty_path_returns_none() {
        // Use current repo (we're inside one during tests)
        let repo = Path::new(".");
        assert!(first_commit_for_path(repo, "").is_none());
    }
}
