use std::path::Path;

use anyhow::Result;
use peach_app::FileInfoInfra;

pub struct PeachFileMetaService;
#[async_trait::async_trait]
impl FileInfoInfra for PeachFileMetaService {
    async fn is_file(&self, path: &Path) -> Result<bool> {
        Ok(peach_fs::PeachFS::is_file(path))
    }

    async fn is_binary(&self, path: &Path) -> Result<bool> {
        peach_fs::PeachFS::is_binary_file(path).await
    }

    async fn exists(&self, path: &Path) -> Result<bool> {
        Ok(peach_fs::PeachFS::exists(path))
    }

    async fn file_size(&self, path: &Path) -> Result<u64> {
        peach_fs::PeachFS::file_size(path).await
    }
}
