use std::sync::Arc;

use peach_app::{
    AgentRepository, CommandInfra, DirectoryReaderInfra, EnvironmentInfra, FileDirectoryInfra,
    FileInfoInfra, FileReaderInfra, FileRemoverInfra, FileWriterInfra, HttpInfra, KVStore,
    McpServerInfra, Services, StrategyFactory, UserInfra, WalkerInfra,
};
use peach_domain::{
    ChatRepository, ConversationRepository, FuzzySearchRepository, ProviderRepository,
    SkillRepository, SnapshotRepository, TextPatchRepository, ValidationRepository,
    WorkspaceIndexRepository,
};

use crate::PeachProviderAuthService;
use crate::agent_registry::PeachAgentRegistryService;
use crate::app_config::PeachAppConfigService;
use crate::attachment::PeachChatRequest;
use crate::auth::PeachAuthService;
use crate::command::CommandLoaderService as PeachCommandLoaderService;
use crate::conversation::PeachConversationService;
use crate::discovery::PeachDiscoveryService;
use crate::fd::FdDefault;
use crate::instructions::PeachCustomInstructionsService;
use crate::mcp::{PeachMcpManager, PeachMcpService};
use crate::policy::PeachPolicyService;
use crate::provider_service::PeachProviderService;
use crate::template::ForgeTemplateService;
use crate::tool_services::{
    PeachFetch, PeachFollowup, PeachFsPatch, PeachFsRead, PeachFsRemove, PeachFsSearch,
    PeachFsUndo, PeachFsWrite, PeachImageRead, PeachPlanCreate, PeachShell, PeachSkillFetch,
};

type McpService<F> = PeachMcpService<PeachMcpManager<F>, F, <F as McpServerInfra>::Client>;
type AuthService<F> = PeachAuthService<F>;

/// PeachApp is the main application container that implements the App trait.
/// It provides access to all core services required by the application.
///
/// Type Parameters:
/// - F: The infrastructure implementation that provides core services like
///   environment, file reading, vector indexing, and embedding.
/// - R: The repository implementation that provides data persistence
#[derive(Clone)]
pub struct PeachServices<
    F: HttpInfra
        + EnvironmentInfra
        + McpServerInfra
        + WalkerInfra
        + SnapshotRepository
        + ConversationRepository
        + KVStore
        + ChatRepository
        + ProviderRepository
        + WorkspaceIndexRepository
        + AgentRepository
        + SkillRepository
        + ValidationRepository,
> {
    chat_service: Arc<PeachProviderService<F>>,
    config_service: Arc<PeachAppConfigService<F>>,
    conversation_service: Arc<PeachConversationService<F>>,
    template_service: Arc<ForgeTemplateService<F>>,
    attachment_service: Arc<PeachChatRequest<F>>,
    discovery_service: Arc<PeachDiscoveryService<F>>,
    mcp_manager: Arc<PeachMcpManager<F>>,
    file_create_service: Arc<PeachFsWrite<F>>,
    plan_create_service: Arc<PeachPlanCreate<F>>,
    file_read_service: Arc<PeachFsRead<F>>,
    image_read_service: Arc<PeachImageRead<F>>,
    file_search_service: Arc<PeachFsSearch<F>>,
    file_remove_service: Arc<PeachFsRemove<F>>,
    file_patch_service: Arc<PeachFsPatch<F>>,
    file_undo_service: Arc<PeachFsUndo<F>>,
    shell_service: Arc<PeachShell<F>>,
    fetch_service: Arc<PeachFetch>,
    followup_service: Arc<PeachFollowup<F>>,
    mcp_service: Arc<McpService<F>>,
    custom_instructions_service: Arc<PeachCustomInstructionsService<F>>,
    auth_service: Arc<AuthService<F>>,
    agent_registry_service: Arc<PeachAgentRegistryService<F>>,
    command_loader_service: Arc<PeachCommandLoaderService<F>>,
    policy_service: PeachPolicyService<F>,
    provider_auth_service: PeachProviderAuthService<F>,
    workspace_service: Arc<crate::context_engine::PeachWorkspaceService<F, FdDefault<F>>>,
    skill_service: Arc<PeachSkillFetch<F>>,
    infra: Arc<F>,
}

impl<
    F: McpServerInfra
        + EnvironmentInfra<Config = peach_config::PeachConfig>
        + FileWriterInfra
        + FileInfoInfra
        + FileReaderInfra
        + HttpInfra
        + WalkerInfra
        + DirectoryReaderInfra
        + CommandInfra
        + UserInfra
        + SnapshotRepository
        + ConversationRepository
        + ChatRepository
        + ProviderRepository
        + KVStore
        + WorkspaceIndexRepository
        + AgentRepository
        + SkillRepository
        + ValidationRepository,
> PeachServices<F>
{
    pub fn new(infra: Arc<F>) -> Self {
        let mcp_manager = Arc::new(PeachMcpManager::new(infra.clone()));
        let mcp_service = Arc::new(PeachMcpService::new(mcp_manager.clone(), infra.clone()));
        let template_service = Arc::new(ForgeTemplateService::new(infra.clone()));
        let attachment_service = Arc::new(PeachChatRequest::new(infra.clone()));
        let suggestion_service = Arc::new(PeachDiscoveryService::new(infra.clone()));
        let conversation_service = Arc::new(PeachConversationService::new(infra.clone()));
        let auth_service = Arc::new(PeachAuthService::new(infra.clone()));
        let chat_service = Arc::new(PeachProviderService::new(infra.clone()));
        let config_service = Arc::new(PeachAppConfigService::new(infra.clone()));
        let file_create_service = Arc::new(PeachFsWrite::new(infra.clone()));
        let plan_create_service = Arc::new(PeachPlanCreate::new(infra.clone()));
        let file_read_service = Arc::new(PeachFsRead::new(infra.clone()));
        let image_read_service = Arc::new(PeachImageRead::new(infra.clone()));
        let file_search_service = Arc::new(PeachFsSearch::new(infra.clone()));
        let file_remove_service = Arc::new(PeachFsRemove::new(infra.clone()));
        let file_patch_service = Arc::new(PeachFsPatch::new(infra.clone()));
        let file_undo_service = Arc::new(PeachFsUndo::new(infra.clone()));
        let shell_service = Arc::new(PeachShell::new(infra.clone()));
        let fetch_service = Arc::new(PeachFetch::new());
        let followup_service = Arc::new(PeachFollowup::new(infra.clone()));
        let custom_instructions_service =
            Arc::new(PeachCustomInstructionsService::new(infra.clone()));
        let agent_registry_service = Arc::new(PeachAgentRegistryService::new(infra.clone()));
        let command_loader_service = Arc::new(PeachCommandLoaderService::new(infra.clone()));
        let policy_service = PeachPolicyService::new(infra.clone());
        let provider_auth_service = PeachProviderAuthService::new(infra.clone());
        let discovery = Arc::new(FdDefault::new(infra.clone()));
        let workspace_service = Arc::new(crate::context_engine::PeachWorkspaceService::new(
            infra.clone(),
            discovery,
        ));
        let skill_service = Arc::new(PeachSkillFetch::new(infra.clone()));

        Self {
            conversation_service,
            attachment_service,
            template_service,
            discovery_service: suggestion_service,
            mcp_manager,
            file_create_service,
            plan_create_service,
            file_read_service,
            image_read_service,
            file_search_service,
            file_remove_service,
            file_patch_service,
            file_undo_service,
            shell_service,
            fetch_service,
            followup_service,
            mcp_service,
            custom_instructions_service,
            auth_service,
            config_service,
            agent_registry_service,
            command_loader_service,
            policy_service,
            provider_auth_service,
            workspace_service,
            skill_service,
            chat_service,
            infra,
        }
    }
}

impl<
    F: FileReaderInfra
        + FileWriterInfra
        + CommandInfra
        + UserInfra
        + McpServerInfra
        + FileRemoverInfra
        + FileInfoInfra
        + FileDirectoryInfra
        + EnvironmentInfra<Config = peach_config::PeachConfig>
        + DirectoryReaderInfra
        + HttpInfra
        + WalkerInfra
        + Clone
        + SnapshotRepository
        + ConversationRepository
        + KVStore
        + ChatRepository
        + ProviderRepository
        + AgentRepository
        + SkillRepository
        + StrategyFactory
        + WorkspaceIndexRepository
        + ValidationRepository
        + FuzzySearchRepository
        + TextPatchRepository
        + Clone
        + 'static,
> Services for PeachServices<F>
{
    type AppConfigService = PeachAppConfigService<F>;
    type ConversationService = PeachConversationService<F>;
    type TemplateService = ForgeTemplateService<F>;
    type ProviderAuthService = PeachProviderAuthService<F>;

    fn provider_auth_service(&self) -> &Self::ProviderAuthService {
        &self.provider_auth_service
    }
    type AttachmentService = PeachChatRequest<F>;
    type CustomInstructionsService = PeachCustomInstructionsService<F>;
    type FileDiscoveryService = PeachDiscoveryService<F>;
    type McpConfigManager = PeachMcpManager<F>;
    type FsWriteService = PeachFsWrite<F>;
    type PlanCreateService = PeachPlanCreate<F>;
    type FsPatchService = PeachFsPatch<F>;
    type FsReadService = PeachFsRead<F>;
    type ImageReadService = PeachImageRead<F>;
    type FsRemoveService = PeachFsRemove<F>;
    type FsSearchService = PeachFsSearch<F>;
    type FollowUpService = PeachFollowup<F>;
    type FsUndoService = PeachFsUndo<F>;
    type NetFetchService = PeachFetch;
    type ShellService = PeachShell<F>;
    type McpService = McpService<F>;
    type AuthService = AuthService<F>;
    type AgentRegistry = PeachAgentRegistryService<F>;
    type CommandLoaderService = PeachCommandLoaderService<F>;
    type PolicyService = PeachPolicyService<F>;
    type ProviderService = PeachProviderService<F>;
    type WorkspaceService = crate::context_engine::PeachWorkspaceService<F, FdDefault<F>>;
    type SkillFetchService = PeachSkillFetch<F>;

    fn config_service(&self) -> &Self::AppConfigService {
        &self.config_service
    }

    fn conversation_service(&self) -> &Self::ConversationService {
        &self.conversation_service
    }

    fn template_service(&self) -> &Self::TemplateService {
        &self.template_service
    }

    fn attachment_service(&self) -> &Self::AttachmentService {
        &self.attachment_service
    }

    fn custom_instructions_service(&self) -> &Self::CustomInstructionsService {
        &self.custom_instructions_service
    }

    fn file_discovery_service(&self) -> &Self::FileDiscoveryService {
        self.discovery_service.as_ref()
    }

    fn mcp_config_manager(&self) -> &Self::McpConfigManager {
        self.mcp_manager.as_ref()
    }

    fn fs_create_service(&self) -> &Self::FsWriteService {
        &self.file_create_service
    }

    fn plan_create_service(&self) -> &Self::PlanCreateService {
        &self.plan_create_service
    }

    fn fs_patch_service(&self) -> &Self::FsPatchService {
        &self.file_patch_service
    }

    fn fs_read_service(&self) -> &Self::FsReadService {
        &self.file_read_service
    }

    fn fs_remove_service(&self) -> &Self::FsRemoveService {
        &self.file_remove_service
    }

    fn fs_search_service(&self) -> &Self::FsSearchService {
        &self.file_search_service
    }

    fn follow_up_service(&self) -> &Self::FollowUpService {
        &self.followup_service
    }

    fn fs_undo_service(&self) -> &Self::FsUndoService {
        &self.file_undo_service
    }

    fn net_fetch_service(&self) -> &Self::NetFetchService {
        &self.fetch_service
    }

    fn shell_service(&self) -> &Self::ShellService {
        &self.shell_service
    }

    fn mcp_service(&self) -> &Self::McpService {
        &self.mcp_service
    }

    fn auth_service(&self) -> &Self::AuthService {
        self.auth_service.as_ref()
    }

    fn agent_registry(&self) -> &Self::AgentRegistry {
        &self.agent_registry_service
    }

    fn command_loader_service(&self) -> &Self::CommandLoaderService {
        &self.command_loader_service
    }

    fn policy_service(&self) -> &Self::PolicyService {
        &self.policy_service
    }

    fn workspace_service(&self) -> &Self::WorkspaceService {
        &self.workspace_service
    }

    fn image_read_service(&self) -> &Self::ImageReadService {
        &self.image_read_service
    }
    fn skill_fetch_service(&self) -> &Self::SkillFetchService {
        &self.skill_service
    }

    fn provider_service(&self) -> &Self::ProviderService {
        &self.chat_service
    }
}

impl<
    F: EnvironmentInfra<Config = peach_config::PeachConfig>
        + HttpInfra
        + McpServerInfra
        + WalkerInfra
        + SnapshotRepository
        + ConversationRepository
        + KVStore
        + ChatRepository
        + ProviderRepository
        + WorkspaceIndexRepository
        + AgentRepository
        + SkillRepository
        + ValidationRepository
        + Send
        + Sync,
> peach_app::EnvironmentInfra for PeachServices<F>
{
    type Config = peach_config::PeachConfig;

    fn get_environment(&self) -> peach_domain::Environment {
        self.infra.get_environment()
    }

    fn get_config(&self) -> anyhow::Result<peach_config::PeachConfig> {
        self.infra.get_config()
    }

    fn update_environment(
        &self,
        ops: Vec<peach_domain::ConfigOperation>,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        self.infra.update_environment(ops)
    }

    fn get_env_var(&self, key: &str) -> Option<String> {
        self.infra.get_env_var(key)
    }

    fn get_env_vars(&self) -> std::collections::BTreeMap<String, String> {
        self.infra.get_env_vars()
    }
}
