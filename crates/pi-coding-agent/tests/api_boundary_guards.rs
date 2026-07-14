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
    expected: ExpectedDiagnostic,
}

#[derive(Clone, Copy)]
struct ExpectedDiagnostic {
    code: &'static str,
    line: u64,
    column_start: u64,
    column_end: u64,
    forbidden: &'static str,
    forbidden_path: &'static str,
    symbol: &'static str,
    fragments: &'static [&'static str],
}

#[test]
fn external_diagnostic_matcher_requires_code_primary_span_and_forbidden_surface() {
    let diagnostic = serde_json::json!({
        "reason": "compiler-message",
        "message": {
            "code": { "code": "E0432" },
            "message": "unresolved import `pi_coding_agent::api::Operation`",
            "spans": [{
                "file_name": "src/main.rs",
                "line_start": 1,
                "line_end": 1,
                "column_start": 30,
                "column_end": 39,
                "is_primary": true,
                "label": "no `Operation` in `api`"
            }],
            "children": [],
            "rendered": "error[E0432]: unresolved import `pi_coding_agent::api::Operation`"
        }
    });
    let expected = ExpectedDiagnostic {
        code: "E0432",
        line: 1,
        column_start: 30,
        column_end: 39,
        forbidden: "Operation",
        forbidden_path: "pi_coding_agent::api",
        symbol: "Operation",
        fragments: &["unresolved import", "pi_coding_agent::api::Operation"],
    };

    assert!(diagnostic_matches(&diagnostic, &expected).is_ok());
    let wrong_code = ExpectedDiagnostic {
        code: "E0603",
        ..expected
    };
    assert!(diagnostic_matches(&diagnostic, &wrong_code).is_err());
}

const FAIL_FIXTURES: [CompileFixture; 12] = [
    CompileFixture {
        category: "operation-dispatch",
        access_path: "api",
        source: "operation_dispatch_api.rs",
        expected: unresolved(28, 37, "Operation", "pi_coding_agent::api"),
    },
    CompileFixture {
        category: "operation-dispatch",
        access_path: "root",
        source: "operation_dispatch_root.rs",
        expected: unresolved(23, 32, "Operation", "pi_coding_agent"),
    },
    CompileFixture {
        category: "operation-dispatch",
        access_path: "doc-hidden",
        source: "operation_dispatch_hidden.rs",
        expected: private(22, 36, "operation"),
    },
    CompileFixture {
        category: "services",
        access_path: "api",
        source: "services_api.rs",
        expected: unresolved(28, 40, "EventService", "pi_coding_agent::api"),
    },
    CompileFixture {
        category: "services",
        access_path: "root",
        source: "services_root.rs",
        expected: unresolved(23, 35, "EventService", "pi_coding_agent"),
    },
    CompileFixture {
        category: "services",
        access_path: "doc-hidden",
        source: "services_hidden.rs",
        expected: private(22, 36, "event_service"),
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "api",
        source: "plugins_api.rs",
        expected: unresolved(28, 45, "PluginLoadOptions", "pi_coding_agent::api"),
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "root",
        source: "plugins_root.rs",
        expected: unresolved(23, 40, "PluginLoadOptions", "pi_coding_agent"),
    },
    CompileFixture {
        category: "plugin-options-registries",
        access_path: "doc-hidden",
        source: "plugins_hidden.rs",
        expected: private(23, 37, "plugin_load_flow"),
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "api",
        source: "flow_api.rs",
        expected: unresolved(28, 32, "Flow", "pi_coding_agent::api"),
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "root",
        source: "flow_root.rs",
        expected: unresolved(23, 27, "Flow", "pi_coding_agent"),
    },
    CompileFixture {
        category: "flow-contracts",
        access_path: "doc-hidden",
        source: "flow_hidden.rs",
        expected: private(22, 36, "export_flow"),
    },
];

const fn unresolved(
    column_start: u64,
    column_end: u64,
    forbidden: &'static str,
    forbidden_path: &'static str,
) -> ExpectedDiagnostic {
    ExpectedDiagnostic {
        code: "E0432",
        line: 1,
        column_start,
        column_end,
        forbidden,
        forbidden_path,
        symbol: forbidden,
        fragments: &["unresolved import"],
    }
}

const fn private(column_start: u64, column_end: u64, symbol: &'static str) -> ExpectedDiagnostic {
    ExpectedDiagnostic {
        code: "E0603",
        line: 1,
        column_start,
        column_end,
        forbidden: "coding_session",
        forbidden_path: "pi_coding_agent",
        symbol,
        fragments: &["is private"],
    }
}

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
        let fixture_path = fixture_root.join("fail").join(fixture.source);
        validate_declared_source_span(&fixture_path, &fixture.expected);
        let output = compile_fixture(consumer.path(), &fixture_path);
        let diagnostics = command_diagnostics(&output);
        assert!(
            !output.status.success(),
            "{} must remain inaccessible through the {} path",
            fixture.category,
            fixture.access_path
        );
        let errors = compiler_error_diagnostics(&output);
        assert!(
            !errors.is_empty(),
            "Cargo emitted no rustc error diagnostic:\n{diagnostics}"
        );
        diagnostic_matches(&errors[0], &fixture.expected).unwrap_or_else(|mismatch| {
            panic!(
                "{} through {} failed for an unrelated first compiler error: {mismatch}\n{}",
                fixture.category, fixture.access_path, diagnostics
            )
        });
    }
}

fn compile_fixture(consumer_root: &Path, fixture: &Path) -> Output {
    fs::copy(fixture, consumer_root.join("src/main.rs")).unwrap_or_else(|error| {
        panic!("copy fixture {}: {error}", fixture.display());
    });
    Command::new(env!("CARGO"))
        .args(["check", "--offline", "--quiet", "--message-format=json"])
        .current_dir(consumer_root)
        .env("CARGO_TARGET_DIR", consumer_root.join("target"))
        .output()
        .unwrap_or_else(|error| panic!("run Cargo for fixture {}: {error}", fixture.display()))
}

fn validate_declared_source_span(fixture: &Path, expected: &ExpectedDiagnostic) {
    let source = fs::read_to_string(fixture)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", fixture.display()));
    let line = source
        .lines()
        .nth(expected.line.saturating_sub(1) as usize)
        .unwrap_or_else(|| {
            panic!(
                "fixture {} omitted line {}",
                fixture.display(),
                expected.line
            )
        });
    let start = expected.column_start.saturating_sub(1) as usize;
    let end = expected.column_end.saturating_sub(1) as usize;
    assert_eq!(
        line.get(start..end),
        Some(expected.forbidden),
        "declared forbidden span drifted in {}",
        fixture.display()
    );
    assert!(
        line.contains(expected.forbidden_path),
        "declared forbidden path `{}` is absent from {}",
        expected.forbidden_path,
        fixture.display()
    );
}

fn compiler_error_diagnostics(output: &Output) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter(|message| {
            message.get("reason").and_then(|value| value.as_str()) == Some("compiler-message")
                && message
                    .pointer("/message/level")
                    .and_then(|value| value.as_str())
                    == Some("error")
        })
        .collect()
}

fn diagnostic_matches(
    diagnostic: &serde_json::Value,
    expected: &ExpectedDiagnostic,
) -> Result<(), String> {
    let actual_code = diagnostic
        .pointer("/message/code/code")
        .and_then(|value| value.as_str());
    if actual_code != Some(expected.code) {
        return Err(format!(
            "expected {}, found {:?}",
            expected.code, actual_code
        ));
    }

    let primary = diagnostic
        .pointer("/message/spans")
        .and_then(|value| value.as_array())
        .and_then(|spans| {
            spans.iter().find(|span| {
                span.get("is_primary").and_then(|value| value.as_bool()) == Some(true)
                    && span
                        .get("file_name")
                        .and_then(|value| value.as_str())
                        .is_some_and(|file| file == "src/main.rs" || file.ends_with("/src/main.rs"))
                    && span.get("line_start").and_then(|value| value.as_u64())
                        == Some(expected.line)
                    && span.get("column_start").and_then(|value| value.as_u64())
                        == Some(expected.column_start)
                    && span.get("column_end").and_then(|value| value.as_u64())
                        == Some(expected.column_end)
            })
        })
        .ok_or_else(|| {
            format!(
                "missing primary src/main.rs span {}:{}-{}",
                expected.line, expected.column_start, expected.column_end
            )
        })?;

    let mut text = diagnostic
        .pointer("/message/message")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_owned();
    if let Some(rendered) = diagnostic
        .pointer("/message/rendered")
        .and_then(|value| value.as_str())
    {
        text.push_str(rendered);
    }
    if let Some(label) = primary.get("label").and_then(|value| value.as_str()) {
        text.push_str(label);
    }
    if let Some(children) = diagnostic
        .pointer("/message/children")
        .and_then(|value| value.as_array())
    {
        for child in children {
            if let Some(message) = child.get("message").and_then(|value| value.as_str()) {
                text.push_str(message);
            }
        }
    }
    for fragment in std::iter::once(&expected.symbol)
        .chain(std::iter::once(&expected.forbidden))
        .chain(expected.fragments.iter())
    {
        if !text.contains(fragment) {
            return Err(format!("diagnostic omitted fragment `{fragment}`"));
        }
    }
    Ok(())
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
        "CompactCancellationHandle",
        "CompactCancellationRejection",
        "CancellationToken",
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
        "compact_cancellation",
        "cancel_operation",
        "operation_generation",
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
