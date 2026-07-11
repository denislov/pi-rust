use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;

#[test]
fn root_public_modules_are_marked_migration_private() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let lines = lib_source.lines().collect::<Vec<_>>();
    let mut violations = Vec::new();

    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub mod ") {
            continue;
        }
        let module_name = trimmed
            .trim_start_matches("pub mod ")
            .trim_end_matches(';')
            .trim_end_matches('{')
            .trim();
        if module_name == "api" {
            continue;
        }

        let previous_non_empty = lines[..index]
            .iter()
            .rev()
            .find(|candidate| !candidate.trim().is_empty())
            .map(|candidate| candidate.trim());
        if previous_non_empty != Some("#[doc(hidden)]") {
            violations.push(format!("{}: {}", index + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "root public modules should be documented as migration-private via #[doc(hidden)] while pi_coding_agent::api remains the stable facade:\n{}",
        violations.join("\n")
    );
}

#[test]
fn root_reexports_are_explicit_compatibility_surface() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let before_api = lib_source
        .split("pub mod api {")
        .next()
        .expect("api module should exist");
    let before_api_lines = before_api.lines().collect::<Vec<_>>();

    let mut violations = Vec::new();
    for (index, line) in before_api_lines.iter().enumerate() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub use ") {
            continue;
        }
        let previous_non_empty = before_api_lines[..index]
            .iter()
            .rev()
            .find(|candidate| !candidate.trim().is_empty())
            .map(|candidate| candidate.trim());
        if previous_non_empty != Some("#[deprecated(note = \"use pi_coding_agent::api instead\")]")
        {
            violations.push(format!("{}: {}", index + 1, trimmed));
        }
    }

    assert!(
        violations.is_empty(),
        "root reexports should be explicitly deprecated compatibility surface; stable users should import pi_coding_agent::api:\n{}",
        violations.join("\n")
    );
}

#[test]
fn coding_session_run_is_the_canonical_operation_dispatcher() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/mod.rs"))
        .expect("coding session owner should be readable");
    let run_body =
        function_body(&source, "pub async fn run(").expect("CodingAgentSession::run should exist");

    for required in [
        "into_internal(",
        "operation.metadata().dispatch_mode",
        "OperationDispatchMode::Async",
        "OperationDispatchMode::SyncReadOnly",
        "OperationDispatchMode::SyncMutable",
        "run_operation(operation).await",
        "run_sync_operation(operation)",
        "run_sync_mut_operation(operation)",
        "CodingAgentOperationOutcome::from_internal(outcome)",
    ] {
        assert!(
            run_body.contains(required),
            "CodingAgentSession::run should contain {required}"
        );
    }

    for forbidden in [
        ".prompt(",
        ".compact(",
        ".summarize_branch(",
        ".self_healing_edit_with_options(",
        ".invoke_agent(",
        ".invoke_team(",
        ".export_current(",
        ".export_current_html(",
        "CodingAgentOperationOutcome::Prompt(",
        "CodingAgentOperationOutcome::Compact(",
        "CodingAgentOperationOutcome::BranchSummary(",
        "CodingAgentOperationOutcome::SelfHealingEdit(",
        "CodingAgentOperationOutcome::AgentInvocation(",
        "CodingAgentOperationOutcome::AgentTeam(",
        "CodingAgentOperationOutcome::PluginLoad(",
        "CodingAgentOperationOutcome::PluginCommand(",
        "CodingAgentOperationOutcome::DefaultAgentProfileChanged",
        "CodingAgentOperationOutcome::DelegationApproved",
        "CodingAgentOperationOutcome::DelegationRejected",
        "CodingAgentOperationOutcome::SessionForked",
        "CodingAgentOperationOutcome::ActiveLeafSwitched",
        "CodingAgentOperationOutcome::Export(",
        "CodingAgentOperationOutcome::ExportHtml(",
    ] {
        assert!(
            !run_body.contains(forbidden),
            "CodingAgentSession::run must not call compatibility workflow {forbidden}"
        );
    }
}

fn function_body<'a>(source: &'a str, signature: &str) -> Option<&'a str> {
    let signature_start = source.find(signature)?;
    let body_start = signature_start + source[signature_start..].find('{')?;
    let mut depth = 0usize;

    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&source[body_start + 1..body_start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}

#[test]
fn stable_api_does_not_export_compatibility_event_receiver() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let compatibility_receiver = ["CodingAgent", "EventReceiver"].concat();
    let api_module = lib_source
        .split("pub mod api {")
        .nth(1)
        .expect("api module should exist")
        .split("\n}\n\n#[cfg")
        .next()
        .expect("api module should end before test support");

    assert!(
        !api_module.contains(&compatibility_receiver),
        "stable api should export the product-event receiver instead of the compatibility receiver"
    );
}

#[test]
fn stable_api_excludes_internal_runtime_contracts() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let api_module = module_body(&lib_source, "pub mod api")
        .expect("stable api module should have a balanced body");
    let exported_identifiers = api_module
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .collect::<BTreeSet<_>>();

    for forbidden in [
        "Operation",
        "OperationMetadata",
        "OperationDispatchMode",
        "PluginLoadOptions",
        "RuntimeService",
        "SessionService",
        "EventService",
        "PluginService",
        "PluginLoadService",
        "CapabilityService",
        "FlowService",
        "ProfileRegistry",
        "ProfileRegistryOptions",
        "PluginRegistry",
        "Flow",
        "FlowNode",
        "FlowOutcome",
    ] {
        assert!(
            !exported_identifiers.contains(forbidden),
            "stable api must not re-export internal runtime contract {forbidden}"
        );
    }
}

fn module_body<'a>(source: &'a str, declaration: &str) -> Option<&'a str> {
    let declaration_start = source.find(declaration)?;
    let body_start = declaration_start + source[declaration_start..].find('{')?;
    let mut depth = 0usize;

    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&source[body_start + 1..body_start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}
