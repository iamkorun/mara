use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct GitStats {
    pub git_dir: PathBuf,
    pub total: u64,
    pub loose_objects: u64,
    pub pack_files: u64,
    pub index: u64,
    pub working_tree: u64,
}

fn dir_size(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(p) = stack.pop() {
        let entries = match std::fs::read_dir(&p) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            match entry.file_type() {
                Ok(ft) if ft.is_dir() => stack.push(path),
                Ok(ft) if ft.is_file() => {
                    if let Ok(meta) = entry.metadata() {
                        total += meta.len();
                    }
                }
                _ => {}
            }
        }
    }
    total
}

pub fn collect(repo: &Path) -> Result<GitStats> {
    let git_dir = repo.join(".git");
    let git_dir = if git_dir.is_dir() {
        git_dir
    } else {
        // worktree or bare; use git rev-parse
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["rev-parse", "--git-dir"])
            .output()
            .context("running git rev-parse --git-dir")?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let p = PathBuf::from(&s);
        if p.is_absolute() {
            p
        } else {
            repo.join(p)
        }
    };

    let total = dir_size(&git_dir);
    let objects = git_dir.join("objects");
    let pack = objects.join("pack");
    let pack_size = dir_size(&pack);
    let objects_total = dir_size(&objects);
    let loose = objects_total.saturating_sub(pack_size);
    let index = std::fs::metadata(git_dir.join("index"))
        .map(|m| m.len())
        .unwrap_or(0);

    let working = dir_size(repo).saturating_sub(total);

    Ok(GitStats {
        git_dir,
        total,
        loose_objects: loose,
        pack_files: pack_size,
        index,
        working_tree: working,
    })
}
