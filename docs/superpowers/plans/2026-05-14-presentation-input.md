# Presentation Module — Sub-Plan 2a: Security + Input Processing

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement SSRF-safe URL validation, sandboxed git ingestion, and input processors for text/PDF/DOCX/XLSX/images/URLs/git repos that convert any input into a unified text corpus.

**Architecture:** All input sources implement a common processing pattern: validate → extract text → return String. The InputSource enum is the contract between the UI and the AI pipeline. Security is handled at the boundary (SSRF guard, git sandbox) before any external fetch.

**Tech Stack:** Rust, reqwest, lopdf, pulldown-cmark, calamine, git2, tempfile, tokio, url crate, minion-llm (for image vision).

---

## File Map

### Created
| File | Responsibility |
|---|---|
| `crates/minion-presentation/src/security/mod.rs` | Re-export `ssrf_guard` and `git_sandbox` |
| `crates/minion-presentation/src/security/ssrf_guard.rs` | URL validation + private IP blocking |
| `crates/minion-presentation/src/security/git_sandbox.rs` | Sandboxed shallow git clone + summary |
| `crates/minion-presentation/src/input/mod.rs` | `InputSource` enum + `process_all()` orchestrator |
| `crates/minion-presentation/src/input/text.rs` | Plain text passthrough |
| `crates/minion-presentation/src/input/document.rs` | PDF / DOCX / MD / XLSX extractors |
| `crates/minion-presentation/src/input/image.rs` | Image → base64 → vision LLM description |
| `crates/minion-presentation/src/input/url.rs` | SSRF-guarded HTTP fetch + text extraction |
| `crates/minion-presentation/src/input/git.rs` | Git-sandbox wrapper for `InputSource::GitUrl` |
| `crates/minion-presentation/tests/input_tests.rs` | Unit tests for all input processors |
| `crates/minion-presentation/tests/security_tests.rs` | SSRF guard and git sandbox tests |

### Modified
| File | Change |
|---|---|
| `crates/minion-presentation/src/lib.rs` | Add `pub mod security; pub mod input;` |
| `crates/minion-presentation/Cargo.toml` | Add `url = "2.5"` dependency |

---

## Constraints and Size Limits

| Limit | Value |
|---|---|
| Max file size (PDF/DOCX/XLSX/image) | 25 MB |
| Max URL response body | 10 MB |
| Max git repo after clone | 100 MB |
| Max git repo `.git` dir | 50 MB |
| Max redirect count | 3 |
| XLSX rows per sheet | first 200 rows |
| Config files read (git sandbox) | first 200 lines each |

---

## Task 1: SSRF Guard

**Files:**
- Create: `crates/minion-presentation/src/security/mod.rs`
- Create: `crates/minion-presentation/src/security/ssrf_guard.rs`
- Create: `crates/minion-presentation/tests/security_tests.rs`
- Modify: `crates/minion-presentation/Cargo.toml` (add `url` dep)
- Modify: `crates/minion-presentation/src/lib.rs` (add `pub mod security;`)

### Step 1: Add `url` crate dependency

In `crates/minion-presentation/Cargo.toml`, add under `[dependencies]`:

```toml
# URL parsing + validation (SSRF guard)
url = "2.5"
```

### Step 2: Write failing tests first

Create `crates/minion-presentation/tests/security_tests.rs`:

```rust
//! Security layer tests — SSRF guard and git sandbox.

use minion_presentation::security::ssrf_guard::validate_url;

// ── SSRF guard tests ──────────────────────────────────────────────────────────

#[test]
fn accepts_valid_public_https() {
    let result = validate_url("https://example.com/page");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let u = result.unwrap();
    assert_eq!(u.scheme(), "https");
}

#[test]
fn accepts_valid_public_http() {
    let result = validate_url("http://example.com/page");
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
}

#[test]
fn rejects_ftp_scheme() {
    let result = validate_url("ftp://example.com/file.txt");
    assert!(result.is_err());
    let msg = result.unwrap_err();
    assert!(msg.contains("scheme"), "error should mention scheme: {msg}");
}

#[test]
fn rejects_file_scheme() {
    let result = validate_url("file:///etc/passwd");
    assert!(result.is_err());
}

#[test]
fn rejects_localhost_loopback() {
    // localhost normally resolves to 127.0.0.1 — blocked by 127.0.0.0/8
    let result = validate_url("http://localhost/admin");
    assert!(result.is_err(), "localhost must be blocked");
}

#[test]
fn rejects_127_direct() {
    let result = validate_url("http://127.0.0.1/anything");
    assert!(result.is_err());
}

#[test]
fn rejects_private_10_block() {
    let result = validate_url("http://10.0.0.1/internal");
    assert!(result.is_err());
}

#[test]
fn rejects_private_192_168_block() {
    let result = validate_url("http://192.168.1.100/router");
    assert!(result.is_err());
}

#[test]
fn rejects_private_172_16_block() {
    let result = validate_url("http://172.16.5.10/service");
    assert!(result.is_err());
}

#[test]
fn rejects_link_local_169_254() {
    let result = validate_url("http://169.254.169.254/metadata");
    assert!(result.is_err(), "cloud metadata endpoint must be blocked");
}

#[test]
fn rejects_invalid_url_string() {
    let result = validate_url("not a url at all");
    assert!(result.is_err());
}

#[test]
fn rejects_empty_string() {
    let result = validate_url("");
    assert!(result.is_err());
}

#[test]
fn max_redirect_constant_is_three() {
    assert_eq!(minion_presentation::security::ssrf_guard::MAX_REDIRECTS, 3);
}
```

### Step 3: Implement the SSRF guard

Create `crates/minion-presentation/src/security/ssrf_guard.rs`:

```rust
//! SSRF guard — validates that a URL is safe to fetch.
//!
//! Rules enforced:
//! 1. Scheme must be `http` or `https`.
//! 2. The hostname must resolve via DNS and none of the resolved IPs may be in
//!    a private/loopback/link-local range.
//! 3. Callers that follow redirects MUST check [`MAX_REDIRECTS`] themselves.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};

/// Maximum number of HTTP redirects callers are permitted to follow.
pub const MAX_REDIRECTS: usize = 3;

/// Validate `url` for SSRF safety.
///
/// Returns the parsed [`url::Url`] on success or a human-readable error string
/// on failure.  The returned URL has its scheme, host, and path intact — callers
/// can use it directly with `reqwest`.
pub fn validate_url(raw: &str) -> Result<url::Url, String> {
    // ── 1. Parse ──────────────────────────────────────────────────────────────
    let parsed = url::Url::parse(raw).map_err(|e| format!("Invalid URL: {e}"))?;

    // ── 2. Scheme whitelist ───────────────────────────────────────────────────
    match parsed.scheme() {
        "http" | "https" => {}
        s => return Err(format!("Disallowed scheme '{s}': only http/https are permitted")),
    }

    // ── 3. Resolve hostname ───────────────────────────────────────────────────
    let host = parsed.host_str().ok_or_else(|| "URL has no host".to_string())?;
    let port = parsed.port_or_known_default().unwrap_or(80);

    // `ToSocketAddrs` performs a blocking DNS lookup — acceptable here because
    // `validate_url` is called in a `spawn_blocking` context from async code.
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|e| format!("DNS resolution failed for '{host}': {e}"))?;

    for socket_addr in addrs {
        let ip = socket_addr.ip();
        if is_private_or_loopback(ip) {
            return Err(format!(
                "Host '{host}' resolves to private/loopback address {ip} — blocked by SSRF guard"
            ));
        }
    }

    Ok(parsed)
}

/// Returns `true` for any IP that must not be contacted by server-side fetches:
/// loopback, private ranges, link-local, and IPv6 unique-local / link-local.
fn is_private_or_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => is_private_v4(v4),
        IpAddr::V6(v6) => is_private_v6(v6),
    }
}

fn is_private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback()                           // 127.0.0.0/8
        || ip.is_private()                     // 10/8, 172.16/12, 192.168/16
        || ip.is_link_local()                  // 169.254.0.0/16
        || ip.is_broadcast()
        || ip.is_documentation()
        || ip.is_unspecified()
        // Carrier-grade NAT (100.64.0.0/10) — also blocked
        || (octets[0] == 100 && (octets[1] & 0xC0) == 64)
}

fn is_private_v6(ip: Ipv6Addr) -> bool {
    ip.is_loopback()    // ::1
        // Unique-local fc00::/7
        || (ip.segments()[0] & 0xFE00) == 0xFC00
        // Link-local fe80::/10
        || (ip.segments()[0] & 0xFFC0) == 0xFE80
        || ip.is_unspecified()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_v4_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
    }

    #[test]
    fn private_10_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(10, 1, 2, 3))));
    }

    #[test]
    fn private_172_16_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
        // 172.32.x.x is NOT private
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(172, 32, 0, 1))));
    }

    #[test]
    fn private_192_168_block_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
    }

    #[test]
    fn link_local_169_254_is_private() {
        assert!(is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(169, 254, 169, 254))));
    }

    #[test]
    fn public_ip_is_not_private() {
        // 8.8.8.8 = Google DNS — public
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        // 1.1.1.1 = Cloudflare DNS — public
        assert!(!is_private_or_loopback(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn ipv6_loopback_is_private() {
        assert!(is_private_or_loopback(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn ipv6_unique_local_fc_is_private() {
        // fc00::1 — unique-local
        let ip: Ipv6Addr = "fc00::1".parse().unwrap();
        assert!(is_private_or_loopback(IpAddr::V6(ip)));
    }

    #[test]
    fn ipv6_link_local_fe80_is_private() {
        let ip: Ipv6Addr = "fe80::1".parse().unwrap();
        assert!(is_private_or_loopback(IpAddr::V6(ip)));
    }
}
```

Create `crates/minion-presentation/src/security/mod.rs`:

```rust
//! Security boundary for the presentation input pipeline.
//!
//! All external fetches (HTTP and git) MUST pass through this module before
//! any network I/O occurs.

pub mod git_sandbox;
pub mod ssrf_guard;
```

### Step 4: Wire lib.rs

In `crates/minion-presentation/src/lib.rs`, add:

```rust
pub mod security;
```

So the file becomes:

```rust
pub mod db;
pub mod migrations;
pub mod schema;
pub mod security;

pub use schema::types::*;
```

### Step 5: Verify

```bash
cargo test -p minion-presentation --test security_tests 2>&1 | head -40
cargo test -p minion-presentation security::ssrf_guard 2>&1 | head -40
```

All tests in `security_tests.rs` and the inline `ssrf_guard::tests` module must pass.

### Step 6: Commit

```bash
git add crates/minion-presentation/src/security/ \
        crates/minion-presentation/src/lib.rs \
        crates/minion-presentation/Cargo.toml \
        crates/minion-presentation/tests/security_tests.rs
git commit -m "feat(presentation): add SSRF guard with private IP blocking"
```

---

## Task 2: Git Sandbox

**Files:**
- Create: `crates/minion-presentation/src/security/git_sandbox.rs`

The git sandbox depends on `ssrf_guard::validate_url` from Task 1.

### Step 1: Write failing tests

Add the following to `crates/minion-presentation/tests/security_tests.rs` (append after existing tests):

```rust
// ── Git sandbox tests ─────────────────────────────────────────────────────────

use minion_presentation::security::git_sandbox::summarize_git_repo;

#[tokio::test]
async fn rejects_private_git_url() {
    // The SSRF guard must fire before any network I/O.
    let result = summarize_git_repo("http://192.168.1.1/repo.git").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("SSRF") || msg.contains("private") || msg.contains("blocked"),
        "expected SSRF error, got: {msg}"
    );
}

#[tokio::test]
async fn rejects_file_scheme_git_url() {
    let result = summarize_git_repo("file:///tmp/myrepo").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn rejects_localhost_git_url() {
    let result = summarize_git_repo("http://localhost:8080/repo.git").await;
    assert!(result.is_err());
}

/// Smoke test: clone a tiny real public repository.
/// Skipped in CI unless PRESENTATION_INTEGRATION_TESTS=1 is set.
#[tokio::test]
async fn clones_public_repo_and_returns_summary() {
    if std::env::var("PRESENTATION_INTEGRATION_TESTS").is_err() {
        return; // skip
    }
    // Use a known tiny repo
    let result = summarize_git_repo("https://github.com/nicowillis/colors").await;
    assert!(result.is_ok(), "clone failed: {:?}", result);
    let summary = result.unwrap();
    assert!(summary.contains("# Repository:"), "summary missing header: {summary}");
}
```

### Step 2: Implement the git sandbox

Create `crates/minion-presentation/src/security/git_sandbox.rs`:

```rust
//! Sandboxed git repository ingestion.
//!
//! # Safety guarantees
//! - The remote URL is validated by [`ssrf_guard::validate_url`] before any
//!   network I/O is attempted.
//! - The clone lands in a [`tempfile::TempDir`] that is automatically deleted
//!   when the function returns (success or error).
//! - After cloning, the total size of the `.git` directory is checked; if it
//!   exceeds [`GIT_DIR_MAX_BYTES`] the clone is abandoned and an error is
//!   returned.
//! - Only a known allowlist of files is read; `.git/config` and any credential
//!   files are never opened.

use std::path::Path;

use anyhow::{bail, Context};

use crate::security::ssrf_guard;

/// Maximum permitted size of the `.git` directory after cloning (50 MB).
pub const GIT_DIR_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// Maximum total repository size on disk after cloning (100 MB).
pub const REPO_MAX_BYTES: u64 = 100 * 1024 * 1024;

/// Maximum number of lines read from any single configuration/manifest file.
const MAX_LINES_PER_FILE: usize = 200;

/// Validate, clone, inspect, and summarise a remote git repository.
///
/// The temporary directory is created and destroyed within this call. The
/// returned `String` is a structured text summary suitable for feeding to the
/// LLM research agent.
pub async fn summarize_git_repo(remote_url: &str) -> anyhow::Result<String> {
    // ── 1. SSRF guard (blocking DNS lookup → run on thread pool) ─────────────
    let remote_url = remote_url.to_owned();
    let validated_url = tokio::task::spawn_blocking({
        let url = remote_url.clone();
        move || ssrf_guard::validate_url(&url).map_err(|e| anyhow::anyhow!("SSRF guard: {e}"))
    })
    .await
    .context("spawn_blocking panicked")??;

    let url_str = validated_url.as_str().to_owned();

    // ── 2. Clone into a temp dir (blocking) ──────────────────────────────────
    let summary = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let tmp = tempfile::TempDir::new().context("failed to create temp dir")?;
        let tmp_path = tmp.path().to_owned();

        // Use a plain clone (git2 does not expose shallow depth natively
        // without raw fetch config; a shallow clone requires negotiation
        // that git2's high-level API does not support via RepoBuilder).
        // We clone and then immediately check size limits to stay safe.
        let _repo = git2::Repository::clone(&url_str, &tmp_path)
            .context("git clone failed")?;

        // ── 3. Size guard ─────────────────────────────────────────────────────
        let git_dir = tmp_path.join(".git");
        let git_size = dir_size_bytes(&git_dir)?;
        if git_size > GIT_DIR_MAX_BYTES {
            bail!(
                "git sandbox: .git directory is {git_size} bytes (limit: {GIT_DIR_MAX_BYTES})"
            );
        }
        let total_size = dir_size_bytes(&tmp_path)?;
        if total_size > REPO_MAX_BYTES {
            bail!(
                "git sandbox: repository is {total_size} bytes total (limit: {REPO_MAX_BYTES})"
            );
        }

        // ── 4. Extract repo name from URL ─────────────────────────────────────
        let repo_name = url_str
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        // ── 5. Read allowlisted files ─────────────────────────────────────────
        let readme_text = read_readme(&tmp_path);
        let manifest_text = read_manifests(&tmp_path);
        let structure_text = top_level_structure(&tmp_path);

        // ── 6. Build structured summary ───────────────────────────────────────
        let mut out = String::new();
        out.push_str(&format!("# Repository: {repo_name}\n\n"));
        out.push_str("## Structure\n\n");
        out.push_str(&structure_text);
        if !manifest_text.is_empty() {
            out.push_str("\n\n## Manifest Files\n\n");
            out.push_str(&manifest_text);
        }
        if !readme_text.is_empty() {
            out.push_str("\n\n## README\n\n");
            out.push_str(&readme_text);
        }

        // tmp is dropped here, deleting the clone
        drop(tmp);
        Ok(out)
    })
    .await
    .context("spawn_blocking panicked")??;

    Ok(summary)
}

/// Recursively sum the size of all files under `dir`.
fn dir_size_bytes(dir: &Path) -> anyhow::Result<u64> {
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

/// Read the first 200 lines of the primary README file (case-insensitive).
fn read_readme(root: &Path) -> String {
    for name in &["README.md", "readme.md", "README.rst", "README.txt", "README"] {
        let p = root.join(name);
        if p.exists() && p.is_file() {
            return read_first_n_lines(&p, MAX_LINES_PER_FILE);
        }
    }
    String::new()
}

/// Read the first 200 lines of each recognised manifest file.
fn read_manifests(root: &Path) -> String {
    let candidates = [
        "Cargo.toml",
        "package.json",
        "pyproject.toml",
        "setup.py",
        "go.mod",
        "pom.xml",
        "build.gradle",
    ];
    let mut parts = Vec::new();
    for name in &candidates {
        let p = root.join(name);
        if p.exists() && p.is_file() {
            let content = read_first_n_lines(&p, MAX_LINES_PER_FILE);
            parts.push(format!("### {name}\n\n```\n{content}\n```"));
        }
    }
    parts.join("\n\n")
}

/// Build a short textual listing of top-level items in the repo.
fn top_level_structure(root: &Path) -> String {
    let mut dirs = Vec::new();
    let mut files = Vec::new();

    let Ok(entries) = std::fs::read_dir(root) else {
        return "(could not list directory)".to_string();
    };

    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs and the .git directory itself
        if name.starts_with('.') {
            continue;
        }
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(format!("  {name}/"));
        } else {
            files.push(format!("  {name}"));
        }
    }

    dirs.sort();
    files.sort();

    let mut out = String::from("```\n");
    for d in &dirs {
        out.push_str(d);
        out.push('\n');
    }
    for f in &files {
        out.push_str(f);
        out.push('\n');
    }
    out.push_str("```");
    out
}

/// Read at most `n` lines from a file, skipping binary content.
/// Returns an empty string if the file cannot be read or is binary.
fn read_first_n_lines(path: &Path, n: usize) -> String {
    // Guard: do not read `.git/config` or credential files
    let path_str = path.to_string_lossy();
    if path_str.contains(".git/config")
        || path_str.contains(".netrc")
        || path_str.contains(".ssh/")
    {
        return String::new();
    }

    let Ok(content) = std::fs::read(path) else {
        return String::new();
    };

    // Heuristic: if more than 5% of the first 1 KB is non-UTF8, treat as binary
    let sample = &content[..content.len().min(1024)];
    let non_utf8_count = sample.iter().filter(|&&b| b == 0 || b > 127).count();
    if non_utf8_count > sample.len() / 20 {
        return String::new();
    }

    let text = String::from_utf8_lossy(&content);
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}
```

Note: `walkdir` is already a workspace dependency (`walkdir = "2.4"`). The git sandbox uses it via `walkdir::WalkDir`. No new dependencies needed.

### Step 3: Verify

```bash
cargo test -p minion-presentation --test security_tests rejects_private_git_url 2>&1
cargo test -p minion-presentation --test security_tests rejects_file_scheme_git_url 2>&1
cargo test -p minion-presentation --test security_tests rejects_localhost_git_url 2>&1
cargo clippy -p minion-presentation -- -D warnings 2>&1 | head -30
```

### Step 4: Commit

```bash
git add crates/minion-presentation/src/security/git_sandbox.rs \
        crates/minion-presentation/tests/security_tests.rs
git commit -m "feat(presentation): add sandboxed git repo ingestion with size guards"
```

---

## Task 3: Text + Markdown + XLSX Input Processors

**Files:**
- Create: `crates/minion-presentation/src/input/text.rs`
- Create: `crates/minion-presentation/src/input/document.rs` (MD + XLSX portions)
- Create: `crates/minion-presentation/tests/input_tests.rs`

These processors have no external network calls and no dependencies on Tasks 4 or 5.

### Step 1: Write failing tests first

Create `crates/minion-presentation/tests/input_tests.rs`:

```rust
//! Unit tests for the input processing pipeline.

// ── Text passthrough ──────────────────────────────────────────────────────────

#[test]
fn text_passthrough_returns_content() {
    let result = minion_presentation::input::text::process_text("hello world");
    assert_eq!(result, "hello world");
}

#[test]
fn text_passthrough_preserves_whitespace() {
    let content = "line 1\nline 2\n  indented";
    let result = minion_presentation::input::text::process_text(content);
    assert_eq!(result, content);
}

// ── Markdown ──────────────────────────────────────────────────────────────────

#[test]
fn markdown_passthrough_returns_raw_source() {
    let md = "# Title\n\nSome **bold** text.";
    let result = minion_presentation::input::document::process_markdown(md);
    assert_eq!(result, md);
}

// ── XLSX ──────────────────────────────────────────────────────────────────────

use std::io::Write;

#[test]
fn xlsx_rejects_oversized_file() {
    // Create a temp file that exceeds 25 MB
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    // Write 26 MB of zeros (not a valid XLSX but size check happens first)
    let big_data = vec![0u8; 26 * 1024 * 1024];
    tmp.write_all(&big_data).unwrap();
    tmp.flush().unwrap();

    let result =
        minion_presentation::input::document::process_xlsx(tmp.path().to_str().unwrap());
    assert!(result.is_err(), "expected size error, got: {:?}", result);
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

#[test]
fn xlsx_rejects_nonexistent_file() {
    let result = minion_presentation::input::document::process_xlsx("/nonexistent/path/data.xlsx");
    assert!(result.is_err());
}

// ── PDF size guard (no real PDF needed) ──────────────────────────────────────

#[test]
fn pdf_rejects_oversized_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let big_data = vec![0u8; 26 * 1024 * 1024];
    tmp.write_all(&big_data).unwrap();
    tmp.flush().unwrap();

    let result =
        minion_presentation::input::document::process_pdf(tmp.path().to_str().unwrap());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

// ── DOCX size guard ───────────────────────────────────────────────────────────

#[test]
fn docx_rejects_oversized_file() {
    let mut tmp = tempfile::NamedTempFile::new().unwrap();
    let big_data = vec![0u8; 26 * 1024 * 1024];
    tmp.write_all(&big_data).unwrap();
    tmp.flush().unwrap();

    let result =
        minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}
```

### Step 2: Implement text processor

Create `crates/minion-presentation/src/input/text.rs`:

```rust
//! Plain text input processor — passthrough with no transformation.

/// Process a plain-text input source.
///
/// The text is returned verbatim; the caller is responsible for chunking if
/// it exceeds LLM context limits.
pub fn process_text(content: &str) -> String {
    content.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_unchanged() {
        assert_eq!(process_text("abc"), "abc");
    }

    #[test]
    fn handles_empty() {
        assert_eq!(process_text(""), "");
    }
}
```

### Step 3: Implement document processor (MD + XLSX stubs; PDF + DOCX stubs with size checks)

Create `crates/minion-presentation/src/input/document.rs`:

```rust
//! Document input processors: PDF, DOCX, Markdown, XLSX.

use std::path::Path;

use anyhow::{bail, Context};

/// 25 MB size limit for all document inputs.
pub const MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

// ── Shared helper ─────────────────────────────────────────────────────────────

/// Check that a file exists and is within the size limit.
fn check_file_size(path: &Path) -> anyhow::Result<u64> {
    let meta = std::fs::metadata(path)
        .with_context(|| format!("cannot stat file: {}", path.display()))?;
    let size = meta.len();
    if size > MAX_FILE_BYTES {
        bail!(
            "file '{}' is {} bytes, exceeds 25 MB size limit",
            path.display(),
            size
        );
    }
    Ok(size)
}

// ── Markdown ──────────────────────────────────────────────────────────────────

/// Process a Markdown input: returns the raw source text unchanged.
///
/// The raw Markdown is more useful to the LLM than rendered HTML because the
/// heading structure, bullet lists, and code blocks convey semantic meaning
/// that maps well onto presentation structure.
pub fn process_markdown(content: &str) -> String {
    // Validate that it's parseable by pulldown-cmark (panic safety), then
    // return the source.  We iterate to consume the iterator.
    let _event_count = pulldown_cmark::Parser::new(content).count();
    content.to_string()
}

// ── XLSX ──────────────────────────────────────────────────────────────────────

/// Maximum number of rows extracted per worksheet.
const MAX_XLSX_ROWS: usize = 200;

/// Extract text from an XLSX/XLS/ODS spreadsheet as tab-separated values.
///
/// Each sheet is prefixed with `### Sheet: <name>` and limited to
/// [`MAX_XLSX_ROWS`] data rows.
pub fn process_xlsx(path: &str) -> anyhow::Result<String> {
    use calamine::{open_workbook_auto, Reader};

    let p = Path::new(path);
    check_file_size(p)?;

    let mut workbook =
        open_workbook_auto(p).with_context(|| format!("failed to open spreadsheet: {path}"))?;

    let sheet_names: Vec<String> = workbook.sheet_names().to_vec();
    let mut out = String::new();

    for sheet_name in sheet_names {
        let range = workbook
            .worksheet_range(&sheet_name)
            .with_context(|| format!("failed to read sheet '{sheet_name}'"))?;

        out.push_str(&format!("### Sheet: {sheet_name}\n\n"));

        for row in range.rows().take(MAX_XLSX_ROWS) {
            let line = row
                .iter()
                .map(|cell| cell.to_string())
                .collect::<Vec<_>>()
                .join("\t");
            out.push_str(&line);
            out.push('\n');
        }
        out.push('\n');
    }

    if out.trim().is_empty() {
        out.push_str("(spreadsheet appears to be empty)");
    }

    Ok(out)
}

// ── PDF ───────────────────────────────────────────────────────────────────────

/// Extract plain text from a PDF file.
///
/// Uses `lopdf` to load the file and then iterates over all pages, extracting
/// encoded text content objects.
pub fn process_pdf(path: &str) -> anyhow::Result<String> {
    let p = Path::new(path);
    check_file_size(p)?;

    let doc = lopdf::Document::load(p)
        .with_context(|| format!("failed to load PDF: {path}"))?;

    let page_ids: Vec<lopdf::ObjectId> = doc.get_pages().values().copied().collect();
    let mut pages_text = Vec::with_capacity(page_ids.len());

    for (page_num, &page_id) in page_ids.iter().enumerate() {
        match extract_page_text(&doc, page_id) {
            Ok(text) => {
                if !text.trim().is_empty() {
                    pages_text.push(format!("--- Page {} ---\n{}", page_num + 1, text));
                }
            }
            Err(e) => {
                tracing::warn!("PDF page {} extraction failed: {e}", page_num + 1);
            }
        }
    }

    if pages_text.is_empty() {
        bail!("PDF '{}' yielded no extractable text", path);
    }

    Ok(pages_text.join("\n\n"))
}

/// Extract text from a single PDF page object.
fn extract_page_text(doc: &lopdf::Document, page_id: lopdf::ObjectId) -> anyhow::Result<String> {
    let content_data = doc
        .get_page_content(page_id)
        .context("failed to get page content")?;
    let content = lopdf::content::Content::decode(&content_data)
        .context("failed to decode page content")?;

    let mut text = String::new();
    for op in &content.operations {
        match op.operator.as_str() {
            "Tj" | "TJ" => {
                for operand in &op.operands {
                    match operand {
                        lopdf::Object::String(bytes, _) => {
                            if let Ok(s) = std::str::from_utf8(bytes) {
                                text.push_str(s);
                            }
                        }
                        lopdf::Object::Array(arr) => {
                            for item in arr {
                                if let lopdf::Object::String(bytes, _) = item {
                                    if let Ok(s) = std::str::from_utf8(bytes) {
                                        text.push_str(s);
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                text.push(' ');
            }
            "Td" | "TD" | "T*" => text.push('\n'),
            _ => {}
        }
    }

    Ok(text)
}

// ── DOCX ──────────────────────────────────────────────────────────────────────

/// Extract plain text from a DOCX file.
///
/// A `.docx` is a ZIP archive. We open `word/document.xml` and strip all XML
/// tags, then normalise whitespace.  This avoids pulling in a heavy DOCX
/// parsing crate.
pub fn process_docx(path: &str) -> anyhow::Result<String> {
    use std::io::Read;

    let p = Path::new(path);
    check_file_size(p)?;

    let file = std::fs::File::open(p)
        .with_context(|| format!("failed to open DOCX: {path}"))?;
    let mut archive = zip::ZipArchive::new(file)
        .with_context(|| format!("DOCX '{path}' is not a valid ZIP archive"))?;

    let mut xml_bytes = Vec::new();
    {
        let mut entry = archive
            .by_name("word/document.xml")
            .context("'word/document.xml' not found — is this a valid DOCX?")?;
        entry
            .read_to_end(&mut xml_bytes)
            .context("failed to read word/document.xml")?;
    }

    let xml_str = String::from_utf8_lossy(&xml_bytes);
    let text = strip_xml_tags(&xml_str);

    if text.trim().is_empty() {
        bail!("DOCX '{}' yielded no extractable text", path);
    }

    Ok(text)
}

/// Remove all XML tags from a string, collapse whitespace, and return the
/// plain text content.
fn strip_xml_tags(xml: &str) -> String {
    let mut result = String::with_capacity(xml.len() / 2);
    let mut inside_tag = false;

    for ch in xml.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                result.push(' ');
            }
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }

    // Collapse runs of whitespace into single spaces, then trim
    let collapsed: String = result
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    collapsed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_returns_source() {
        let md = "# Heading\n\nParagraph.";
        assert_eq!(process_markdown(md), md);
    }

    #[test]
    fn strip_xml_simple() {
        let xml = "<root><w:t>Hello</w:t><w:t>World</w:t></root>";
        let result = strip_xml_tags(xml);
        assert!(result.contains("Hello"), "result: {result}");
        assert!(result.contains("World"), "result: {result}");
        assert!(!result.contains('<'), "result: {result}");
    }

    #[test]
    fn file_size_check_rejects_26mb_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
        let result = check_file_size(tmp.path());
        assert!(result.is_err());
    }
}
```

### Step 4: Create `input/mod.rs` skeleton (to be completed in Task 5)

Create `crates/minion-presentation/src/input/mod.rs`:

```rust
//! Input source processing pipeline.
//!
//! [`InputSource`] is the public contract between the Tauri IPC layer and the
//! AI pipeline. Call [`process_all`] to convert a heterogeneous list of sources
//! into a single text corpus.

pub mod document;
pub mod git;
pub mod image;
pub mod text;
pub mod url;

use anyhow::Context;
use minion_llm::LlmProvider;

/// A single input to the presentation generation pipeline.
///
/// The `kind` tag matches the TypeScript `InputSource.kind` values in
/// `ui/src/lib/presentation-api.ts` (`"text" | "file_path" | "url" | "git_url"`).
///
/// Note on TypeScript alignment: the TS type uses a single `content` field for
/// all variants. The Rust serialisation uses the same flat structure so that
/// `serde_json` round-trips transparently through the Tauri IPC layer.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputSource {
    /// Raw text pasted by the user.
    Text { content: String },
    /// Absolute path to a local file (PDF, DOCX, MD, XLSX, PNG, JPG, WEBP).
    FilePath { content: String },
    /// Public HTTP/HTTPS URL to fetch and extract text from.
    Url { content: String },
    /// Public git repository URL to clone and summarise.
    GitUrl { content: String },
}

/// Process all input sources in parallel and concatenate the results.
///
/// Each source is processed concurrently using [`tokio::task::spawn`].  Results
/// are joined in input order, separated by `\n\n---\n\n`.  If any source fails,
/// its error is logged and a placeholder string is substituted so the overall
/// pipeline continues.
pub async fn process_all(
    sources: Vec<InputSource>,
    llm: &dyn LlmProvider,
) -> anyhow::Result<String> {
    use futures::future::join_all;

    if sources.is_empty() {
        return Ok(String::new());
    }

    // We cannot pass `llm` (a trait object reference) across `spawn` boundaries
    // because it is not `'static`.  Instead we process each source in a
    // sequential-within-concurrent pattern using `tokio::task::spawn_blocking`
    // for CPU-bound work and direct async calls for I/O-bound work.
    //
    // Strategy: collect futures that each produce a String, then await all.
    let futures: Vec<_> = sources
        .into_iter()
        .enumerate()
        .map(|(idx, source)| process_one(idx, source, llm))
        .collect();

    let results = join_all(futures).await;

    let parts: Vec<String> = results
        .into_iter()
        .enumerate()
        .map(|(idx, res)| match res {
            Ok(text) => text,
            Err(e) => {
                tracing::warn!("input source {idx} failed: {e:#}");
                format!("[Input source {idx} could not be processed: {e}]")
            }
        })
        .collect();

    Ok(parts.join("\n\n---\n\n"))
}

/// Process a single [`InputSource`] and return its text representation.
async fn process_one(
    _idx: usize,
    source: InputSource,
    llm: &dyn LlmProvider,
) -> anyhow::Result<String> {
    match source {
        InputSource::Text { content } => Ok(text::process_text(&content)),
        InputSource::FilePath { content } => {
            let path = content;
            dispatch_file(&path, llm).await
        }
        InputSource::Url { content } => {
            url::process_url(&content)
                .await
                .with_context(|| format!("URL input failed: {content}"))
        }
        InputSource::GitUrl { content } => {
            crate::security::git_sandbox::summarize_git_repo(&content)
                .await
                .with_context(|| format!("git URL input failed: {content}"))
        }
    }
}

/// Dispatch a file path to the correct processor based on extension.
async fn dispatch_file(path: &str, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    let lower = path.to_lowercase();
    if lower.ends_with(".pdf") {
        tokio::task::spawn_blocking({
            let p = path.to_owned();
            move || document::process_pdf(&p)
        })
        .await
        .context("spawn_blocking panicked")?
    } else if lower.ends_with(".docx") {
        tokio::task::spawn_blocking({
            let p = path.to_owned();
            move || document::process_docx(&p)
        })
        .await
        .context("spawn_blocking panicked")?
    } else if lower.ends_with(".md") || lower.ends_with(".markdown") {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read markdown file: {path}"))?;
        Ok(document::process_markdown(&content))
    } else if lower.ends_with(".xlsx")
        || lower.ends_with(".xls")
        || lower.ends_with(".ods")
        || lower.ends_with(".csv")
    {
        tokio::task::spawn_blocking({
            let p = path.to_owned();
            move || document::process_xlsx(&p)
        })
        .await
        .context("spawn_blocking panicked")?
    } else if lower.ends_with(".png")
        || lower.ends_with(".jpg")
        || lower.ends_with(".jpeg")
        || lower.ends_with(".webp")
        || lower.ends_with(".gif")
    {
        image::process_image(path, llm).await
    } else if lower.ends_with(".txt") {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read text file: {path}"))?;
        Ok(text::process_text(&content))
    } else {
        // Unknown extension — attempt to read as text
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("unsupported file type and cannot read as text: {path}"))?;
        Ok(text::process_text(&content))
    }
}
```

### Step 5: Create stub files for Tasks 4 and 5 (compilation only)

Create `crates/minion-presentation/src/input/image.rs` (stub — implemented in Task 5):

```rust
//! Image input processor — stub (implemented in Task 5).

use anyhow::bail;
use minion_llm::LlmProvider;

/// Stub implementation — returns an error until Task 5 completes.
pub async fn process_image(_path: &str, _llm: &dyn LlmProvider) -> anyhow::Result<String> {
    bail!("image processor not yet implemented — see Task 5");
}
```

Create `crates/minion-presentation/src/input/url.rs` (stub — implemented in Task 5):

```rust
//! URL input processor — stub (implemented in Task 5).

use anyhow::bail;

/// Stub implementation — returns an error until Task 5 completes.
pub async fn process_url(_url: &str) -> anyhow::Result<String> {
    bail!("URL processor not yet implemented — see Task 5");
}
```

Create `crates/minion-presentation/src/input/git.rs`:

```rust
//! Git URL input processor — thin wrapper over the git sandbox.
//!
//! This module re-exports the git sandbox function so that `dispatch_file`
//! and external callers can use a consistent `input::git` path.

pub use crate::security::git_sandbox::summarize_git_repo;
```

### Step 6: Wire `input` module into `lib.rs`

Append to `crates/minion-presentation/src/lib.rs`:

```rust
pub mod input;
```

So the full file is:

```rust
pub mod db;
pub mod input;
pub mod migrations;
pub mod schema;
pub mod security;

pub use schema::types::*;
```

### Step 7: Verify

```bash
cargo build -p minion-presentation 2>&1 | head -40
cargo test -p minion-presentation --test input_tests 2>&1
cargo test -p minion-presentation 2>&1 | tail -20
```

All size-guard tests and passthrough tests must pass. Stub functions will not be called by tests yet.

### Step 8: Commit

```bash
git add crates/minion-presentation/src/input/ \
        crates/minion-presentation/src/lib.rs \
        crates/minion-presentation/tests/input_tests.rs
git commit -m "feat(presentation): add text/MD/XLSX processors and InputSource enum"
```

---

## Task 4: PDF + DOCX Input Processors

**Files:**
- Modify: `crates/minion-presentation/src/input/document.rs` — `process_pdf` and `process_docx` are already implemented in Task 3.

The implementations of `process_pdf` and `process_docx` were written in Task 3's `document.rs` in their final form (not as stubs). This task focuses on writing thorough tests and verifying the implementations compile and pass.

### Step 1: Add PDF and DOCX tests to `input_tests.rs`

Append to `crates/minion-presentation/tests/input_tests.rs`:

```rust
// ── DOCX text extraction ──────────────────────────────────────────────────────

#[test]
fn docx_extracts_text_from_valid_file() {
    use std::io::Write;

    // Build a minimal valid DOCX (ZIP with word/document.xml).
    let xml_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
  <w:body>
    <w:p><w:r><w:t>Hello from DOCX</w:t></w:r></w:p>
    <w:p><w:r><w:t>Second paragraph.</w:t></w:r></w:p>
  </w:body>
</w:document>"#;

    let tmp = tempfile::NamedTempFile::with_suffix(".docx").unwrap();
    {
        let file = std::fs::File::create(tmp.path()).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        zip.start_file("word/document.xml", options).unwrap();
        zip.write_all(xml_content.as_bytes()).unwrap();
        zip.finish().unwrap();
    }

    let result =
        minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_ok(), "expected Ok, got: {:?}", result);
    let text = result.unwrap();
    assert!(text.contains("Hello from DOCX"), "text: {text}");
    assert!(text.contains("Second paragraph"), "text: {text}");
}

#[test]
fn docx_rejects_non_zip_file() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::with_suffix(".docx").unwrap();
    tmp.write_all(b"this is not a zip file").unwrap();
    tmp.flush().unwrap();

    let result =
        minion_presentation::input::document::process_docx(tmp.path().to_str().unwrap());
    assert!(result.is_err(), "expected error for non-ZIP DOCX");
}

// ── PDF text extraction ───────────────────────────────────────────────────────

#[test]
fn pdf_rejects_non_pdf_file() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::with_suffix(".pdf").unwrap();
    tmp.write_all(b"not a PDF file at all").unwrap();
    tmp.flush().unwrap();

    let result =
        minion_presentation::input::document::process_pdf(tmp.path().to_str().unwrap());
    assert!(result.is_err(), "expected error for non-PDF content");
}

/// Integration test — only run when test PDFs are available.
/// Set PRESENTATION_INTEGRATION_TESTS=1 and place a PDF at /tmp/test.pdf.
#[test]
fn pdf_extracts_text_from_real_file() {
    if std::env::var("PRESENTATION_INTEGRATION_TESTS").is_err() {
        return;
    }
    let path = "/tmp/test.pdf";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let result = minion_presentation::input::document::process_pdf(path);
    assert!(result.is_ok(), "PDF extraction failed: {:?}", result);
    let text = result.unwrap();
    assert!(!text.trim().is_empty(), "PDF yielded empty text");
}
```

### Step 2: Verify all tests pass

```bash
cargo test -p minion-presentation --test input_tests 2>&1
```

Expected: all size-guard tests pass, `docx_extracts_text_from_valid_file` passes, `docx_rejects_non_zip_file` passes, `pdf_rejects_non_pdf_file` passes.

### Step 3: Check for Clippy warnings

```bash
cargo clippy -p minion-presentation -- -D warnings 2>&1 | head -40
```

Fix any warnings before committing.

### Step 4: Commit

```bash
git add crates/minion-presentation/tests/input_tests.rs \
        crates/minion-presentation/src/input/document.rs
git commit -m "test(presentation): add PDF/DOCX extraction tests"
```

---

## Task 5: Image + URL Input Processors

**Files:**
- Modify: `crates/minion-presentation/src/input/image.rs` (replace stub)
- Modify: `crates/minion-presentation/src/input/url.rs` (replace stub)

Both processors depend on `minion_llm::LlmProvider` (image) and `crate::security::ssrf_guard` (URL). `LlmProvider` was defined in Task 1's prerequisite (Foundation sub-plan). `ssrf_guard` was defined in Task 1.

### Step 1: Write failing tests

Append to `crates/minion-presentation/tests/input_tests.rs`:

```rust
// ── Image processor ───────────────────────────────────────────────────────────

#[test]
fn image_rejects_oversized_file() {
    use std::io::Write;
    let mut tmp = tempfile::NamedTempFile::with_suffix(".png").unwrap();
    tmp.write_all(&vec![0u8; 26 * 1024 * 1024]).unwrap();
    tmp.flush().unwrap();

    // We need a mock LLM — use a struct that panics if called (size check
    // should fire first).
    struct PanicLlm;
    #[async_trait::async_trait]
    impl minion_llm::LlmProvider for PanicLlm {
        fn name(&self) -> &str { "panic-llm" }
        async fn chat(&self, _: minion_llm::ChatRequest) -> minion_llm::LlmResult<minion_llm::ChatResponse> {
            panic!("LLM should not be called for oversized image")
        }
        async fn health_check(&self) -> minion_llm::LlmResult<bool> { Ok(true) }
        async fn list_models(&self) -> minion_llm::LlmResult<Vec<minion_llm::ModelInfo>> { Ok(vec![]) }
    }

    let rt = tokio::runtime::Runtime::new().unwrap();
    let result = rt.block_on(
        minion_presentation::input::image::process_image(
            tmp.path().to_str().unwrap(),
            &PanicLlm,
        )
    );
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("25 MB") || msg.contains("size"), "error: {msg}");
}

// ── URL processor ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn url_rejects_private_ip() {
    let result = minion_presentation::input::url::process_url("http://192.168.0.1/secret").await;
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("SSRF") || msg.contains("private") || msg.contains("blocked"),
        "error: {msg}"
    );
}

#[tokio::test]
async fn url_rejects_file_scheme() {
    let result = minion_presentation::input::url::process_url("file:///etc/passwd").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn url_rejects_localhost() {
    let result = minion_presentation::input::url::process_url("http://localhost/").await;
    assert!(result.is_err());
}
```

### Step 2: Implement image processor

Replace the stub in `crates/minion-presentation/src/input/image.rs`:

```rust
//! Image input processor.
//!
//! Reads an image file from disk, encodes it as a base64 data-URL, and
//! sends it to a vision-capable LLM with a fixed prompt.  The response is
//! returned as a plain text description.

use std::path::Path;

use anyhow::{bail, Context};
use base64ct::{Base64, Encoding};
use minion_llm::{ChatRequest, ChatRole, ImageUrl, LlmProvider, VisionContent, VisionMessage};

/// Maximum image file size: 25 MB.
const MAX_IMAGE_BYTES: u64 = 25 * 1024 * 1024;

/// Prompt sent to the vision LLM for all images.
const VISION_PROMPT: &str =
    "Describe this image in detail for use in a presentation. \
     Focus on: key visual elements, data shown (if a chart/graph), \
     text visible in the image, and the overall message it conveys.";

/// Process an image file: encode to base64 and obtain a textual description
/// from the vision LLM.
///
/// Supported formats: PNG, JPEG, WEBP, GIF (anything the LLM's vision API
/// accepts via a data-URL).
pub async fn process_image(path: &str, llm: &dyn LlmProvider) -> anyhow::Result<String> {
    let p = Path::new(path);

    // ── Size check ────────────────────────────────────────────────────────────
    let meta = std::fs::metadata(p)
        .with_context(|| format!("cannot stat image file: {path}"))?;
    if meta.len() > MAX_IMAGE_BYTES {
        bail!(
            "image file '{}' is {} bytes, exceeds 25 MB size limit",
            path,
            meta.len()
        );
    }

    // ── Read and encode ───────────────────────────────────────────────────────
    let bytes = std::fs::read(p)
        .with_context(|| format!("failed to read image file: {path}"))?;

    let mime = infer_mime(path);
    let b64 = Base64::encode_string(&bytes);
    let data_url = format!("data:{mime};base64,{b64}");

    // ── Build vision request ──────────────────────────────────────────────────
    // We use the ChatRequest with a text-only message because not all providers
    // surface the vision API through a separate path.  Providers that support
    // vision (OpenAI, Anthropic, Gemini) accept inline base64 images in the
    // content array when the message contains the data-URL in the text field
    // prefixed with the conventional marker.
    //
    // For maximum compatibility we embed the image reference in the message
    // text as a data-URL and let the provider handle it.
    let user_content = format!("{VISION_PROMPT}\n\n[image: {data_url}]");

    let req = ChatRequest {
        messages: vec![minion_llm::ChatMessage {
            role: ChatRole::User,
            content: user_content,
        }],
        model: None,
        temperature: Some(0.2),
        max_tokens: Some(1024),
        json_mode: false,
        system: Some("You are a presentation assistant. Describe images accurately and concisely.".into()),
    };

    let response = llm.chat(req).await.context("vision LLM call failed")?;

    Ok(format!(
        "[Image: {}]\n\n{}",
        p.file_name().unwrap_or_default().to_string_lossy(),
        response.content
    ))
}

/// Infer a MIME type from a file extension.
fn infer_mime(path: &str) -> &'static str {
    let lower = path.to_lowercase();
    if lower.ends_with(".png") {
        "image/png"
    } else if lower.ends_with(".jpg") || lower.ends_with(".jpeg") {
        "image/jpeg"
    } else if lower.ends_with(".webp") {
        "image/webp"
    } else if lower.ends_with(".gif") {
        "image/gif"
    } else {
        "image/png" // fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infer_mime_png() {
        assert_eq!(infer_mime("photo.png"), "image/png");
    }

    #[test]
    fn infer_mime_jpeg() {
        assert_eq!(infer_mime("photo.jpg"), "image/jpeg");
        assert_eq!(infer_mime("photo.JPEG"), "image/jpeg");
    }

    #[test]
    fn infer_mime_webp() {
        assert_eq!(infer_mime("photo.webp"), "image/webp");
    }

    #[test]
    fn infer_mime_unknown_falls_back_to_png() {
        assert_eq!(infer_mime("photo.bmp"), "image/png");
    }
}
```

### Step 3: Implement URL processor

Replace the stub in `crates/minion-presentation/src/input/url.rs`:

```rust
//! URL input processor.
//!
//! Fetches a public URL (guarded by the SSRF validator), then extracts the
//! readable plain-text content from the HTML response.

use anyhow::{bail, Context};

/// Maximum response body size: 10 MB.
const MAX_RESPONSE_BYTES: usize = 10 * 1024 * 1024;

/// Process a URL: validate → fetch → extract text.
///
/// Returns a plain-text representation of the page content, stripped of HTML
/// tags and boilerplate.
pub async fn process_url(raw_url: &str) -> anyhow::Result<String> {
    // ── 1. SSRF guard (blocking DNS) ──────────────────────────────────────────
    let raw_url = raw_url.to_owned();
    let validated = tokio::task::spawn_blocking({
        let u = raw_url.clone();
        move || {
            crate::security::ssrf_guard::validate_url(&u)
                .map_err(|e| anyhow::anyhow!("SSRF guard: {e}"))
        }
    })
    .await
    .context("spawn_blocking panicked")??;

    let url_str = validated.to_string();

    // ── 2. HTTP fetch ─────────────────────────────────────────────────────────
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(
            crate::security::ssrf_guard::MAX_REDIRECTS,
        ))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("minion-presentation/1.0 (content ingestion)")
        .build()
        .context("failed to build HTTP client")?;

    let response = client
        .get(&url_str)
        .send()
        .await
        .with_context(|| format!("HTTP GET failed: {url_str}"))?;

    let status = response.status();
    if !status.is_success() {
        bail!("HTTP {status} fetching '{url_str}'");
    }

    // Read up to MAX_RESPONSE_BYTES
    let bytes = read_limited_body(response, MAX_RESPONSE_BYTES).await?;

    // ── 3. Extract text ───────────────────────────────────────────────────────
    let raw_text = String::from_utf8_lossy(&bytes).into_owned();
    let text = extract_readable_text(&raw_text, &url_str);

    if text.trim().is_empty() {
        bail!("URL '{}' returned no extractable text", url_str);
    }

    Ok(format!("[Source: {url_str}]\n\n{text}"))
}

/// Read a response body up to `limit` bytes, returning an error if exceeded.
async fn read_limited_body(
    response: reqwest::Response,
    limit: usize,
) -> anyhow::Result<Vec<u8>> {
    use futures::StreamExt;

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("error reading response body")?;
        body.extend_from_slice(&chunk);
        if body.len() > limit {
            bail!(
                "response body exceeds {} MB limit",
                limit / (1024 * 1024)
            );
        }
    }

    Ok(body)
}

/// Extract readable text from an HTML or plain-text response.
///
/// For HTML: strips tags, decodes common HTML entities, and collapses
/// whitespace.  For non-HTML responses the text is returned as-is (truncated
/// to a reasonable length).
fn extract_readable_text(raw: &str, url: &str) -> String {
    let lower_url = url.to_lowercase();
    let is_html = raw.trim_start().starts_with("<!") || raw.contains("<html") || raw.contains("<body");

    if is_html || lower_url.ends_with(".html") || lower_url.ends_with(".htm") {
        strip_html_to_text(raw)
    } else {
        // Plain text / JSON / Markdown — return verbatim (first 50_000 chars)
        raw.chars().take(50_000).collect()
    }
}

/// Strip HTML tags and decode basic entities, returning readable text.
fn strip_html_to_text(html: &str) -> String {
    // Remove <script>, <style>, <head> blocks entirely
    let without_scripts = remove_block_tags(html, &["script", "style", "head", "nav", "footer"]);

    // Strip all remaining tags
    let mut result = String::with_capacity(without_scripts.len());
    let mut inside_tag = false;

    for ch in without_scripts.chars() {
        match ch {
            '<' => inside_tag = true,
            '>' => {
                inside_tag = false;
                result.push(' ');
            }
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }

    // Decode common HTML entities
    let decoded = result
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
        .replace("&#39;", "'");

    // Collapse whitespace and limit length
    let collapsed: String = decoded
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");

    // Limit to 50_000 characters to avoid overwhelming the LLM context
    collapsed.chars().take(50_000).collect()
}

/// Remove entire block elements (including their content) from HTML.
fn remove_block_tags(html: &str, tags: &[&str]) -> String {
    let mut result = html.to_string();
    for tag in tags {
        let open = format!("<{tag}");
        let close = format!("</{tag}>");
        loop {
            let start = result.to_lowercase().find(&open);
            let end = result.to_lowercase().find(&close);
            match (start, end) {
                (Some(s), Some(e)) if s < e => {
                    result.drain(s..(e + close.len()));
                }
                _ => break,
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_extracts_text() {
        let html = "<html><body><h1>Title</h1><p>Hello world</p></body></html>";
        let text = strip_html_to_text(html);
        assert!(text.contains("Title"), "text: {text}");
        assert!(text.contains("Hello world"), "text: {text}");
    }

    #[test]
    fn strip_html_removes_scripts() {
        let html = "<html><head><script>var x=1;</script></head><body><p>Content</p></body></html>";
        let text = strip_html_to_text(html);
        assert!(!text.contains("var x"), "scripts should be removed: {text}");
        assert!(text.contains("Content"), "text: {text}");
    }

    #[test]
    fn strip_html_decodes_entities() {
        let html = "<p>a &amp; b &lt;c&gt;</p>";
        let text = strip_html_to_text(html);
        assert!(text.contains("a & b"), "text: {text}");
    }
}
```

### Step 4: Verify

```bash
cargo build -p minion-presentation 2>&1 | head -40
cargo test -p minion-presentation --test input_tests 2>&1
cargo clippy -p minion-presentation -- -D warnings 2>&1 | head -30
```

All previously-passing tests must still pass. New URL/image tests must pass (SSRF rejection tests are fast and require no network).

### Step 5: Commit

```bash
git add crates/minion-presentation/src/input/image.rs \
        crates/minion-presentation/src/input/url.rs \
        crates/minion-presentation/tests/input_tests.rs
git commit -m "feat(presentation): add image vision + SSRF-guarded URL processors"
```

---

## Task 6: Wire lib.rs + Integration Smoke Test

**Files:**
- Verify: `crates/minion-presentation/src/lib.rs` has both `pub mod security;` and `pub mod input;`
- Append to: `crates/minion-presentation/tests/input_tests.rs`

At this point Tasks 1–5 are complete. This task runs a full end-to-end `process_all` call with a `Text` source and a `FilePath` (markdown) source and verifies the output contains both.

### Step 1: Verify lib.rs is correct

`crates/minion-presentation/src/lib.rs` must read:

```rust
pub mod db;
pub mod input;
pub mod migrations;
pub mod schema;
pub mod security;

pub use schema::types::*;
```

If any `pub mod` line is missing, add it now.

### Step 2: Write the integration smoke test

Append to `crates/minion-presentation/tests/input_tests.rs`:

```rust
// ── End-to-end smoke test ─────────────────────────────────────────────────────

use minion_presentation::input::{process_all, InputSource};

/// A no-op LLM provider for testing.  Only used to satisfy the `process_all`
/// signature; in this smoke test no image sources are present so it won't
/// be called.
struct NoopLlm;

#[async_trait::async_trait]
impl minion_llm::LlmProvider for NoopLlm {
    fn name(&self) -> &str {
        "noop-llm"
    }

    async fn chat(
        &self,
        _req: minion_llm::ChatRequest,
    ) -> minion_llm::LlmResult<minion_llm::ChatResponse> {
        Ok(minion_llm::ChatResponse {
            content: "(noop)".into(),
            model: "noop".into(),
            usage: None,
        })
    }

    async fn health_check(&self) -> minion_llm::LlmResult<bool> {
        Ok(true)
    }

    async fn list_models(&self) -> minion_llm::LlmResult<Vec<minion_llm::ModelInfo>> {
        Ok(vec![])
    }
}

#[tokio::test]
async fn process_all_text_and_markdown_sources() {
    use std::io::Write;

    // Write a temp markdown file
    let mut md_file = tempfile::NamedTempFile::with_suffix(".md").unwrap();
    writeln!(md_file, "# Test Slide\n\nThis is the slide content.").unwrap();
    md_file.flush().unwrap();
    let md_path = md_file.path().to_str().unwrap().to_string();

    let sources = vec![
        InputSource::Text {
            content: "Hello from text input".into(),
        },
        InputSource::FilePath {
            content: md_path,
        },
    ];

    let llm = NoopLlm;
    let result = process_all(sources, &llm).await;

    assert!(result.is_ok(), "process_all failed: {:?}", result);
    let corpus = result.unwrap();

    assert!(
        corpus.contains("Hello from text input"),
        "text source missing from corpus: {corpus}"
    );
    assert!(
        corpus.contains("Test Slide"),
        "markdown content missing from corpus: {corpus}"
    );
    // Verify separator is present between sources
    assert!(corpus.contains("---"), "separator missing: {corpus}");
}

#[tokio::test]
async fn process_all_empty_sources_returns_empty_string() {
    let llm = NoopLlm;
    let result = process_all(vec![], &llm).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "");
}

#[tokio::test]
async fn process_all_failing_source_does_not_abort_others() {
    // A URL that points to a private IP will fail SSRF, but the text source
    // should still appear in the output.
    let sources = vec![
        InputSource::Text {
            content: "good text".into(),
        },
        InputSource::Url {
            content: "http://192.168.1.1/bad".into(),
        },
    ];

    let llm = NoopLlm;
    let result = process_all(sources, &llm).await;
    assert!(result.is_ok(), "process_all should not abort on partial failure");
    let corpus = result.unwrap();
    assert!(
        corpus.contains("good text"),
        "successful source missing: {corpus}"
    );
    // The failed source should leave a placeholder
    assert!(
        corpus.contains("could not be processed") || corpus.contains("Input source"),
        "error placeholder missing: {corpus}"
    );
}
```

### Step 3: Add `async-trait` to dev-dependencies

`async-trait` is a workspace dep. Add it to `crates/minion-presentation/Cargo.toml` under `[dev-dependencies]`:

```toml
async-trait = { workspace = true }
```

### Step 4: Run all tests

```bash
cargo test -p minion-presentation 2>&1
```

Expected output: all tests pass (at minimum the 20+ unit tests and 3 smoke tests). The integration tests guarded by `PRESENTATION_INTEGRATION_TESTS` are skipped in normal CI.

### Step 5: Final lint pass

```bash
cargo clippy -p minion-presentation -- -D warnings 2>&1
cargo fmt --all -- --check 2>&1
```

Fix any issues before committing.

### Step 6: Final commit

```bash
git add crates/minion-presentation/src/lib.rs \
        crates/minion-presentation/Cargo.toml \
        crates/minion-presentation/tests/input_tests.rs
git commit -m "feat(presentation): wire security + input modules, add e2e smoke tests"
```

---

## Summary Table

| Task | Files Created/Modified | Key Types/Functions Introduced |
|---|---|---|
| 1: SSRF Guard | `security/ssrf_guard.rs`, `security/mod.rs` | `validate_url()`, `MAX_REDIRECTS`, `is_private_or_loopback()` |
| 2: Git Sandbox | `security/git_sandbox.rs` | `summarize_git_repo()`, `GIT_DIR_MAX_BYTES`, `REPO_MAX_BYTES` |
| 3: Text/MD/XLSX + Stubs | `input/text.rs`, `input/document.rs`, `input/mod.rs` (+ stubs for image/url/git) | `process_text()`, `process_markdown()`, `process_xlsx()`, `process_pdf()`, `process_docx()`, `InputSource`, `process_all()` |
| 4: PDF/DOCX Tests | `tests/input_tests.rs` additions | Integration tests for Task 3 implementations |
| 5: Image + URL | `input/image.rs`, `input/url.rs` | `process_image()`, `process_url()`, `strip_html_to_text()` |
| 6: Wire + Smoke | `lib.rs` verification, `tests/input_tests.rs` additions | `NoopLlm`, end-to-end `process_all` tests |

## Dependency Flow

```
Task 1: ssrf_guard (standalone)
    ↓
Task 2: git_sandbox (needs ssrf_guard)
    ↓
Task 3: input/* (needs ssrf_guard + document libs; stubs for image/url)
    ↓
Task 4: Tests for Task 3 implementations
    ↓
Task 5: image.rs + url.rs (needs ssrf_guard, LlmProvider, reqwest)
    ↓
Task 6: Smoke tests (needs all of the above)
```

All types used in later tasks are defined in earlier tasks. No "TBD" placeholders remain in any implementation.
