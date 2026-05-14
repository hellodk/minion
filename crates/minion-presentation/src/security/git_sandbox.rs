use std::path::Path;
use anyhow::{bail, Context};
use crate::security::ssrf_guard;

pub const GIT_DIR_MAX_BYTES: u64 = 50 * 1024 * 1024;
pub const REPO_MAX_BYTES: u64 = 100 * 1024 * 1024;
const MAX_LINES_PER_FILE: usize = 200;

pub async fn summarize_git_repo(remote_url: &str) -> anyhow::Result<String> {
    let remote_url = remote_url.to_owned();
    let validated_url = tokio::task::spawn_blocking({
        let url = remote_url.clone();
        move || ssrf_guard::validate_url(&url).map_err(|e| anyhow::anyhow!("SSRF guard: {e}"))
    })
    .await
    .context("spawn_blocking panicked")??;

    let url_str = validated_url.as_str().to_owned();

    let summary = tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
        let tmp = tempfile::TempDir::new().context("failed to create temp dir")?;
        let tmp_path = tmp.path().to_owned();

        let _repo = git2::Repository::clone(&url_str, &tmp_path)
            .context("git clone failed")?;

        let git_dir = tmp_path.join(".git");
        let git_size = dir_size_bytes(&git_dir)?;
        if git_size > GIT_DIR_MAX_BYTES {
            bail!("git sandbox: .git directory is {git_size} bytes (limit: {GIT_DIR_MAX_BYTES})");
        }
        let total_size = dir_size_bytes(&tmp_path)?;
        if total_size > REPO_MAX_BYTES {
            bail!("git sandbox: repository is {total_size} bytes total (limit: {REPO_MAX_BYTES})");
        }

        let repo_name = url_str
            .trim_end_matches('/')
            .trim_end_matches(".git")
            .rsplit('/')
            .next()
            .unwrap_or("unknown")
            .to_string();

        let readme_text = read_readme(&tmp_path);
        let manifest_text = read_manifests(&tmp_path);
        let structure_text = top_level_structure(&tmp_path);

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
        drop(tmp);
        Ok(out)
    })
    .await
    .context("spawn_blocking panicked")??;

    Ok(summary)
}

fn dir_size_bytes(dir: &Path) -> anyhow::Result<u64> {
    let mut total: u64 = 0;
    for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            total += entry.metadata().map(|m| m.len()).unwrap_or(0);
        }
    }
    Ok(total)
}

fn read_readme(root: &Path) -> String {
    for name in &["README.md", "readme.md", "README.rst", "README.txt", "README"] {
        let p = root.join(name);
        if p.exists() && p.is_file() {
            return read_first_n_lines(&p, MAX_LINES_PER_FILE);
        }
    }
    String::new()
}

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

fn top_level_structure(root: &Path) -> String {
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return "(could not list directory)".to_string();
    };
    for entry in entries.filter_map(|e| e.ok()) {
        let name = entry.file_name().to_string_lossy().to_string();
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

fn read_first_n_lines(path: &Path, n: usize) -> String {
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
    let sample = &content[..content.len().min(1024)];
    let non_utf8_count = sample.iter().filter(|&&b| b == 0 || b > 127).count();
    if non_utf8_count > sample.len() / 20 {
        return String::new();
    }
    let text = String::from_utf8_lossy(&content);
    text.lines().take(n).collect::<Vec<_>>().join("\n")
}
