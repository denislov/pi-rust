use super::*;

impl CodingAgentSession {
    pub async fn create(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::create(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        let runtime_service = runtime_service_for_options(&options);
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
            runtime_service,
            options.tool_authorization_mode(),
        )
    }

    pub async fn open(options: CodingAgentSessionOptions) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        let runtime_service = runtime_service_for_options(&options);
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
            runtime_service,
            options.tool_authorization_mode(),
        )
    }

    pub async fn open_or_create(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        let session_service = SessionService::open_or_create(&options)?;
        let profile_registry = profile_registry_for_options(&options, Some(&session_service))?;
        let runtime_service = runtime_service_for_options(&options);
        Self::from_services(
            session_service,
            default_plugin_load_options(&options),
            profile_registry,
            runtime_service,
            options.tool_authorization_mode(),
        )
    }

    pub async fn non_persistent(
        options: CodingAgentSessionOptions,
    ) -> Result<Self, CodingSessionError> {
        if options.session_id().is_some() || options.session_path().is_some() {
            return Err(CodingSessionError::Input {
                message: "non-persistent coding sessions do not accept a session id or path".into(),
            });
        }
        Self::from_transient(
            TransientSessionState::new(option_default_agent_profile_id(&options)),
            default_plugin_load_options(&options),
            profile_registry_for_options(&options, None)?,
            runtime_service_for_options(&options),
            options.tool_authorization_mode(),
        )
    }

    #[cfg(test)]
    pub(crate) async fn non_persistent_with_event_capacity_for_tests(
        options: CodingAgentSessionOptions,
        event_capacity: usize,
    ) -> Result<Self, CodingSessionError> {
        let mut session = Self::non_persistent(options).await?;
        session.event_service = EventService::with_event_capacity_and_coordinator_for_tests(
            event_capacity,
            session.snapshot_coordinator.clone(),
        );
        session
            .authorization_service
            .set_event_service(session.event_service.clone());
        Ok(session)
    }

    #[cfg(test)]
    pub(crate) async fn non_persistent_with_event_capacities_for_tests(
        options: CodingAgentSessionOptions,
        channel_capacity: usize,
        retained_capacity: usize,
    ) -> Result<Self, CodingSessionError> {
        let mut session = Self::non_persistent(options).await?;
        session.event_service = EventService::with_event_capacities_and_coordinator_for_tests(
            channel_capacity,
            retained_capacity,
            session.snapshot_coordinator.clone(),
        );
        session
            .authorization_service
            .set_event_service(session.event_service.clone());
        Ok(session)
    }

    pub fn list(
        options: CodingAgentSessionOptions,
    ) -> Result<Vec<CodingAgentSessionSummary>, CodingSessionError> {
        SessionService::list(&options)
    }

    pub(crate) fn hydrate(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        crate::operations::session_navigation::hydrate(options)
    }

    pub(crate) fn tree_view(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionTree, CodingSessionError> {
        crate::operations::session_navigation::tree_view(options)
    }

    pub(crate) fn clone_session(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        crate::operations::session_navigation::clone_session(options)
    }

    pub(crate) fn fork_session(
        options: CodingAgentSessionOptions,
        target_leaf_id: Option<&str>,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        crate::operations::session_navigation::fork_session(options, target_leaf_id)
    }

    pub fn export_session_html(
        options: CodingAgentSessionOptions,
        path: impl AsRef<Path>,
    ) -> Result<PathBuf, CodingSessionError> {
        let session_service = SessionService::open(&options)?;
        let mut context = session_service.export_context(ExportOptions::html(path.as_ref()))?;
        let outcome = FlowService::new().run_export(&mut context)?;
        outcome.path.ok_or_else(|| CodingSessionError::Session {
            message: "export completed without a written html path".into(),
        })
    }

    fn from_services(
        session_service: SessionService,
        default_plugin_load_options: PluginLoadOptions,
        profile_registry: ProfileRegistry,
        runtime_service: RuntimeService,
        tool_authorization_mode: crate::authorization::ToolAuthorizationMode,
    ) -> Result<Self, CodingSessionError> {
        let mut session_service = session_service;
        let replay_state = replay_derived_owner_state(&mut session_service)?;
        let snapshot_coordinator = SnapshotCoordinator::new();
        let event_service = EventService::with_snapshot_coordinator(snapshot_coordinator.clone());
        let client_service = ClientService::new(snapshot_coordinator.clone());
        let authorization_service = AuthorizationService::new(
            tool_authorization_mode,
            snapshot_coordinator.clone(),
            event_service.clone(),
        );

        let session = Self {
            persistence: SessionPersistence::Persistent(session_service),
            runtime_service,
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            pending_delegation_confirmations: replay_state.pending_delegation_confirmations,
            authorization_service,
            capability_snapshots: CapabilitySnapshotService::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            snapshot_coordinator,
            client_service,
            pending_submission: None,
            startup_recovery_markers: Mutex::new(replay_state.startup_recovery_markers),
        };
        session.refresh_snapshot_projection();
        session
            .event_service
            .emit_session_opened(session.view().session_id);
        Ok(session)
    }

    fn from_transient(
        state: TransientSessionState,
        default_plugin_load_options: PluginLoadOptions,
        profile_registry: ProfileRegistry,
        runtime_service: RuntimeService,
        tool_authorization_mode: crate::authorization::ToolAuthorizationMode,
    ) -> Result<Self, CodingSessionError> {
        let snapshot_coordinator = SnapshotCoordinator::new();
        let client_service = ClientService::new(snapshot_coordinator.clone());
        let event_service = EventService::with_snapshot_coordinator(snapshot_coordinator.clone());
        let authorization_service = AuthorizationService::new(
            tool_authorization_mode,
            snapshot_coordinator.clone(),
            event_service.clone(),
        );
        let session = Self {
            persistence: SessionPersistence::NonPersistent(state),
            runtime_service,
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            pending_delegation_confirmations: PendingDelegationConfirmationQueue::default(),
            authorization_service,
            capability_snapshots: CapabilitySnapshotService::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            snapshot_coordinator,
            client_service,
            pending_submission: None,
            startup_recovery_markers: Mutex::new(Vec::new()),
        };
        session.refresh_snapshot_projection();
        Ok(session)
    }
}

fn default_plugin_load_options(options: &CodingAgentSessionOptions) -> PluginLoadOptions {
    let cwd = options
        .cwd()
        .map(Path::to_path_buf)
        .unwrap_or_else(default_cwd);
    let paths = crate::config::resolve_paths(&cwd);
    PluginLoadOptions::new()
        .with_discovery_root(paths.project_dir.join("plugins"), PluginSource::Project)
        .with_discovery_root(paths.global_dir.join("plugins"), PluginSource::User)
}

fn profile_registry_for_options(
    options: &CodingAgentSessionOptions,
    session_service: Option<&SessionService>,
) -> Result<ProfileRegistry, CodingSessionError> {
    let cwd = options
        .cwd()
        .map(Path::to_path_buf)
        .or_else(|| session_service.and_then(session_cwd))
        .unwrap_or_else(default_cwd);
    let paths = crate::config::resolve_paths(&cwd);
    ProfileRegistry::load(
        ProfileRegistryOptions::new()
            .with_user_root(paths.global_dir)
            .with_project_root(paths.project_dir),
    )
}

fn option_default_agent_profile_id(options: &CodingAgentSessionOptions) -> ProfileId {
    options
        .default_agent_profile_id()
        .cloned()
        .unwrap_or_else(|| ProfileId::from("default"))
}

fn runtime_service_for_options(options: &CodingAgentSessionOptions) -> RuntimeService {
    options
        .ai_client()
        .cloned()
        .map(RuntimeService::with_ai_client)
        .unwrap_or_else(RuntimeService::new)
}
