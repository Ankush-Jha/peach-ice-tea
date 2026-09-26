use std::path::PathBuf;
use std::sync::Arc;

use peach_app::{CommandInfra, CustomInstructionsService, EnvironmentInfra, FileReaderInfra};

/// This service looks for AGENTS.md files in three locations in order of
/// priority:
/// 1. Base path (environment.base_path)
/// 2. Git root directory (if available)
/// 3. Current working directory (environment.cwd)
#[derive(Clone)]
pub struct PeachCustomInstructionsService<F> {
    infra: Arc<F>,
    cache: tokio::sync::OnceCell<Vec<String>>,
}

impl<F: EnvironmentInfra + FileReaderInfra + CommandInfra> PeachCustomInstructionsService<F> {
    pub fn new(infra: Arc<F>) -> Self {
        Self { infra, cache: Default::default() }
    }

    async fn discover_agents_files(&self) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let environment = self.infra.get_environment();

        // Base custom instructions
        let base_agent_md = environment.global_agentsmd_path();
        if !paths.contains(&base_agent_md) {
            paths.push(base_agent_md);
        }

        // Repo custom instructions
        if let Some(git_root_path) = self.get_git_root().await {
            let git_agent_md = git_root_path.join("AGENTS.md");
            if !paths.contains(&git_agent_md) {
                paths.push(git_agent_md);
            }
        }

        // Working dir custom instructions
        let cwd_agent_md = environment.local_agentsmd_path();
        if !paths.contains(&cwd_agent_md) {
            paths.push(cwd_agent_md);
        }

        paths
    }

    /// harness: R-MEM-1 (T6.4, D-067) — the project memory file, at the git
    /// root or else the working directory. Loaded after AGENTS.md.
    async fn discover_memory_file(&self) -> PathBuf {
        let root = self.get_git_root().await.unwrap_or_else(|| self.infra.get_environment().cwd);
        root.join(".peach").join("memory.md")
    }

    async fn get_git_root(&self) -> Option<PathBuf> {
        let output = self
            .infra
            .execute_command(
                "git rev-parse --show-toplevel".to_owned(),
                self.infra.get_environment().cwd,
                true, // silent mode - don't print git output
                None, // no environment variables needed for git command
            )
            .await
            .ok()?;

        if output.success() {
            Some(PathBuf::from(output.stdout.trim()))
        } else {
            None
        }
    }

    async fn init(&self) -> Vec<String> {
        let paths = self.discover_agents_files().await;

        let mut custom_instructions = Vec::new();

        for path in paths {
            if let Ok(content) = self.infra.read_utf8(&path).await {
                custom_instructions.push(content);
            }
        }

        if let Ok(memory) = self.infra.read_utf8(&self.discover_memory_file().await).await
            && !memory.trim().is_empty()
        {
            custom_instructions.push(bounded_memory(&memory));
        }

        custom_instructions
    }
}

/// Most of the memory file loaded into the system prompt (R-MEM-1: "bounded
/// size"). A larger file is clipped with a loud note naming the file.
const MEMORY_MAX_CHARS: usize = 16_000;

fn bounded_memory(memory: &str) -> String {
    let total = memory.chars().count();
    if total <= MEMORY_MAX_CHARS {
        return format!("# Project memory (.peach/memory.md)\n\n{memory}");
    }
    let head: String = memory.chars().take(MEMORY_MAX_CHARS).collect();
    format!(
        "# Project memory (.peach/memory.md)\n\n{head}\n\n{} more characters not shown. Full file: read .peach/memory.md.",
        total - MEMORY_MAX_CHARS
    )
}

#[async_trait::async_trait]
impl<F: EnvironmentInfra + FileReaderInfra + CommandInfra> CustomInstructionsService
    for PeachCustomInstructionsService<F>
{
    async fn get_custom_instructions(&self) -> Vec<String> {
        self.cache.get_or_init(|| self.init()).await.clone()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_memory_is_headed_and_a_long_file_is_clipped_loudly() {
        let short = bounded_memory("Always use tabs.");
        let long = bounded_memory(&"x".repeat(MEMORY_MAX_CHARS + 5));

        assert_eq!(short, "# Project memory (.peach/memory.md)\n\nAlways use tabs.");
        assert!(long.ends_with("5 more characters not shown. Full file: read .peach/memory.md."));
    }
}
