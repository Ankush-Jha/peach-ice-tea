use std::path::Path;

use peach_app::FileDirectoryInfra;

#[derive(Default)]
pub struct PeachCreateDirsService;

#[async_trait::async_trait]
impl FileDirectoryInfra for PeachCreateDirsService {
    async fn create_dirs(&self, path: &Path) -> anyhow::Result<()> {
        Ok(peach_fs::PeachFS::create_dir_all(path).await?)
    }
}
