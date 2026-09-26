use std::path::Path;

use peach_app::FileRemoverInfra;

/// Low-level file remove service
///
/// Provides primitive file deletion operations without snapshot coordination.
/// Snapshot management should be handled at the service layer.
#[derive(Default)]
pub struct PeachFileRemoveService;

impl PeachFileRemoveService {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl FileRemoverInfra for PeachFileRemoveService {
    async fn remove(&self, path: &Path) -> anyhow::Result<()> {
        Ok(peach_fs::PeachFS::remove_file(path).await?)
    }
}
