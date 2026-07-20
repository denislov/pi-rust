use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED_GLOBAL_PROVIDER_MUTATION_FILES: &[&str] =
    &["crates/pi-ai/tests/provider_registry_boundary_guards.rs"];

#[test]
fn pi_ai_provider_tests_do_not_mutate_global_registry() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let tests_root = crate_root.join("tests");
    let mut violations = Vec::new();

    collect_global_provider_mutations(repo_root, &tests_root, &mut violations);

    assert!(
        violations.is_empty(),
        "pi-ai tests must use scoped ProviderRegistry/AiClient registration:\n{}",
        violations.join("\n")
    );
}

fn collect_global_provider_mutations(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read tests directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read test entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_global_provider_mutations(repo_root, &entry.path(), violations);
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    if ALLOWED_GLOBAL_PROVIDER_MUTATION_FILES.contains(&relative.as_str()) {
        return;
    }

    let content = fs::read_to_string(path).expect("read test file");
    for (line_index, line) in content.lines().enumerate() {
        if is_global_provider_mutation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

fn is_global_provider_mutation(line: &str) -> bool {
    line.contains("pi_ai::providers::register_builtins()")
        || line.contains("providers::register_builtins()")
        || line.contains("registry::register(")
        || line.contains("registry::unregister(")
        || line.contains("pi_ai::registry::register(")
        || line.contains("pi_ai::registry::unregister(")
}

#[test]
fn pi_ai_providers_do_not_read_env_api_keys_directly() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let providers_root = crate_root.join("src/providers");
    let mut violations = Vec::new();

    collect_provider_env_key_reads(repo_root, &providers_root, &mut violations);

    assert!(
        violations.is_empty(),
        "provider implementations must receive API keys from StreamOptions populated by ProviderAuthResolver instead of reading env_api_key directly:\n{}",
        violations.join("\n")
    );
}

#[test]
fn pi_ai_provider_modules_do_not_read_process_env_directly() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let providers_root = crate_root.join("src/providers");
    let mut violations = Vec::new();

    collect_provider_process_env_reads(repo_root, &providers_root, &mut violations);

    assert!(
        violations.is_empty(),
        "provider modules must receive env-derived auth/runtime material through ProviderAuthResolver, StreamOptions, or shared env utilities instead of reading process env directly:\n{}",
        violations.join("\n")
    );
}

#[test]
fn retired_bedrock_and_aws_surface_is_absent() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(!crate_root.join("src/providers/bedrock").exists());

    let implementation_files = [
        "Cargo.toml",
        "src/providers/mod.rs",
        "src/protocol/request.rs",
        "src/registry/auth.rs",
        "src/registry/env.rs",
        "src/transport/http.rs",
        "src/model/generated.json",
    ];
    for relative in implementation_files {
        let content = fs::read_to_string(crate_root.join(relative)).expect("read implementation");
        for retired in [
            "amazon-bedrock",
            "bedrock-converse-stream",
            "aws-config",
            "aws-credential-types",
            "aws-sigv4",
            "AWS_ACCESS_KEY_ID",
            "AWS_PROFILE",
            "SigV4",
        ] {
            assert!(
                !content.contains(retired),
                "retired Bedrock/AWS surface `{retired}` remains in {relative}"
            );
        }
    }

    let generator = fs::read_to_string(crate_root.join("tools/generate_models.cjs"))
        .expect("read catalog generator");
    assert!(generator.contains("RETIRED_PROVIDERS"));
    assert!(generator.contains("RETIRED_APIS"));
    assert!(generator.contains("amazon-bedrock"));
    assert!(generator.contains("bedrock-converse-stream"));
}

#[test]
fn azure_openai_runtime_env_defaults_stay_behind_provider_auth_resolver() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let provider_file = crate_root.join("src/providers/azure_openai_responses/mod.rs");
    let mut violations = Vec::new();

    collect_provider_env_patterns(
        repo_root,
        &provider_file,
        &[
            "AZURE_OPENAI_API_VERSION",
            "AZURE_OPENAI_BASE_URL",
            "AZURE_OPENAI_RESOURCE_NAME",
            "AZURE_OPENAI_DEPLOYMENT_NAME_MAP",
        ],
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "Azure OpenAI runtime/auth env defaults must be resolved by ProviderAuthResolver and injected through StreamOptions, not read directly inside the provider:\n{}",
        violations.join("\n")
    );
}

fn collect_provider_env_patterns(
    repo_root: &Path,
    path: &Path,
    patterns: &[&str],
    violations: &mut Vec<String>,
) {
    let content = fs::read_to_string(path).expect("read provider file");
    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    for (line_index, line) in content.lines().enumerate() {
        if patterns.iter().any(|pattern| line.contains(pattern)) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

fn collect_provider_env_key_reads(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    collect_provider_source_violations(repo_root, path, violations, |line| {
        line.contains("env_keys::env_api_key") || line.contains("env_api_key(")
    });
}

fn collect_provider_process_env_reads(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    collect_provider_source_violations(repo_root, path, violations, |line| {
        line.contains("std::env::var") || line.contains("std::env::var_os")
    });
}

fn collect_provider_source_violations(
    repo_root: &Path,
    path: &Path,
    violations: &mut Vec<String>,
    is_violation: impl Copy + Fn(&str) -> bool,
) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read providers directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read provider entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_provider_source_violations(repo_root, &entry.path(), violations, is_violation);
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    let content = fs::read_to_string(path).expect("read provider file");
    for (line_index, line) in content.lines().enumerate() {
        if is_violation(line) {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

#[test]
fn pi_ai_api_facade_keeps_global_provider_runtime_helpers_out() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let api_facade = fs::read_to_string(crate_root.join("src/api.rs")).expect("read pi-ai api.rs");

    for global_helper in ["register,", "stream_model,"] {
        assert!(
            !api_facade.contains(global_helper),
            "pi_ai::api should expose scoped AiClient/ProviderRegistry APIs, not global provider runtime helper `{global_helper}`"
        );
    }
}

#[test]
fn registry_global_runtime_helpers_are_removed() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let registry_source = fs::read_to_string(crate_root.join("src/registry/mod.rs"))
        .expect("read pi-ai registry owner");

    for helper in [
        "pub fn register(api: &str",
        "pub fn unregister(api: &str",
        "pub fn lookup(api: &str",
        "pub fn stream_model(model: &Model",
    ] {
        assert!(
            !registry_source.contains(helper),
            "global registry helper `{helper}` must be removed"
        );
    }
}

#[test]
fn pi_ai_root_global_runtime_helpers_are_removed() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs")).expect("read pi-ai lib.rs");

    for helper in ["register", "stream_model"] {
        assert!(
            !lib_source.contains(&format!("pub use registry::{helper};")),
            "root-level pi_ai::{helper} must be removed"
        );
    }
}

#[test]
fn global_builtin_registration_helper_is_removed() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let providers_source = fs::read_to_string(crate_root.join("src/providers/mod.rs"))
        .expect("read pi-ai providers module");

    assert!(
        !providers_source.contains("pub fn register_builtins()"),
        "pi_ai::providers::register_builtins must be removed"
    );
}

const ALLOWED_GLOBAL_STREAM_TEST_FILES: &[&str] = &[
    "crates/pi-ai/tests/public_api.rs",
    "crates/pi-ai/tests/provider_registry_boundary_guards.rs",
];

#[test]
fn pi_ai_provider_tests_use_scoped_registry_for_streaming() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let tests_root = crate_root.join("tests");
    let mut violations = Vec::new();

    collect_global_stream_calls(repo_root, &tests_root, &mut violations);

    assert!(
        violations.is_empty(),
        "pi-ai provider tests should stream through scoped ProviderRegistry/AiClient instances instead of global runtime helpers:\n{}",
        violations.join("\n")
    );
}

fn collect_global_stream_calls(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read tests directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read test entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_global_stream_calls(repo_root, &entry.path(), violations);
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    if ALLOWED_GLOBAL_STREAM_TEST_FILES.contains(&relative.as_str()) {
        return;
    }

    let content = fs::read_to_string(path).expect("read test file");
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("registry::stream_model(")
            || line.contains("pi_ai::registry::stream_model(")
            || line.contains("pi_ai::stream_model(")
        {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

#[test]
fn pi_ai_examples_use_scoped_provider_runtime() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let examples_root = crate_root.join("examples");
    let mut violations = Vec::new();

    collect_global_runtime_calls(repo_root, &examples_root, &mut violations);

    assert!(
        violations.is_empty(),
        "pi-ai examples should demonstrate scoped ProviderRegistry/AiClient usage instead of global provider runtime helpers:\n{}",
        violations.join("\n")
    );
}

fn collect_global_runtime_calls(repo_root: &Path, path: &Path, violations: &mut Vec<String>) {
    let Ok(metadata) = fs::metadata(path) else {
        return;
    };

    if metadata.is_dir() {
        let mut entries = fs::read_dir(path)
            .expect("read examples directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("read example entries");
        entries.sort_by_key(|entry| entry.path());
        for entry in entries {
            collect_global_runtime_calls(repo_root, &entry.path(), violations);
        }
        return;
    }

    if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
        return;
    }

    let relative = path
        .strip_prefix(repo_root)
        .expect("scanned file should be under repo root")
        .to_string_lossy()
        .replace('\\', "/");
    let content = fs::read_to_string(path).expect("read example file");
    for (line_index, line) in content.lines().enumerate() {
        if line.contains("registry::register(")
            || line.contains("registry::unregister(")
            || line.contains("registry::stream_model(")
            || line.contains("pi_ai::registry::register(")
            || line.contains("pi_ai::registry::unregister(")
            || line.contains("pi_ai::registry::stream_model(")
            || line.contains("pi_ai::stream_model(")
        {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}
