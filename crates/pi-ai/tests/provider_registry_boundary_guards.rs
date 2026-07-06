use std::fs;
use std::path::{Path, PathBuf};

const ALLOWED_GLOBAL_PROVIDER_MUTATION_FILES: &[&str] = &[
    "crates/pi-ai/tests/support/mod.rs",
    "crates/pi-ai/tests/support_guards.rs",
    "crates/pi-ai/tests/provider_registry_boundary_guards.rs",
];

#[test]
fn pi_ai_provider_tests_keep_global_registry_mutation_behind_guards() {
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
        "global provider registry mutation must stay behind test guards; use ProviderRegistry/register_builtins_into for scoped built-in coverage:\n{}",
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
fn bedrock_bearer_env_default_stays_behind_provider_auth_resolver() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let mut violations = Vec::new();

    collect_provider_env_patterns(
        repo_root,
        &crate_root.join("src/providers/bedrock/mod.rs"),
        &["AWS_BEARER_TOKEN_BEDROCK"],
        &mut violations,
    );
    collect_provider_env_patterns(
        repo_root,
        &crate_root.join("src/providers/bedrock/auth.rs"),
        &["AWS_BEARER_TOKEN_BEDROCK"],
        &mut violations,
    );

    assert!(
        violations.is_empty(),
        "Bedrock bearer-token env defaults must be resolved by ProviderAuthResolver and injected through StreamOptions, not read directly inside the provider:\n{}",
        violations.join("\n")
    );
}

#[test]
fn azure_openai_runtime_env_defaults_stay_behind_provider_auth_resolver() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_root
        .parent()
        .and_then(Path::parent)
        .expect("crate should live under crates/pi-ai");
    let provider_file = crate_root.join("src/providers/azure_openai_responses.rs");
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
            collect_provider_env_key_reads(repo_root, &entry.path(), violations);
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
        if line.contains("env_keys::env_api_key") || line.contains("env_api_key(") {
            violations.push(format!("{}:{}: {}", relative, line_index + 1, line.trim()));
        }
    }
}

#[test]
fn pi_ai_api_facade_keeps_global_provider_runtime_helpers_out() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs")).expect("read pi-ai lib.rs");
    let api_facade = lib_source
        .split_once("pub mod api {")
        .map(|(_, api_facade)| api_facade)
        .expect("pi_ai::api facade should exist");

    for global_helper in ["register,", "stream_model,"] {
        assert!(
            !api_facade.contains(global_helper),
            "pi_ai::api should expose scoped AiClient/ProviderRegistry APIs, not global provider runtime helper `{global_helper}`"
        );
    }
}

#[test]
fn registry_global_runtime_helpers_are_deprecated_compatibility_functions() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let registry_source =
        fs::read_to_string(crate_root.join("src/registry.rs")).expect("read pi-ai registry.rs");

    for helper in [
        "pub fn register(api: &str",
        "pub fn unregister(api: &str",
        "pub fn lookup(api: &str",
        "pub fn stream_model(model: &Model",
    ] {
        let index = registry_source
            .find(helper)
            .unwrap_or_else(|| panic!("global registry helper should exist: {helper}"));
        let preceding = &registry_source[index.saturating_sub(360)..index];
        assert!(
            preceding.contains("#[deprecated(")
                && preceding.contains("AiClient")
                && preceding.contains("ProviderRegistry"),
            "global registry helper `{helper}` should be deprecated with scoped AiClient/ProviderRegistry replacement guidance"
        );
    }
}

#[test]
fn pi_ai_root_global_runtime_helpers_are_deprecated_compatibility_exports() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let lib_source = fs::read_to_string(crate_root.join("src/lib.rs")).expect("read pi-ai lib.rs");

    for helper in ["register", "stream_model"] {
        let export = format!("pub use registry::{helper};");
        let index = lib_source.find(&export).unwrap_or_else(|| {
            panic!("root-level global runtime helper should be exported as `{export}`")
        });
        let preceding = &lib_source[index.saturating_sub(260)..index];
        assert!(
            preceding.contains("#[deprecated(")
                && preceding.contains("AiClient")
                && preceding.contains("ProviderRegistry"),
            "root-level pi_ai::{helper} should be a deprecated compatibility export with scoped runtime replacement guidance"
        );
    }
}

#[test]
fn global_builtin_registration_helper_is_deprecated_with_scoped_replacements() {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let providers_source = fs::read_to_string(crate_root.join("src/providers/mod.rs"))
        .expect("read pi-ai providers module");

    let index = providers_source
        .find("pub fn register_builtins()")
        .expect("global built-in registration helper should remain for compatibility");
    let preceding = &providers_source[index.saturating_sub(320)..index];
    assert!(
        preceding.contains("#[deprecated(")
            && preceding.contains("register_builtins_into")
            && preceding.contains("AiClient::register_builtins"),
        "pi_ai::providers::register_builtins should be deprecated with scoped ProviderRegistry/AiClient replacement guidance"
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
