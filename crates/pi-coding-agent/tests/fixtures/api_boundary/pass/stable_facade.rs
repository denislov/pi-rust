#![allow(dead_code)]

use pi_coding_agent::api::runtime::{CodingAgentSessionOptions, CodingSessionError};
use pi_coding_agent::api::operation::{AgentInvocationOptions, AgentInvocationOutcome, AgentTeamMemberOutcome, AgentTeamOptions, AgentTeamOutcome, BranchSummaryReusePolicy, CodingAgentOperation, CodingAgentOperationOutcome, CodingAgentPluginLoadOutcome, PendingDelegationConfirmation, PromptTurnOptions, PromptTurnOutcome, SelfHealingEditOutcome, SelfHealingEditReplacement, SelfHealingEditRequest};
use pi_coding_agent::api::client::{CodingAgentSnapshot, CodingAgentSnapshotCursor};
use pi_coding_agent::api::view::{CodingAgentPluginDiagnostic, CodingAgentSessionExport, CodingAgentSessionSummary, CodingAgentSessionView, ProfileId};
use pi_coding_agent::api::cli::runtime::{PromptInvocation};

fn prompt() -> PromptTurnOptions {
    PromptTurnOptions::new(PromptInvocation::Text("fixture".into()))
}

fn operations() -> [CodingAgentOperation; 16] {
    [
        CodingAgentOperation::Prompt(prompt()),
        CodingAgentOperation::Compact(prompt()),
        CodingAgentOperation::BranchSummary {
            options: prompt(),
            source_leaf_id: "source".into(),
            target_leaf_id: "target".into(),
            custom_instructions: None,
            reuse: BranchSummaryReusePolicy::ReuseExisting,
        },
        CodingAgentOperation::SelfHealingEdit(SelfHealingEditRequest::new(
            "src/lib.rs",
            vec![SelfHealingEditReplacement::new("old", "new")],
        )),
        CodingAgentOperation::InvokeAgent(AgentInvocationOptions::new("agent", "task", prompt())),
        CodingAgentOperation::InvokeTeam(AgentTeamOptions::new("team", "task", prompt())),
        CodingAgentOperation::PluginLoad,
        CodingAgentOperation::PluginCommand {
            command_id: "plugin.command".into(),
            args: Default::default(),
        },
        CodingAgentOperation::SetDefaultAgentProfile {
            profile_id: ProfileId::from("agent"),
        },
        CodingAgentOperation::ApproveDelegation {
            operation_id: "operation".into(),
            tool_call_id: "tool".into(),
        },
        CodingAgentOperation::RejectDelegation {
            operation_id: "operation".into(),
            tool_call_id: "tool".into(),
            reason: "reason".into(),
        },
        CodingAgentOperation::ForkSession { target_leaf_id: None },
        CodingAgentOperation::SwitchActiveLeaf { target_leaf_id: "leaf".into() },
        CodingAgentOperation::SetSessionTreeLabel {
            entry_id: "leaf".into(),
            label: Some("checkpoint".into()),
        },
        CodingAgentOperation::ExportCurrent,
        CodingAgentOperation::ExportCurrentHtml("session.html".into()),
    ]
}

fn outcomes(outcome: CodingAgentOperationOutcome) {
    match outcome {
        CodingAgentOperationOutcome::Prompt(value) | CodingAgentOperationOutcome::Compact(value) => {
            touch(value)
        }
        CodingAgentOperationOutcome::BranchSummary(value) => touch(value),
        CodingAgentOperationOutcome::SelfHealingEdit(value) => touch(value),
        CodingAgentOperationOutcome::AgentInvocation(value) => touch(value),
        CodingAgentOperationOutcome::AgentTeam(value) => touch(value),
        CodingAgentOperationOutcome::PluginLoad(value) => touch(value),
        CodingAgentOperationOutcome::PluginCommand(value) => touch(value),
        CodingAgentOperationOutcome::DefaultAgentProfileChanged
        | CodingAgentOperationOutcome::DelegationApproved
        | CodingAgentOperationOutcome::DelegationRejected
        | CodingAgentOperationOutcome::SessionForked
        | CodingAgentOperationOutcome::ActiveLeafSwitched => {}
        CodingAgentOperationOutcome::SessionTreeLabelChanged {
            entry_id,
            label,
            updated_at,
        } => touch((entry_id, label, updated_at)),
        CodingAgentOperationOutcome::Export(value) => touch(value),
        CodingAgentOperationOutcome::ExportHtml(value) => touch(value),
    }
}

fn touch<T>(_: T) {}

fn support_types() {
    touch::<Option<PromptTurnOutcome>>(None);
    touch::<Option<SelfHealingEditOutcome>>(None);
    touch::<Option<AgentInvocationOutcome>>(None);
    touch::<Option<AgentTeamOutcome>>(None);
    touch::<Option<AgentTeamMemberOutcome>>(None);
    touch::<Option<CodingAgentPluginLoadOutcome>>(None);
    touch::<Option<CodingAgentPluginDiagnostic>>(None);
    touch::<Option<CodingAgentSessionExport>>(None);
    touch::<Option<CodingSessionError>>(None);
    touch::<Option<CodingAgentSessionOptions>>(None);
    touch::<Option<CodingAgentSessionSummary>>(None);
    touch::<Option<CodingAgentSessionView>>(None);
    touch::<Option<CodingAgentSnapshot>>(None);
    touch::<Option<CodingAgentSnapshotCursor>>(None);
    touch::<Option<PendingDelegationConfirmation>>(None);
}

fn main() {
    assert_eq!(operations().len(), 15);
    support_types();
}
