<p align="center">
  <h1 align="center">mara 🐾</h1>
  <p align="center">Find exactly what's bloating your git history</p>
</p>

<p align="center">
  <a href="https://github.com/iamkorun/mara/actions/workflows/ci.yml"><img src="https://github.com/iamkorun/mara/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://crates.io/crates/mara"><img src="https://img.shields.io/crates/v/mara.svg" alt="crates.io"></a>
  <a href="https://github.com/iamkorun/mara/blob/main/LICENSE"><img src="https://img.shields.io/badge/License-MIT-yellow.svg" alt="License: MIT"></a>
  <a href="https://github.com/iamkorun/mara/stargazers"><img src="https://img.shields.io/github/stars/iamkorun/mara.svg?style=social" alt="GitHub stars"></a>
  <a href="https://buymeacoffee.com/iamkorun"><img src="https://img.shields.io/badge/Buy%20Me%20a%20Coffee-ffdd00?logo=buy-me-a-coffee&logoColor=black" alt="Buy Me a Coffee"></a>
</p>

---

<!-- TODO: Add demo GIF -->
![demo](docs/demo.gif)

## The Problem

Git repos quietly accumulate bloat. A binary sneaks into a commit. A `node_modules/` folder gets staged by mistake. A 50MB dataset lands in the history and never leaves — even after you delete the file.

Over time `.git` grows to hundreds of MB and every `git clone` or `git fetch` slows to a crawl. Finding *which files* caused the bloat requires obscure `git rev-list | git cat-file` plumbing incantations that most developers don't know and don't want to learn.

## The Solution

**mara** is a zero-config single-binary CLI that walks all git objects in your repository and reports the largest blobs — with the exact file path, size, the commit that introduced it, and a ready-to-run `git filter-repo` cleanup command.

Named after the [Patagonian mara](https://en.wikipedia.org/wiki/Patagonian_mara) — a burrowing animal that digs through open terrain to find exactly what it's looking for.

**How mara fills the gap:**

| Tool | Purpose | File-level detail? |
|------|---------|-------------------|
| `git-sizer` | Aggregate stats | ✗ No |
| `BFG Repo Cleaner` | Java, removal-focused | ✗ No |
| `git filter-repo` | Python, cleanup tool | ✗ No |
| **mara** | **Find AND report** | **✅ Yes** |

## Demo

```
$ mara scan
SIZE      SHA       DATE        FILE
  12.3MB  a040d9d   2026-03-12  assets/raw/video.mov
   4.8MB  7dfab63   2026-02-01  dist/bundle.min.js
   2.1MB  277e3ee   2026-01-15  node_modules/@types/large-pkg/big.d.ts
 890.4KB  b3c1d22   2025-11-08  vendor/ffmpeg-static/bin/ffmpeg
 412.0KB  e91f5a8   2025-09-30  test/fixtures/sample-data.csv
...
Scanned 4,821 objects in 0.3s

$ mara suggest --limit 3
# ⚠️  These commands REWRITE HISTORY. Back up your repo first.
# Install: pip install git-filter-repo
git filter-repo --path assets/raw/video.mov --invert-paths
git filter-repo --path dist/bundle.min.js --invert-paths
git filter-repo --path 'node_modules/@types/large-pkg/big.d.ts' --invert-paths
```

## Quick Start

```sh
cargo install mara
cd your-repo && mara scan
```

## Installation

### From crates.io (recommended)

```sh
cargo install mara
```

### From source

```sh
git clone https://github.com/iamkorun/mara.git
cd mara
cargo install --path .
```

### Pre-built binaries

Pre-built binaries for Linux, macOS, and Windows are **coming soon** via GitHub Releases.

### Requirements

- Rust 1.70+ (for `cargo install`)
- `git` on your PATH

## Usage

### `mara scan` — find large blobs

```sh
# Scan current repo (shows top 20 blobs > 100KB)
mara scan

# Only show blobs larger than 1MB
mara scan --min-size 1M

# Show top 10 results
mara scan --limit 10

# Scan a specific repo path
mara scan --path /path/to/repo

# Combine flags
mara scan --min-size 500K --limit 5 --path /path/to/repo
```

**Sample output:**

```
SIZE      SHA       DATE        FILE
  12.3MB  a040d9d   2026-03-12  assets/raw/video.mov
   4.8MB  7dfab63   2026-02-01  dist/bundle.min.js
   2.1MB  277e3ee   2026-01-15  node_modules/@types/large-pkg/big.d.ts
 890.4KB  b3c1d22   2025-11-08  vendor/ffmpeg-static/bin/ffmpeg
 412.0KB  e91f5a8   2025-09-30  test/fixtures/sample-data.csv
...
Scanned 4,821 objects in 0.3s
```

### `mara stat` — .git size breakdown

```sh
mara stat

# Scan a different repo
mara stat --path /path/to/repo
```

**Sample output:**

```
.git total:      48.2 MB
  objects:       41.1 MB
  pack:          40.8 MB
  index:         243 KB
working tree:    8.7 MB
```

### `mara suggest` — generate cleanup commands

```sh
# Get filter-repo commands for top 20 bloaters
mara suggest

# Limit to top 5, only files > 1MB
mara suggest --limit 5 --min-size 1M

# Scan a different repo
mara suggest --path /path/to/repo
```

**Sample output:**

```
# ⚠️  These commands REWRITE HISTORY. Back up your repo first.
# Install: pip install git-filter-repo
git filter-repo --path assets/raw/video.mov --invert-paths
git filter-repo --path dist/bundle.min.js --invert-paths
git filter-repo --path 'node_modules/@types/large-pkg/big.d.ts' --invert-paths
```

> Paths with spaces are automatically shell-escaped in suggest output.

## How It Works

mara runs `git rev-list --objects --all` piped into `git cat-file --batch-check` to enumerate every object in the git history and classify blobs by size. Blobs are deduplicated by OID so the same large file appearing in multiple commits counts once. For each top blob, mara resolves the file path using `git log --diff-filter=A` to find the commit that first introduced it. The result is sorted descending by size and printed in a clean table. Zero external dependencies beyond `clap` and `anyhow`.

## Cleanup Workflow

Once you know what to remove:

1. Run `mara suggest` to get ready-to-run filter-repo commands
2. Back up your repo: `cp -r your-repo your-repo-backup`
3. Install git-filter-repo: `pip install git-filter-repo`
4. Run the suggested commands one at a time
5. Force-push the cleaned history: `git push --force-with-lease`

> ⚠️ **Warning:** `git filter-repo` rewrites git history. All collaborators must re-clone after cleanup. Coordinate with your team before running.

## Features

- **Zero config** — run it in any git repo, no setup required
- **Single Rust binary** — no runtime deps, just `cargo install` and go
- **Blob-level detail** — exact file paths, not aggregate stats
- **Deduplicates by OID** — same blob in many commits counted once
- **Shell-safe output** — paths with spaces are properly escaped in `suggest`
- **Friendly errors** — clear messages on missing git, empty repos, or bad paths
- **Fast** — scans thousands of objects in under a second

## Contributing

Contributions are welcome! Please open an issue first to discuss larger changes.

```sh
git clone https://github.com/iamkorun/mara.git
cd mara
cargo test
```

## License

[MIT](LICENSE)

---

## Star History

<a href="https://star-history.com/#iamkorun/mara&Date">
  <img src="https://api.star-history.com/svg?repos=iamkorun/mara&type=Date" alt="Star History Chart" width="600">
</a>

---

<p align="center">
  <a href="https://buymeacoffee.com/iamkorun"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me a Coffee" width="200"></a>
</p>
