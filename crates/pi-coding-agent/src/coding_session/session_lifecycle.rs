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
        SessionService::hydrate(&options)
    }

    pub(crate) fn tree_view(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionTree, CodingSessionError> {
        SessionService::tree_view(&options)
    }

    pub(crate) fn clone_session(
        options: CodingAgentSessionOptions,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .clone_current()?
            .hydrated_view()
    }

    pub(crate) fn fork_session(
        options: CodingAgentSessionOptions,
        target_leaf_id: Option<&str>,
    ) -> Result<CodingAgentSessionHydration, CodingSessionError> {
        SessionService::open(&options)?
            .fork_current(target_leaf_id)?
            .hydrated_view()
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
    ) -> Result<Self, CodingSessionError> {
        let mut session_service = session_service;
        let replay_state = replay_derived_owner_state(&mut session_service)?;
        let snapshot_coordinator = SnapshotCoordinator::new();
        let event_service = EventService::with_snapshot_coordinator(snapshot_coordinator.clone());
        let client_service = ClientService::new(snapshot_coordinator.clone());

        let session = Self {
            persistence: SessionPersistence::Persistent(session_service),
            runtime_service,
            flow_service: FlowService::new(),
            event_service,
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            plugin_load_service: PluginLoadService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            pending_delegation_confirmations: replay_state.pending_delegation_confirmations,
            branch_summary_service: BranchSummaryService::new(),
            delegation_confirmation_service: DelegationConfirmationService::new(),
            delegation_execution_service: DelegationExecutionService::new(),
            manual_compaction_service: ManualCompactionService::new(),
            self_healing_edit_service: SelfHealingEditService::new(),
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
    ) -> Result<Self, CodingSessionError> {
        let snapshot_coordinator = SnapshotCoordinator::new();
        let client_service = ClientService::new(snapshot_coordinator.clone());
        let session = Self {
            persistence: SessionPersistence::NonPersistent(state),
            runtime_service,
            flow_service: FlowService::new(),
            event_service: EventService::with_snapshot_coordinator(snapshot_coordinator.clone()),
            capability_service: CapabilityService::new(),
            plugin_service: PluginService::new(),
            plugin_load_service: PluginLoadService::new(),
            profile_registry,
            default_plugin_load_options,
            operation_control: OperationControl::with_snapshot_coordinator(
                snapshot_coordinator.clone(),
            ),
            pending_delegation_confirmations: PendingDelegationConfirmationQueue::default(),
            branch_summary_service: BranchSummaryService::new(),
            delegation_confirmation_service: DelegationConfirmationService::new(),
            delegation_execution_service: DelegationExecutionService::new(),
            manual_compaction_service: ManualCompactionService::new(),
            self_healing_edit_service: SelfHealingEditService::new(),
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
