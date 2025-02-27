use crate::repository::{GitFileStatus, RepoPath};
use anyhow::{anyhow, Result};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    sync::Arc,
};

#[derive(Clone)]
pub struct GitStatus {
    pub entries: Arc<[(RepoPath, GitFileStatus)]>,
}

impl GitStatus {
    pub(crate) fn new(
        git_binary: &Path,
        working_directory: &Path,
        path_prefixes: &[PathBuf],
    ) -> Result<Self> {
        let child = util::command::new_std_command(git_binary)
            .current_dir(working_directory)
            .args([
                "--no-optional-locks",
                "status",
                "--porcelain=v1",
                "--untracked-files=all",
                "-z",
            ])
            .args(path_prefixes.iter().map(|path_prefix| {
                if *path_prefix == Path::new("") {
                    Path::new(".")
                } else {
                    path_prefix
                }
            }))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| anyhow!("Failed to start git status process: {}", e))?;

        let output = child
            .wait_with_output()
            .map_err(|e| anyhow!("Failed to read git blame output: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git status process failed: {}", stderr));
        }
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut entries = stdout
            .split('\0')
            .filter_map(|entry| {
                if entry.is_char_boundary(3) {
                    let (status, path) = entry.split_at(3);
                    let status = status.trim();
                    Some((
                        RepoPath(PathBuf::from(path)),
                        match status {
                            "A" | "??" => GitFileStatus::Added,
                            "M" => GitFileStatus::Modified,
                            _ => return None,
                        },
                    ))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        entries.sort_unstable_by(|a, b| a.0.cmp(&b.0));
        Ok(Self {
            entries: entries.into(),
        })
    }

    pub fn get(&self, path: &Path) -> Option<GitFileStatus> {
        self.entries
            .binary_search_by(|(repo_path, _)| repo_path.0.as_path().cmp(path))
            .ok()
            .map(|index| self.entries[index].1)
    }
}

impl Default for GitStatus {
    fn default() -> Self {
        Self {
            entries: Arc::new([]),
        }
    }
}
