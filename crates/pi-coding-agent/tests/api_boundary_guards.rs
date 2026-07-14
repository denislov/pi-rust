use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn external_consumer_fixtures_enforce_the_stable_facade_boundary() {
    run_external_consumer_fixtures();
}

#[derive(Clone, Copy)]
struct CompileFixture {
    category: &'static str,
    access_path: &'static str,
    source: &'static str,
}

const FAIL_FIXTURES: [CompileFixture; 12] = [
    CompileFixture {
        category: "operation-dispatch",
        access_path: "api",
        source: "operation_dispatch_api.rs",
    },
    CompileFixture {
        category: "operation-dispatch",
        access_path: "root",
        source: "operation_dispatch_root.rs",
    },
    CompileFixture {
        category: "operation-dispatch",
        access_path: "doc-hidden",
        source: "operation_dispatch_hidden.rs",
    },
    CompileFixture {
        category: "services",
        access_path: "api",
        source: "services_api.rs",
    },
    CompileFixture {
        category: "services",
        access_path: "root",
        source: "services_root.rs",
    },
    CompileFixture {
        category: "services",
        access_path: "doc-hidden",
        source: "services_hidden.rs",
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "api",
        source: "plugins_api.rs",
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "root",
        source: "plugins_root.rs",
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "doc-hidden",
        source: "plugins_hidden.rs",
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "api",
        source: "flow_api.rs",
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "root",
        source: "flow_root.rs",
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "doc-hidden",
        source: "flow_hidden.rs",
    },
];

fn run_external_consumer_fixtures() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_root = crate_root.join("tests/fixtures/api_boundary");
    let consumer = tempfile::tempdir().expect("create external consumer directory");
    let source_dir = consumer.path().join("src");
    fs::create_dir(&source_dir).expect("create external consumer source directory");
    fs::write(
        consumer.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"pi-coding-agent-api-boundary-fixture\"\nversion = \"0.0.0\"\nedition = \"2024\"\n\n[dependencies]\npi-coding-agent = {{ path = {:?} }}\n",
            crate_root
        ),
    )
    .expect("write external consumer manifest");
    let workspace_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("pi-coding-agent should live below the workspace root");
    fs::copy(
        workspace_root.join("Cargo.lock"),
        consumer.path().join("Cargo.lock"),
    )
    .expect("copy the workspace lockfile for deterministic offline resolution");

    let positive = compile_fixture(consumer.path(), &fixture_root.join("pass/stable_facade.rs"));
    assert!(
        positive.status.success(),
        "stable facade external consumer should compile:\n{}",
        command_diagnostics(&positive)
    );

    let expected_matrix = FAIL_FIXTURES
        .iter()
        .map(|fixture| (fixture.category, fixture.access_path))
        .collect::<BTreeSet<_>>();
    assert_eq!(
        expected_matrix.len(),
        12,
        "negative fixture matrix must independently cover four categories through three paths"
    );

    for fixture in FAIL_FIXTURES {
        let output = compile_fixture(
            consumer.path(),
            &fixture_root.join("fail").join(fixture.source),
        );
        let diagnostics = command_diagnostics(&output);
        assert!(
            !output.status.success(),
            "{} must remain inaccessible through the {} path",
            fixture.category,
            fixture.access_path
        );
        assert!(
            diagnostics.contains("error[E0432]") || diagnostics.contains("error[E0603]"),
            "{} through {} should fail because an import is unresolved or private, not for an unrelated compiler reason:\n{}",
            fixture.category,
            fixture.access_path,
            diagnostics
        );
    }
}

fn compile_fixture(consumer_root: &Path, fixture: &Path) -> Output {
    fs::copy(fixture, consumer_root.join("src/main.rs")).unwrap_or_else(|error| {
        panic!("copy fixture {}: {error}", fixture.display());
    });
    Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet"])
        .current_dir(consumer_root)
        .env("CARGO_TARGET_DIR", consumer_root.join("target"))
        .output()
        .unwrap_or_else(|error| panic!("run Cargo for fixture {}: {error}", fixture.display()))
}

fn command_diagnostics(output: &Output) -> String {
    format!(
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

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
        "run_operation(operation, submission).await",
        "run_sync_operation(operation, submission)",
        "run_sync_mut_operation(operation, submission)",
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

#[test]
fn client_connection_is_stateful_but_not_a_dispatcher_or_service_escape_hatch() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_root.join("src/coding_session/public_projection.rs"))
        .expect("public projection source should be readable");
    let connection = source
        .split("pub struct CodingAgentClientConnection")
        .nth(1)
        .expect("public connection should exist")
        .split("pub struct CodingAgentReconnectReceiver")
        .next()
        .unwrap();
    assert!(connection.contains("coordinator: Arc<SnapshotCoordinator>"));
    assert!(connection.contains("prepare_submission("));
    for forbidden in [
        "pub async fn run(",
        "pub async fn submit(",
        "RuntimeService",
        "SessionService",
        "ProductEventReceiver",
    ] {
        assert!(
            !connection.contains(forbidden),
            "connection leaked {forbidden}"
        );
    }
}

#[test]
fn public_lifecycle_values_are_curated_without_authority_leaks() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs"))
        .expect("pi-coding-agent lib.rs should be readable");
    let projection = fs::read_to_string(crate_root.join("src/coding_session/public_projection.rs"))
        .expect("public projection should be readable");
    let errors = fs::read_to_string(crate_root.join("src/coding_session/error.rs"))
        .expect("coding session errors should be readable");
    let api_module = module_body(&lib_source, "pub mod api")
        .expect("stable api module should have a balanced body");
    let exported_identifiers = api_module
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .filter(|identifier| !identifier.is_empty())
        .collect::<BTreeSet<_>>();

    for required in [
        "CodingAgentDetachOutcome",
        "CodingAgentShutdownOutcome",
        "CodingAgentLifecycleRejection",
        "CodingAgentSubmittedEventDurability",
        "CodingAgentOutcomeAcknowledgementId",
        "CodingAgentSubmittedTerminalAnchor",
        "CodingAgentTerminalUncertainty",
    ] {
        assert!(
            exported_identifiers.contains(required),
            "stable api omitted adjacent lifecycle value {required}"
        );
    }

    for forbidden in [
        "SnapshotCoordinator",
        "ClientHandle",
        "ClientGeneration",
        "EventService",
        "ProductEventReceiver",
        "OperationControl",
        "Sender",
        "Receiver",
        "HashMap",
        "BTreeMap",
        "VecDeque",
        "LifecycleEpoch",
        "ReceiptSignature",
    ] {
        assert!(
            !exported_identifiers.contains(forbidden),
            "stable lifecycle api leaked internal authority {forbidden}"
        );
    }

    let acknowledgement = projection
        .split("pub struct CodingAgentOutcomeAcknowledgementId")
        .nth(1)
        .expect("opaque outcome acknowledgement should exist")
        .split("pub enum CodingAgentTerminalUncertainty")
        .next()
        .expect("terminal uncertainty should follow acknowledgement");
    assert!(acknowledgement.starts_with("(String);"));
    assert!(!acknowledgement.contains("pub fn new("));
    assert!(!acknowledgement.contains("pub fn generation("));
    assert!(!acknowledgement.contains("pub fn signature("));

    for source in [&projection, &errors] {
        for forbidden in ["format!(\"{:?}\"", "format!(\"{:#?}\""] {
            assert!(
                !source.contains(forbidden),
                "stable lifecycle identity/code must not use Debug formatting: {forbidden}"
            );
        }
    }

    for stable_code in [
        "\"detached\"",
        "\"stale_generation\"",
        "\"runtime_shut_down\"",
    ] {
        assert!(
            errors.contains(stable_code),
            "lifecycle rejection omitted explicit stable code {stable_code}"
        );
    }

    let connection = projection
        .split("impl CodingAgentClientConnection")
        .nth(1)
        .expect("public connection implementation should exist")
        .split("pub struct CodingAgentReconnectReceiver")
        .next()
        .expect("reconnect receiver should follow connection");
    for forbidden in [
        "pub async fn run(",
        "pub async fn submit(",
        "pub fn dispatch(",
        "pub fn detach_client(",
        "pub fn shutdown_client(",
    ] {
        assert!(
            !connection.contains(forbidden),
            "connection leaked lifecycle/operation authority through {forbidden}"
        );
    }
}

#[test]
fn public_lifecycle_connection_derives_detach_authority_from_self() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let projection = fs::read_to_string(crate_root.join("src/coding_session/public_projection.rs"))
        .expect("public projection should be readable");
    let connection = projection
        .split("impl CodingAgentClientConnection")
        .nth(1)
        .expect("public connection implementation should exist")
        .split("pub struct CodingAgentReconnectReceiver")
        .next()
        .unwrap();

    assert!(
        connection.contains(
            "pub fn detach(&self) -> Result<CodingAgentDetachOutcome, CodingSessionError>"
        )
    );
    assert!(connection.contains(".detach(&self.handle())"));
    assert!(!connection.contains("pub fn detach_client("));
    assert!(!connection.contains("pub fn detach_generation("));
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
