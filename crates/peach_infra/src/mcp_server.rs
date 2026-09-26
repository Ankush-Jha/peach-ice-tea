use std::collections::BTreeMap;

use peach_app::McpServerInfra;
use peach_domain::{Environment, McpServerConfig};

use crate::mcp_client::PeachMcpClient;

#[derive(Clone)]
pub struct PeachMcpServer;

#[async_trait::async_trait]
impl McpServerInfra for PeachMcpServer {
    type Client = PeachMcpClient;

    async fn connect(
        &self,
        config: McpServerConfig,
        env_vars: &BTreeMap<String, String>,
        environment: &Environment,
    ) -> anyhow::Result<Self::Client> {
        Ok(PeachMcpClient::new(config, env_vars, environment.clone()))
    }
}
