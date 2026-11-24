use crate::bespoke_event_handling::apply_bespoke_event_handling;
use crate::command_exec::CommandExecManager;
use crate::command_exec::StartCommandExecParams;
use crate::config_manager::ConfigManager;
use crate::error_code::INPUT_TOO_LARGE_ERROR_CODE;
use crate::error_code::invalid_params;
use crate::models::supported_models;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::ConnectionRequestId;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::RequestContext;
use crate::outgoing_message::ThreadScopedOutgoingMessageSender;
use crate::skills_watcher::SkillsWatcher;
use crate::thread_status::ThreadWatchManager;
use crate::thread_status::resolve_thread_status;
use chrono::Duration as ChronoDuration;
use chrono::SecondsFormat;
use maplecode_analytics::AnalyticsEventsClient;
use maplecode_analytics::AnalyticsJsonRpcError;
use maplecode_analytics::InputError;
use maplecode_analytics::TurnSteerRequestError;
use maplecode_app_server_protocol::Account;
use maplecode_app_server_protocol::AccountLoginCompletedNotification;
use maplecode_app_server_protocol::AccountTokenUsageDailyBucket;
use maplecode_app_server_protocol::AccountTokenUsageSummary;
use maplecode_app_server_protocol::AccountUpdatedNotification;
use maplecode_app_server_protocol::AddCreditsNudgeCreditType;
use maplecode_app_server_protocol::AddCreditsNudgeEmailStatus;
use maplecode_app_server_protocol::AdditionalContextEntry;
use maplecode_app_server_protocol::AdditionalContextKind;
use maplecode_app_server_protocol::AppListUpdatedNotification;
use maplecode_app_server_protocol::AppSummary;
use maplecode_app_server_protocol::AppTemplateSummary;
use maplecode_app_server_protocol::AppTemplateUnavailableReason;
use maplecode_app_server_protocol::AppsInstalledParams;
use maplecode_app_server_protocol::AppsInstalledResponse;
use maplecode_app_server_protocol::AppsListParams;
use maplecode_app_server_protocol::AppsListResponse;
use maplecode_app_server_protocol::AppsReadParams;
use maplecode_app_server_protocol::AppsReadResponse;
use maplecode_app_server_protocol::AskForApproval;
use maplecode_app_server_protocol::AuthMode;
use maplecode_app_server_protocol::CancelLoginAccountParams;
use maplecode_app_server_protocol::CancelLoginAccountResponse;
use maplecode_app_server_protocol::CancelLoginAccountStatus;
use maplecode_app_server_protocol::ClientInfo;
use maplecode_app_server_protocol::ClientRequest;
use maplecode_app_server_protocol::ClientResponsePayload;
use maplecode_app_server_protocol::CollaborationModeListParams;
use maplecode_app_server_protocol::CollaborationModeListResponse;
use maplecode_app_server_protocol::CommandExecParams;
use maplecode_app_server_protocol::CommandExecResizeParams;
use maplecode_app_server_protocol::CommandExecTerminateParams;
use maplecode_app_server_protocol::CommandExecWriteParams;
use maplecode_app_server_protocol::ConfigWarningNotification;
use maplecode_app_server_protocol::ConsumeAccountRateLimitResetCreditOutcome;
use maplecode_app_server_protocol::ConsumeAccountRateLimitResetCreditParams;
use maplecode_app_server_protocol::ConsumeAccountRateLimitResetCreditResponse;
use maplecode_app_server_protocol::ConversationGitInfo;
use maplecode_app_server_protocol::ConversationSummary;
use maplecode_app_server_protocol::DeprecationNoticeNotification;
use maplecode_app_server_protocol::DynamicToolFunctionSpec;
use maplecode_app_server_protocol::DynamicToolNamespaceTool;
use maplecode_app_server_protocol::DynamicToolSpec;
use maplecode_app_server_protocol::EnvironmentAddParams;
use maplecode_app_server_protocol::EnvironmentAddResponse;
use maplecode_app_server_protocol::EnvironmentInfoParams;
use maplecode_app_server_protocol::EnvironmentInfoResponse;
use maplecode_app_server_protocol::EnvironmentShellInfo;
use maplecode_app_server_protocol::EnvironmentStatusKind;
use maplecode_app_server_protocol::EnvironmentStatusParams;
use maplecode_app_server_protocol::EnvironmentStatusResponse;
use maplecode_app_server_protocol::ExperimentalFeature as ApiExperimentalFeature;
use maplecode_app_server_protocol::ExperimentalFeatureListParams;
use maplecode_app_server_protocol::ExperimentalFeatureListResponse;
use maplecode_app_server_protocol::ExperimentalFeatureStage as ApiExperimentalFeatureStage;
use maplecode_app_server_protocol::FeedbackUploadParams;
use maplecode_app_server_protocol::FeedbackUploadResponse;
use maplecode_app_server_protocol::GetAccountParams;
use maplecode_app_server_protocol::GetAccountRateLimitsResponse;
use maplecode_app_server_protocol::GetAccountResponse;
use maplecode_app_server_protocol::GetAccountTokenUsageParams;
use maplecode_app_server_protocol::GetAccountTokenUsageResponse;
use maplecode_app_server_protocol::GetAuthStatusParams;
use maplecode_app_server_protocol::GetAuthStatusResponse;
use maplecode_app_server_protocol::GetConversationSummaryParams;
use maplecode_app_server_protocol::GetConversationSummaryResponse;
use maplecode_app_server_protocol::GetWorkspaceMessagesResponse;
use maplecode_app_server_protocol::GitDiffToRemoteParams;
use maplecode_app_server_protocol::GitDiffToRemoteResponse;
use maplecode_app_server_protocol::GitInfo as ApiGitInfo;
use maplecode_app_server_protocol::HookHandlerMetadata;
use maplecode_app_server_protocol::HookMetadata;
use maplecode_app_server_protocol::HooksListParams;
use maplecode_app_server_protocol::HooksListResponse;
use maplecode_app_server_protocol::InitializeParams;
use maplecode_app_server_protocol::InitializeResponse;
use maplecode_app_server_protocol::InstalledApp;
use maplecode_app_server_protocol::JSONRPCErrorError;
use maplecode_app_server_protocol::ListMcpServerStatusParams;
use maplecode_app_server_protocol::ListMcpServerStatusResponse;
use maplecode_app_server_protocol::LoginAccountParams;
use maplecode_app_server_protocol::LoginAccountResponse;
use maplecode_app_server_protocol::LoginApiKeyParams;
use maplecode_app_server_protocol::LoginAppBrand;
use maplecode_app_server_protocol::LogoutAccountResponse;
use maplecode_app_server_protocol::MapleCodeErrorInfo;
use maplecode_app_server_protocol::MarketplaceAddParams;
use maplecode_app_server_protocol::MarketplaceAddResponse;
use maplecode_app_server_protocol::MarketplaceInterface;
use maplecode_app_server_protocol::MarketplaceRemoveParams;
use maplecode_app_server_protocol::MarketplaceRemoveResponse;
use maplecode_app_server_protocol::MarketplaceUpgradeErrorInfo;
use maplecode_app_server_protocol::MarketplaceUpgradeParams;
use maplecode_app_server_protocol::MarketplaceUpgradeResponse;
use maplecode_app_server_protocol::McpResourceReadParams;
use maplecode_app_server_protocol::McpResourceReadResponse;
use maplecode_app_server_protocol::McpServerOauthClientRegistration;
use maplecode_app_server_protocol::McpServerOauthLoginCompletedNotification;
use maplecode_app_server_protocol::McpServerOauthLoginParams;
use maplecode_app_server_protocol::McpServerOauthLoginResponse;
use maplecode_app_server_protocol::McpServerRefreshResponse;
use maplecode_app_server_protocol::McpServerStatus;
use maplecode_app_server_protocol::McpServerStatusDetail;
use maplecode_app_server_protocol::McpServerToolCallParams;
use maplecode_app_server_protocol::McpServerToolCallResponse;
use maplecode_app_server_protocol::MemoryResetResponse;
use maplecode_app_server_protocol::MockExperimentalMethodParams;
use maplecode_app_server_protocol::MockExperimentalMethodResponse;
use maplecode_app_server_protocol::ModelListParams;
use maplecode_app_server_protocol::ModelListResponse;
use maplecode_app_server_protocol::ModelProviderError;
use maplecode_app_server_protocol::PermissionProfileListParams;
use maplecode_app_server_protocol::PermissionProfileListResponse;
use maplecode_app_server_protocol::PermissionProfileSummary;
use maplecode_app_server_protocol::PluginDetail;
use maplecode_app_server_protocol::PluginInstallParams;
use maplecode_app_server_protocol::PluginInstallResponse;
use maplecode_app_server_protocol::PluginInstalledParams;
use maplecode_app_server_protocol::PluginInstalledResponse;
use maplecode_app_server_protocol::PluginInterface;
use maplecode_app_server_protocol::PluginListMarketplaceKind;
use maplecode_app_server_protocol::PluginListParams;
use maplecode_app_server_protocol::PluginListResponse;
use maplecode_app_server_protocol::PluginMarketplaceEntry;
use maplecode_app_server_protocol::PluginReadParams;
use maplecode_app_server_protocol::PluginReadResponse;
use maplecode_app_server_protocol::PluginShareCheckoutParams;
use maplecode_app_server_protocol::PluginShareCheckoutResponse;
use maplecode_app_server_protocol::PluginShareContext;
use maplecode_app_server_protocol::PluginShareDeleteParams;
use maplecode_app_server_protocol::PluginShareDeleteResponse;
use maplecode_app_server_protocol::PluginShareDiscoverability;
use maplecode_app_server_protocol::PluginShareListItem;
use maplecode_app_server_protocol::PluginShareListParams;
use maplecode_app_server_protocol::PluginShareListResponse;
use maplecode_app_server_protocol::PluginSharePrincipal;
use maplecode_app_server_protocol::PluginSharePrincipalType;
use maplecode_app_server_protocol::PluginShareSaveParams;
use maplecode_app_server_protocol::PluginShareSaveResponse;
use maplecode_app_server_protocol::PluginShareTarget;
use maplecode_app_server_protocol::PluginShareUpdateDiscoverability;
use maplecode_app_server_protocol::PluginShareUpdateTargetsParams;
use maplecode_app_server_protocol::PluginShareUpdateTargetsResponse;
use maplecode_app_server_protocol::PluginSkillReadParams;
use maplecode_app_server_protocol::PluginSkillReadResponse;
use maplecode_app_server_protocol::PluginSource;
use maplecode_app_server_protocol::PluginSummary;
use maplecode_app_server_protocol::PluginUninstallParams;
use maplecode_app_server_protocol::PluginUninstallResponse;
use maplecode_app_server_protocol::RateLimitResetCredit;
use maplecode_app_server_protocol::RateLimitResetCreditStatus;
use maplecode_app_server_protocol::RateLimitResetCreditsSummary;
use maplecode_app_server_protocol::RateLimitResetType;
use maplecode_app_server_protocol::RequestId;
use maplecode_app_server_protocol::ReviewStartParams;
use maplecode_app_server_protocol::ReviewStartResponse;
use maplecode_app_server_protocol::ReviewTarget as ApiReviewTarget;
use maplecode_app_server_protocol::SandboxMode;
use maplecode_app_server_protocol::SendAddCreditsNudgeEmailParams;
use maplecode_app_server_protocol::SendAddCreditsNudgeEmailResponse;
use maplecode_app_server_protocol::ServerNotification;
use maplecode_app_server_protocol::SkillSummary;
use maplecode_app_server_protocol::SkillsConfigWriteParams;
use maplecode_app_server_protocol::SkillsConfigWriteResponse;
use maplecode_app_server_protocol::SkillsExtraRootsSetParams;
use maplecode_app_server_protocol::SkillsExtraRootsSetResponse;
use maplecode_app_server_protocol::SkillsListParams;
use maplecode_app_server_protocol::SkillsListResponse;
use maplecode_app_server_protocol::SortDirection;
use maplecode_app_server_protocol::Thread;
use maplecode_app_server_protocol::ThreadApproveGuardianDeniedActionParams;
use maplecode_app_server_protocol::ThreadApproveGuardianDeniedActionResponse;
use maplecode_app_server_protocol::ThreadArchiveParams;
use maplecode_app_server_protocol::ThreadArchiveResponse;
use maplecode_app_server_protocol::ThreadArchivedNotification;
use maplecode_app_server_protocol::ThreadBackgroundTerminal;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsCleanParams;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsCleanResponse;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsListParams;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsListResponse;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsTerminateParams;
use maplecode_app_server_protocol::ThreadBackgroundTerminalsTerminateResponse;
use maplecode_app_server_protocol::ThreadClosedNotification;
use maplecode_app_server_protocol::ThreadCompactStartParams;
use maplecode_app_server_protocol::ThreadCompactStartResponse;
use maplecode_app_server_protocol::ThreadDecrementElicitationParams;
use maplecode_app_server_protocol::ThreadDecrementElicitationResponse;
use maplecode_app_server_protocol::ThreadDeleteParams;
use maplecode_app_server_protocol::ThreadDeleteResponse;
use maplecode_app_server_protocol::ThreadDeletedNotification;
use maplecode_app_server_protocol::ThreadForkParams;
use maplecode_app_server_protocol::ThreadForkResponse;
use maplecode_app_server_protocol::ThreadGoal;
use maplecode_app_server_protocol::ThreadGoalClearParams;
use maplecode_app_server_protocol::ThreadGoalClearResponse;
use maplecode_app_server_protocol::ThreadGoalClearedNotification;
use maplecode_app_server_protocol::ThreadGoalGetParams;
use maplecode_app_server_protocol::ThreadGoalGetResponse;
use maplecode_app_server_protocol::ThreadGoalSetParams;
use maplecode_app_server_protocol::ThreadGoalSetResponse;
use maplecode_app_server_protocol::ThreadGoalStatus;
use maplecode_app_server_protocol::ThreadGoalUpdatedNotification;
use maplecode_app_server_protocol::ThreadHistoryBuilder;
#[cfg(test)]
use maplecode_app_server_protocol::ThreadHistoryMode;
use maplecode_app_server_protocol::ThreadIncrementElicitationParams;
use maplecode_app_server_protocol::ThreadIncrementElicitationResponse;
use maplecode_app_server_protocol::ThreadInjectItemsParams;
use maplecode_app_server_protocol::ThreadInjectItemsResponse;
use maplecode_app_server_protocol::ThreadItemEntry;
use maplecode_app_server_protocol::ThreadItemPayload;
use maplecode_app_server_protocol::ThreadItemsListParams;
use maplecode_app_server_protocol::ThreadItemsListResponse;
use maplecode_app_server_protocol::ThreadItemsReadParams;
use maplecode_app_server_protocol::ThreadItemsReadResponse;
use maplecode_app_server_protocol::ThreadListCwdFilter;
use maplecode_app_server_protocol::ThreadListParams;
use maplecode_app_server_protocol::ThreadListResponse;
use maplecode_app_server_protocol::ThreadLoadedListParams;
use maplecode_app_server_protocol::ThreadLoadedListResponse;
use maplecode_app_server_protocol::ThreadMemoryModeSetParams;
use maplecode_app_server_protocol::ThreadMemoryModeSetResponse;
use maplecode_app_server_protocol::ThreadMetadataGitInfoUpdateParams;
use maplecode_app_server_protocol::ThreadMetadataUpdateParams;
use maplecode_app_server_protocol::ThreadMetadataUpdateResponse;
use maplecode_app_server_protocol::ThreadNameUpdatedNotification;
use maplecode_app_server_protocol::ThreadProjectUpdatedNotification;
use maplecode_app_server_protocol::ThreadReadParams;
use maplecode_app_server_protocol::ThreadReadResponse;
use maplecode_app_server_protocol::ThreadRealtimeAppendAudioParams;
use maplecode_app_server_protocol::ThreadRealtimeAppendAudioResponse;
use maplecode_app_server_protocol::ThreadRealtimeAppendSpeechParams;
use maplecode_app_server_protocol::ThreadRealtimeAppendSpeechResponse;
use maplecode_app_server_protocol::ThreadRealtimeAppendTextParams;
use maplecode_app_server_protocol::ThreadRealtimeAppendTextResponse;
use maplecode_app_server_protocol::ThreadRealtimeListVoicesResponse;
use maplecode_app_server_protocol::ThreadRealtimeStartParams;
use maplecode_app_server_protocol::ThreadRealtimeStartResponse;
use maplecode_app_server_protocol::ThreadRealtimeStartTransport;
use maplecode_app_server_protocol::ThreadRealtimeStopParams;
use maplecode_app_server_protocol::ThreadRealtimeStopResponse;
use maplecode_app_server_protocol::ThreadResumeInitialTurnsPageParams;
use maplecode_app_server_protocol::ThreadResumeParams;
use maplecode_app_server_protocol::ThreadResumeResponse;
use maplecode_app_server_protocol::ThreadSearchOccurrence;
use maplecode_app_server_protocol::ThreadSearchOccurrencesParams;
use maplecode_app_server_protocol::ThreadSearchOccurrencesResponse;
use maplecode_app_server_protocol::ThreadSearchParams;
use maplecode_app_server_protocol::ThreadSearchResponse;
use maplecode_app_server_protocol::ThreadSearchResult;
use maplecode_app_server_protocol::ThreadSearchSortKey;
use maplecode_app_server_protocol::ThreadSearchTextRange;
use maplecode_app_server_protocol::ThreadSetNameParams;
use maplecode_app_server_protocol::ThreadSetNameResponse;
use maplecode_app_server_protocol::ThreadSettings;
use maplecode_app_server_protocol::ThreadSettingsUpdateParams;
use maplecode_app_server_protocol::ThreadSettingsUpdateResponse;
use maplecode_app_server_protocol::ThreadShellCommandParams;
use maplecode_app_server_protocol::ThreadShellCommandResponse;
use maplecode_app_server_protocol::ThreadSortKey;
use maplecode_app_server_protocol::ThreadSourceKind;
use maplecode_app_server_protocol::ThreadStartParams;
use maplecode_app_server_protocol::ThreadStartResponse;
use maplecode_app_server_protocol::ThreadStartedNotification;
use maplecode_app_server_protocol::ThreadStatus;
use maplecode_app_server_protocol::ThreadTimelineListParams;
use maplecode_app_server_protocol::ThreadTimelineListResponse;
use maplecode_app_server_protocol::ThreadTurnsListParams;
use maplecode_app_server_protocol::ThreadTurnsListResponse;
use maplecode_app_server_protocol::ThreadUnarchiveParams;
use maplecode_app_server_protocol::ThreadUnarchiveResponse;
use maplecode_app_server_protocol::ThreadUnarchivedNotification;
use maplecode_app_server_protocol::ThreadUnsubscribeParams;
use maplecode_app_server_protocol::ThreadUnsubscribeResponse;
use maplecode_app_server_protocol::ThreadUnsubscribeStatus;
use maplecode_app_server_protocol::Turn;
use maplecode_app_server_protocol::TurnEnvironmentParams;
use maplecode_app_server_protocol::TurnError;
use maplecode_app_server_protocol::TurnInterruptParams;
use maplecode_app_server_protocol::TurnInterruptResponse;
use maplecode_app_server_protocol::TurnItemsView;
use maplecode_app_server_protocol::TurnSettingsUpdateParams;
use maplecode_app_server_protocol::TurnSettingsUpdateResponse;
use maplecode_app_server_protocol::TurnSettingsUpdateStatus;
use maplecode_app_server_protocol::TurnStartParams;
use maplecode_app_server_protocol::TurnStartResponse;
use maplecode_app_server_protocol::TurnStatus;
use maplecode_app_server_protocol::TurnSteerParams;
use maplecode_app_server_protocol::TurnSteerResponse;
use maplecode_app_server_protocol::UserInput as V2UserInput;
use maplecode_app_server_protocol::WindowsSandboxReadiness;
use maplecode_app_server_protocol::WindowsSandboxReadinessResponse;
use maplecode_app_server_protocol::WindowsSandboxSetupCompletedNotification;
use maplecode_app_server_protocol::WindowsSandboxSetupMode;
use maplecode_app_server_protocol::WindowsSandboxSetupStartParams;
use maplecode_app_server_protocol::WindowsSandboxSetupStartResponse;
use maplecode_app_server_protocol::WorkspaceMessage;
use maplecode_app_server_protocol::WorkspaceMessageType;
use maplecode_arg0::Arg0DispatchPaths;
use maplecode_backend_client::AddCreditsNudgeCreditType as BackendAddCreditsNudgeCreditType;
use maplecode_backend_client::Client as BackendClient;
use maplecode_backend_client::ConsumeRateLimitResetCreditCode as BackendConsumeRateLimitResetCreditCode;
use maplecode_backend_client::MapleCodeWorkspaceMessage as BackendWorkspaceMessage;
use maplecode_backend_client::MapleCodeWorkspaceMessageType as BackendWorkspaceMessageType;
use maplecode_backend_client::MapleCodeWorkspaceMessagesResponse as BackendWorkspaceMessagesResponse;
use maplecode_backend_client::RateLimitResetCreditDetails as BackendRateLimitResetCreditDetails;
use maplecode_backend_client::RateLimitResetCreditsDetails as BackendRateLimitResetCreditsDetails;
use maplecode_backend_client::RequestError as BackendRequestError;
use maplecode_backend_client::TokenUsageProfile;
use maplecode_chatgpt::connectors;
use maplecode_config::CloudConfigBundleLoadError;
use maplecode_config::CloudConfigBundleLoadErrorCode;
use maplecode_config::ConfigLayerStack;
use maplecode_config::loader::project_trust_key;
use maplecode_config::types::McpServerTransportConfig;
use maplecode_connectors::AppInfo;
use maplecode_core::ForkSnapshot;
use maplecode_core::MapleCodeThread;
use maplecode_core::MapleCodeThreadSettingsOverrides;
use maplecode_core::McpManager;
use maplecode_core::NewThread;
use maplecode_core::NotSubmittedReason;
#[cfg(test)]
use maplecode_core::SessionMeta;
use maplecode_core::StartThreadOptions;
use maplecode_core::SteerSubmission;
use maplecode_core::ThreadConfigSnapshot;
use maplecode_core::ThreadManager;
use maplecode_core::TurnInput;
use maplecode_core::TurnInputRequest;
use maplecode_core::TurnInputSubmission;
use maplecode_core::TurnStartOptions;
use maplecode_core::config::Config;
use maplecode_core::config::ConfigOverrides;
use maplecode_core::config::NetworkProxyAuditMetadata;
use maplecode_core::config::edit::ConfigEdit;
use maplecode_core::config::edit::ConfigEditsBuilder;
use maplecode_core::connectors::AccessibleConnectorsStatus;
use maplecode_core::exec::ExecCapturePolicy;
use maplecode_core::exec::ExecExpiration;
use maplecode_core::exec::ExecParams;
use maplecode_core::exec_env::create_env;
use maplecode_core::path_utils;
#[cfg(test)]
use maplecode_core::read_head_for_summary;
use maplecode_core::sandboxing::SandboxPermissions;
use maplecode_core::truncate_rollout_after_turn_id;
use maplecode_core::truncate_rollout_before_turn_id;
use maplecode_core::validate_environment_ids_and_cwds;
use maplecode_core::windows_sandbox::WindowsSandboxLevelExt;
use maplecode_core::windows_sandbox::WindowsSandboxSetupMode as CoreWindowsSandboxSetupMode;
use maplecode_core::windows_sandbox::WindowsSandboxSetupRequest;
use maplecode_core::windows_sandbox::sandbox_setup_is_complete;
use maplecode_core_plugins::PluginInstallError as CorePluginInstallError;
use maplecode_core_plugins::PluginInstallRequest;
use maplecode_core_plugins::PluginReadRequest;
use maplecode_core_plugins::PluginUninstallError as CorePluginUninstallError;
use maplecode_core_plugins::PluginsManager;
use maplecode_core_plugins::loader::load_plugin_apps;
use maplecode_core_plugins::manifest::PluginManifestInterface;
use maplecode_core_plugins::marketplace::MarketplaceError;
use maplecode_core_plugins::marketplace::MarketplacePluginSource;
use maplecode_core_plugins::marketplace_add::MarketplaceAddError;
use maplecode_core_plugins::marketplace_add::MarketplaceAddRequest;
use maplecode_core_plugins::marketplace_add::add_marketplace as add_marketplace_to_maplecode_home;
use maplecode_core_plugins::marketplace_remove::MarketplaceRemoveError;
use maplecode_core_plugins::marketplace_remove::MarketplaceRemoveRequest as CoreMarketplaceRemoveRequest;
use maplecode_core_plugins::marketplace_remove::remove_marketplace;
use maplecode_core_plugins::remote::RemoteMarketplace;
use maplecode_core_plugins::remote::RemoteMarketplaceSource;
use maplecode_core_plugins::remote::RemotePluginCatalogError;
use maplecode_core_plugins::remote::RemotePluginDetail as RemoteCatalogPluginDetail;
use maplecode_core_plugins::remote::RemotePluginServiceConfig;
use maplecode_core_plugins::remote::RemotePluginShareContext as RemoteCatalogPluginShareContext;
use maplecode_core_plugins::remote::RemotePluginShareSummary as RemoteCatalogPluginShareSummary;
use maplecode_core_plugins::remote::RemotePluginSummary as RemoteCatalogPluginSummary;
use maplecode_exec_server::EnvironmentManager;
use maplecode_exec_server::EnvironmentObservedStatus;
use maplecode_exec_server::LOCAL_ENVIRONMENT_ID;
use maplecode_exec_server::LOCAL_FS;
use maplecode_features::FEATURES;
use maplecode_features::Feature;
use maplecode_features::Stage;
use maplecode_feedback::FeedbackAttachmentPath;
use maplecode_feedback::FeedbackUploadOptions;
use maplecode_feedback::MapleCodeFeedback;
use maplecode_git_utils::git_diff_to_remote;
use maplecode_git_utils::resolve_root_git_project_for_trust;
use maplecode_login::AuthManager;
use maplecode_login::LoginSuccessPage;
use maplecode_login::LoginSuccessPageBrand;
use maplecode_login::MAPLECODE_OPEN_APP_URL;
use maplecode_login::MapleCodeAuth;
use maplecode_login::ServerOptions as LoginServerOptions;
use maplecode_login::ShutdownHandle;
use maplecode_login::complete_device_code_login;
use maplecode_login::login_with_api_key;
use maplecode_login::login_with_bedrock_api_key;
use maplecode_login::oauth_client_id;
use maplecode_login::request_device_code;
use maplecode_login::run_login_server;
use maplecode_mcp::McpRuntimeContext;
use maplecode_mcp::McpServerStatusSnapshot;
use maplecode_mcp::McpSnapshotDetail;
use maplecode_mcp::collect_mcp_server_status_snapshot_with_detail;
use maplecode_mcp::discover_supported_scopes;
use maplecode_mcp::read_mcp_resource as read_mcp_resource_without_thread;
use maplecode_mcp::resolve_oauth_scopes;
use maplecode_memories_write::clear_memory_roots_contents;
use maplecode_model_provider::create_model_provider;
use maplecode_models_manager::collaboration_mode_presets::builtin_collaboration_mode_presets;
use maplecode_protocol::ThreadId;
use maplecode_protocol::config_types::CollaborationMode;
use maplecode_protocol::config_types::ForcedLoginMethod;
use maplecode_protocol::config_types::Personality;
use maplecode_protocol::config_types::ReasoningSummary;
use maplecode_protocol::config_types::TrustLevel;
use maplecode_protocol::config_types::WindowsSandboxLevel;
use maplecode_protocol::error::MapleCodeErr;
use maplecode_protocol::error::Result as MapleCodeResult;
#[cfg(test)]
use maplecode_protocol::items::TurnItem;
use maplecode_protocol::models::ResponseItem;
use maplecode_protocol::openai_models::ReasoningEffort;
use maplecode_protocol::protocol::AdditionalContextEntry as CoreAdditionalContextEntry;
use maplecode_protocol::protocol::AdditionalContextKind as CoreAdditionalContextKind;
use maplecode_protocol::protocol::AgentStatus;
use maplecode_protocol::protocol::ConversationAudioParams;
use maplecode_protocol::protocol::ConversationSpeechParams;
use maplecode_protocol::protocol::ConversationStartParams;
use maplecode_protocol::protocol::ConversationStartTransport;
use maplecode_protocol::protocol::ConversationTextParams;
use maplecode_protocol::protocol::EnvironmentConfigState;
use maplecode_protocol::protocol::EventMsg;
#[cfg(test)]
use maplecode_protocol::protocol::GitInfo as CoreGitInfo;
use maplecode_protocol::protocol::McpAuthStatus as CoreMcpAuthStatus;
use maplecode_protocol::protocol::Op;
use maplecode_protocol::protocol::RealtimeVoicesList;
use maplecode_protocol::protocol::ReviewRequest;
use maplecode_protocol::protocol::ReviewTarget as CoreReviewTarget;
use maplecode_protocol::protocol::SessionConfiguredEvent;
#[cfg(test)]
use maplecode_protocol::protocol::SessionMetaLine;
use maplecode_protocol::protocol::TurnEnvironmentRequest;
use maplecode_protocol::protocol::W3cTraceContext;
#[cfg(test)]
use maplecode_protocol::protocol::strip_user_message_prefix;
use maplecode_protocol::user_input::MAX_USER_INPUT_TEXT_CHARS;
use maplecode_protocol::user_input::UserInput as CoreInputItem;
use maplecode_rmcp_client::McpOAuthClientRegistration;
use maplecode_rmcp_client::StreamableHttpRedirectMode;
use maplecode_rmcp_client::perform_oauth_login_return_url;
use maplecode_rollout::InitialHistory;
use maplecode_rollout::ResumedHistory;
use maplecode_rollout::RolloutItem;
use maplecode_rollout::persisted_rollout_item;
use maplecode_rollout::state_db::StateDbHandle;
use maplecode_rollout::state_db::reconcile_rollout;
use maplecode_state::ThreadMetadata;
use maplecode_state::log_db::LogDbLayer;
use maplecode_thread_store::ArchiveThreadParams as StoreArchiveThreadParams;
use maplecode_thread_store::ArchiveThreadsParams as StoreArchiveThreadsParams;
use maplecode_thread_store::ClearableField as StoreClearableField;
use maplecode_thread_store::DeleteThreadsParams as StoreDeleteThreadsParams;
use maplecode_thread_store::GitInfoPatch as StoreGitInfoPatch;
use maplecode_thread_store::ItemSortKey as StoreItemSortKey;
use maplecode_thread_store::ListItemsParams as StoreListItemsParams;
use maplecode_thread_store::ListItemsPosition as StoreListItemsPosition;
use maplecode_thread_store::ListThreadsParams as StoreListThreadsParams;
use maplecode_thread_store::ListTimelineParams as StoreListTimelineParams;
use maplecode_thread_store::ListTurnsParams as StoreListTurnsParams;
use maplecode_thread_store::LoadThreadHistoryParams as StoreLoadThreadHistoryParams;
use maplecode_thread_store::LocalThreadStore;
use maplecode_thread_store::ReadThreadByRolloutPathParams as StoreReadThreadByRolloutPathParams;
use maplecode_thread_store::ReadThreadParams as StoreReadThreadParams;
use maplecode_thread_store::SearchThreadOccurrencesParams as StoreSearchThreadOccurrencesParams;
use maplecode_thread_store::SearchThreadsParams as StoreSearchThreadsParams;
use maplecode_thread_store::SortDirection as StoreSortDirection;
use maplecode_thread_store::StoredThread;
use maplecode_thread_store::StoredTurn;
use maplecode_thread_store::StoredTurnItemsView;
use maplecode_thread_store::StoredTurnStatus;
use maplecode_thread_store::ThreadMetadataPatch as StoreThreadMetadataPatch;
use maplecode_thread_store::ThreadRelationFilter as StoreThreadRelationFilter;
use maplecode_thread_store::ThreadSortKey as StoreThreadSortKey;
use maplecode_thread_store::ThreadStore;
use maplecode_thread_store::ThreadStoreError;
use maplecode_utils_absolute_path::AbsolutePathBuf;
use maplecode_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::Error as IoError;
use std::path::Path;
use std::path::PathBuf;
use std::result::Result;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::sync::Semaphore;
use tokio::sync::SemaphorePermit;
use tokio::sync::broadcast;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use tokio_util::sync::DropGuard;
use tokio_util::task::TaskTracker;
use toml::Value as TomlValue;
use tracing::Instrument;
use tracing::error;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

#[cfg(test)]
use maplecode_app_server_protocol::ServerRequest;

pub(crate) mod account_processor;
mod apps_processor;
mod bedrock_auth;
mod catalog_processor;
mod command_exec_processor;
pub(crate) mod config_processor;
#[cfg(test)]
#[path = "request_processors/config_reload_tests.rs"]
mod config_reload_tests;
mod diagnostics;
mod environment_processor;
mod feedback_doctor_report;
mod feedback_processor;
mod feedback_rollout_history;
mod feedback_thread_index;
mod fs_processor;
mod git_processor;
mod initialize_processor;
mod marketplace_processor;
mod mcp_event_stream;
mod mcp_processor;
mod memory_status;
mod persisted_resume_settings;
mod plugins;
mod process_exec_processor;
mod projects;
mod remote_control_processor;
mod rollout;
mod search;
mod thread_attachments;
mod thread_enrichment;
mod thread_fork_goal;
mod thread_input;
mod thread_memory;
mod thread_processor;
mod thread_queue_processor;
mod thread_read_state;
mod thread_sections;
mod token_usage_replay;
mod turn_processor;
mod windows_sandbox_processor;

pub(crate) use account_processor::AccountRequestProcessor;
pub(crate) use apps_processor::AppsRequestProcessor;
pub(crate) use catalog_processor::CatalogRequestProcessor;
pub(crate) use command_exec_processor::CommandExecRequestProcessor;
pub(crate) use config_processor::ConfigRequestProcessor;
pub(crate) use diagnostics::read_server_diagnostics;
pub(crate) use environment_processor::EnvironmentRequestProcessor;
pub(crate) use feedback_processor::FeedbackRequestProcessor;
pub(crate) use fs_processor::FsRequestProcessor;
pub(crate) use git_processor::GitRequestProcessor;
pub(crate) use initialize_processor::InitializeRequestProcessor;
pub(crate) use marketplace_processor::MarketplaceRequestProcessor;
pub(crate) use mcp_event_stream::McpEventStreamReady;
pub(crate) use mcp_event_stream::McpEventStreams;
pub(crate) use mcp_processor::McpRequestProcessor;
pub(crate) use plugins::PluginRequestProcessor;
pub(crate) use process_exec_processor::ProcessExecRequestProcessor;
pub(crate) use projects::ProjectRequestProcessor;
pub(crate) use remote_control_processor::RemoteControlRequestProcessor;
pub(crate) use search::SearchRequestProcessor;
pub(crate) use thread_goal_processor::ThreadGoalRequestProcessor;
pub(crate) use thread_processor::ThreadRequestProcessor;
pub(crate) use thread_processor::ThreadResumeTarget;
pub(crate) use thread_queue_processor::ThreadQueueRequestProcessor;
pub(crate) use turn_processor::TurnRequestProcessor;
pub(crate) use windows_sandbox_processor::WindowsSandboxRequestProcessor;

use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use crate::filters::compute_source_filters;
use crate::filters::source_kind_matches;
use crate::thread_state::ConnectionCapabilities;
use crate::thread_state::PendingThreadUnloads;
use crate::thread_state::ThreadListenerCommand;
use crate::thread_state::ThreadState;
use crate::thread_state::ThreadStateManager;
use token_usage_replay::restored_token_usage_turn_id;
use token_usage_replay::send_thread_token_usage_update_to_connection;

pub(crate) fn apply_live_thread_settings(
    thread: &mut Thread,
    config_snapshot: &ThreadConfigSnapshot,
) {
    thread.model = Some(config_snapshot.model.clone());
    thread.reasoning_effort = config_snapshot.reasoning_effort.clone();
    thread.environments = Some(
        config_snapshot
            .environment_selections()
            .iter()
            .map(Into::into)
            .collect(),
    );
}

fn resolve_request_cwd(cwd: Option<PathBuf>) -> Result<Option<AbsolutePathBuf>, JSONRPCErrorError> {
    cwd.map(|cwd| {
        AbsolutePathBuf::relative_to_current_dir(path_utils::normalize_for_native_workdir(cwd))
            .map_err(|err| invalid_request(format!("invalid cwd: {err}")))
    })
    .transpose()
}

fn resolve_turn_environment_requests(
    thread_manager: &ThreadManager,
    environments: Option<Vec<TurnEnvironmentParams>>,
) -> Result<Option<Vec<TurnEnvironmentRequest>>, JSONRPCErrorError> {
    let Some(environments) = environments else {
        return Ok(None);
    };
    let mut requests = Vec::with_capacity(environments.len());
    for environment in environments {
        let environment_id = environment.environment_id;
        let cwd = environment
            .cwd
            .to_inferred_path_uri()
            .ok_or_else(|| {
                invalid_request(format!(
                    "invalid cwd for environment `{environment_id}`: path `{}` does not use absolute POSIX or Windows path syntax",
                    environment.cwd
                ))
            })?;
        let workspace_roots = environment
            .runtime_workspace_roots
            .map(|roots| {
                let mut resolved_roots = Vec::new();
                for root in roots {
                    let root = root.to_inferred_path_uri().ok_or_else(|| {
                        invalid_request(format!(
                            "invalid runtime workspace root for environment `{environment_id}`: path `{root}` does not use absolute POSIX or Windows path syntax"
                        ))
                    })?;
                    if !resolved_roots.contains(&root) {
                        resolved_roots.push(root);
                    }
                }
                Ok::<_, JSONRPCErrorError>(resolved_roots)
            })
            .transpose()?
            .unwrap_or_else(|| vec![cwd.clone()]);
        requests.push(TurnEnvironmentRequest {
            environment_id,
            cwd,
            workspace_roots,
            config: EnvironmentConfigState::FromThread,
        });
    }
    validate_environment_ids_and_cwds(
        &thread_manager.environment_manager(),
        requests
            .iter()
            .map(|request| (request.environment_id.as_str(), &request.cwd)),
    )
    .map_err(environment_selection_error)?;
    Ok(Some(requests))
}

/// Project-local layers cannot define providers, so explicit selectors can use the startup map.
fn config_for_provider(
    startup: &Arc<Config>,
    provider_id: &str,
) -> Result<Arc<Config>, JSONRPCErrorError> {
    if startup.model_provider_id == provider_id {
        return Ok(Arc::clone(startup));
    }
    let provider = startup.model_providers.get(provider_id).ok_or_else(|| {
        model_provider_error(&ModelProviderError::ModelProviderNotConfigured {
            model_provider: provider_id.to_owned(),
        })
    })?;
    let mut config = startup.as_ref().clone();
    config.model_provider_id = provider_id.to_owned();
    config.model_provider = provider.clone();
    Ok(Arc::new(config))
}

fn resolve_runtime_workspace_roots(workspace_roots: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    let mut resolved_roots = Vec::new();
    for root in workspace_roots {
        if !resolved_roots.iter().any(|existing| existing == &root) {
            resolved_roots.push(root);
        }
    }
    resolved_roots
}

pub(crate) mod config_errors;
mod request_errors;
mod thread_delete;
mod thread_goal_processor;
mod thread_lifecycle;
mod thread_resume_redaction;
mod thread_summary;

use self::config_errors::*;
use self::request_errors::*;
use self::thread_goal_processor::api_thread_goal_from_state;
use self::thread_lifecycle::*;
use self::thread_resume_redaction::*;
use self::thread_summary::*;

pub(crate) use self::thread_lifecycle::populate_thread_turns_from_history;
#[cfg(test)]
pub(crate) use self::thread_summary::read_summary_from_rollout;
#[cfg(test)]
pub(crate) use self::thread_summary::summary_to_thread;
pub(crate) use self::thread_summary::thread_settings_from_config_snapshot;

pub(crate) fn build_legacy_api_turns_from_rollout_items(items: &[RolloutItem]) -> Vec<Turn> {
    let mut builder = ThreadHistoryBuilder::new();
    for item in items {
        if let Some(item) = persisted_rollout_item(
            item,
            maplecode_protocol::protocol::ThreadHistoryMode::Legacy,
        ) {
            builder.handle_rollout_item(item.as_ref());
        }
    }
    builder.finish()
}
