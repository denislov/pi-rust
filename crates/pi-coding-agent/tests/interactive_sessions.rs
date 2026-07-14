mod support;

use pi_agent_core::transcript::{
    SessionEntry, SessionHeader, StoredAgentMessage, StoredUsage, create_timestamp,
};
use pi_ai::api::StopReason;
use pi_ai::providers::faux::{FauxProvider, FauxResponse};
use pi_coding_agent::CliArgs;
use pi_coding_agent::interactive::test_harness::{
    run_scripted_interactive_with_args_and_session_dir, run_scripted_interactive_with_session_dir,
    run_scripted_interactive_with_session_dir_and_waits,
};

fn text_response(text: &str) -> FauxResponse {
    FauxResponse {
        text_deltas: vec![text.to_string()],
        thinking_deltas: vec![],
        tool_calls: vec![],
    }
}

#[tokio::test]
async fn interactive_mode_appends_to_session() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("saved")]);
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "persist me\r")
        .await
        .unwrap();
    assert!(result.session_file.exists());
    let contents = std::fs::read_to_string(result.session_file).unwrap();
    assert!(contents.contains("persist me"));
    assert!(contents.contains("saved"));
}

#[tokio::test]
async fn interactive_footer_updates_to_created_session_id_after_prompt() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("saved")]);
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "persist me\r")
        .await
        .unwrap();
    let files = jsonl_files(temp.path());
    assert_eq!(files.len(), 1);
    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 1);
    let manifest = read_session_manifest(&sessions[0]);
    let session_id = manifest["session_id"]
        .as_str()
        .expect("session id should be present in manifest");
    let visible_session_prefix = &session_id[..13];

    let final_frame = result.rendered_lines.join("\n");
    assert!(
        final_frame.contains(&format!("• {visible_session_prefix}")),
        "{final_frame}"
    );
    assert!(!final_frame.contains("• session"), "{final_frame}");
}

#[tokio::test]
async fn interactive_session_command_reports_created_rust_native_session() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("saved")]);

    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![("persist me\r", "saved"), ("/session\r", "saved")],
    )
    .await
    .unwrap();

    let frame = result.rendered_lines.join("\n");
    assert!(frame.contains("Session Info"), "{frame}");
    assert!(frame.contains("Storage: rust-native"), "{frame}");
    assert!(frame.contains("Entries:"), "{frame}");
    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 1);
    let manifest = read_session_manifest(&sessions[0]);
    let leaf_id = manifest["active_leaf_id"]
        .as_str()
        .expect("active leaf should be present after prompt");
    assert!(leaf_id.starts_with("leaf_"));
    assert!(
        frame.contains(&format!("Active leaf: {leaf_id}")),
        "{frame}"
    );
}

#[tokio::test]
async fn interactive_tree_command_opens_created_rust_native_session() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("tree answer")]);

    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![("tree prompt\r", "tree answer"), ("/tree\r", "tree answer")],
    )
    .await
    .unwrap();

    let frame = result.rendered_lines.join("\n");
    assert!(frame.contains("Session Tree"), "{frame}");
    assert!(frame.contains("user: tree prompt"), "{frame}");
    assert!(!frame.contains("assistant: tree answer"), "{frame}");
}

#[tokio::test]
async fn interactive_tree_navigation_forks_to_selected_rust_native_leaf() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("first answer", StopReason::Stop),
        FauxProvider::text_call("second answer", StopReason::Stop),
        FauxProvider::text_call("fork continuation", StopReason::Stop),
    ]);

    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("first prompt\r", "first answer"),
            ("second prompt\r", "second answer"),
            ("/tree\r\x1b[A\r", "session.forked"),
            ("continue selected branch\r", "fork continuation"),
        ],
    )
    .await
    .unwrap();

    let frame = result.rendered_lines.join("\n");
    assert!(frame.contains("Navigated to selected point"), "{frame}");

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 2);
    let event_logs = sessions
        .iter()
        .map(|session| std::fs::read_to_string(session.join("events.jsonl")).unwrap())
        .collect::<Vec<_>>();
    let forked = event_logs
        .iter()
        .find(|events| events.contains(r#""kind":"session.forked""#))
        .expect("forked session should record provenance");
    assert!(forked.contains("first prompt"), "{forked}");
    assert!(!forked.contains("second prompt"), "{forked}");
    assert!(forked.contains("continue selected branch"), "{forked}");
}

#[tokio::test]
async fn interactive_tree_navigation_summarizes_abandoned_leaf_before_forking() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("first answer", StopReason::Stop),
        FauxProvider::text_call("second answer", StopReason::Stop),
        FauxProvider::text_call("model branch summary", StopReason::Stop),
        FauxProvider::text_call("summary fork continuation", StopReason::Stop),
    ]);

    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("first prompt\r", "first answer"),
            ("second prompt\r", "second answer"),
            ("/tree\r\x1b[A\r", "model branch summary"),
            ("continue summarized branch\r", "summary fork continuation"),
        ],
    )
    .await
    .unwrap();

    let frame = result.rendered_lines.join("\n");
    assert!(frame.contains("Navigated to selected point"), "{frame}");
    assert!(frame.contains("model branch summary"), "{frame}");

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 2);
    let event_logs = sessions
        .iter()
        .map(|session| std::fs::read_to_string(session.join("events.jsonl")).unwrap())
        .collect::<Vec<_>>();
    let forked = event_logs
        .iter()
        .find(|events| events.contains(r#""kind":"session.forked""#))
        .expect("forked session should record provenance");
    assert!(
        forked.contains(r#""kind":"branch.summary.created""#),
        "{forked}"
    );
    assert!(forked.contains("model branch summary"), "{forked}");
    assert!(forked.contains("continue summarized branch"), "{forked}");
}

#[tokio::test]
async fn scripted_interactive_branch_summary_preserves_visible_and_persisted_behavior() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("branch prompt answer")]);

    // Step 1: Create a session with a prompt to establish an active Rust-native leaf.
    run_scripted_interactive_with_session_dir(provider, temp.path(), "branch prompt\r")
        .await
        .unwrap();

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(
        sessions.len(),
        1,
        "exactly one session should exist after the initial prompt"
    );
    let manifest = read_session_manifest(&sessions[0]);
    let session_id = manifest["session_id"]
        .as_str()
        .expect("session id should be present in manifest")
        .to_owned();
    let leaf_id = manifest["active_leaf_id"]
        .as_str()
        .expect("active leaf should be present after prompt")
        .to_owned();

    // Step 2: Resume the session and run a direct /branch-summary command.
    // The direct command uses AlwaysCreate semantics (reuse_existing: false)
    // and must NOT trigger navigation hydration or session replacement.
    let args = CliArgs {
        resume: true,
        ..CliArgs::default()
    };
    let provider = FauxProvider::new(Vec::new());
    let branch_summary_command = format!("/branch-summary {leaf_id} {leaf_id}\r\x03");
    let result = run_scripted_interactive_with_args_and_session_dir(
        provider,
        args,
        temp.path(),
        &branch_summary_command,
    )
    .await
    .unwrap();

    let frame = result.rendered_lines.join("\n");

    // Visible: the branch-summary command ran and its projection was emitted.
    assert!(
        frame.contains("Summarizing branch..."),
        "direct branch-summary should be visibly projected: {frame}"
    );

    // Direct-command semantics: no navigation hydration notice.
    assert!(
        !frame.contains("Navigated to selected point"),
        "direct branch-summary must not adopt navigation hydration semantics: {frame}"
    );

    // No session replacement: the same single session persists.
    let sessions_after = rust_session_dirs(temp.path());
    assert_eq!(
        sessions_after.len(),
        1,
        "direct branch-summary must not create a new session: {frame}"
    );
    let manifest_after = read_session_manifest(&sessions_after[0]);
    assert_eq!(
        manifest_after["session_id"].as_str(),
        Some(session_id.as_str()),
        "direct branch-summary must not replace the session identity: {frame}"
    );

    // Durable: the branch-summary operation appended session events without a fork.
    let events = std::fs::read_to_string(sessions_after[0].join("events.jsonl")).unwrap();
    assert!(
        !events.contains(r#""kind":"session.forked""#),
        "direct branch-summary must not record a session fork: {events}"
    );
    // The original prompt content must still be durable.
    assert!(
        events.contains("branch prompt"),
        "direct branch-summary must not destroy prior durable facts: {events}"
    );
}

#[tokio::test]
async fn scripted_interactive_default_profile_selection_persists_and_refreshes_projection() {
    let _env = support::EnvGuard::with_pi_rust_dir(tempfile::tempdir().unwrap().path());
    let temp = tempfile::tempdir().unwrap();

    // Set up a project-level agent profile so the profile menu has a selectable entry.
    std::fs::create_dir_all(temp.path().join(".pi-rust/agents")).unwrap();
    std::fs::write(
        temp.path().join(".pi-rust/agents/coder.toml"),
        r#"
schema_version = 1
id = "coder"
display_name = "Coder"
"#,
    )
    .unwrap();

    // Step 1: Create a persistent session with an initial prompt, then select "coder".
    //   "initial prompt\r"       - submit a prompt to open a session
    //   "/agent\r\x1b[B\rcoder\r" - open the agent menu, down to "Use", filter and confirm
    let provider = FauxProvider::new(vec![text_response("initial response")]);
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("initial prompt\r", "initial response"),
            ("/agent\r\x1b[B\rcoder\r", ""),
        ],
    )
    .await
    .unwrap();

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 1, "exactly one session should exist");

    let frame = result.rendered_lines.join("\n");
    assert!(
        frame.contains("Default agent profile: coder"),
        "profile selection should be visibly projected: {frame}"
    );

    // The manifest must now persist the new default profile.
    let manifest = read_session_manifest(&sessions[0]);
    assert_eq!(
        manifest["default_agent_profile_id"].as_str(),
        Some("coder"),
        "manifest should persist the canonical default profile mutation: {frame}"
    );

    // Step 2: Reopen the session and verify the default profile is preserved.
    let args = CliArgs {
        resume: true,
        ..CliArgs::default()
    };
    let provider = FauxProvider::new(vec![text_response("verification response")]);
    run_scripted_interactive_with_args_and_session_dir(provider, args, temp.path(), "verify\r")
        .await
        .unwrap();

    let manifest = read_session_manifest(&sessions[0]);
    assert_eq!(
        manifest["default_agent_profile_id"].as_str(),
        Some("coder"),
        "manifest should preserve the default profile after reopen"
    );
}

#[tokio::test]
async fn interactive_mode_continues_same_session_across_prompts() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::with_call_queue(vec![
        FauxProvider::text_call("first saved", StopReason::Stop),
        FauxProvider::text_call("second saved", StopReason::Stop),
    ]);

    run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("first prompt\r", "first saved"),
            ("second prompt\r", "second saved"),
        ],
    )
    .await
    .unwrap();

    let files = jsonl_files(temp.path());
    assert_eq!(files.len(), 1);
    let contents = std::fs::read_to_string(&files[0]).unwrap();
    assert!(contents.contains("first prompt"));
    assert!(contents.contains("first saved"));
    assert!(contents.contains("second prompt"));
    assert!(contents.contains("second saved"));
}

#[tokio::test]
async fn interactive_reload_reports_project_plugin_manifest_diagnostics() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/project-lua");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "project-lua"
name = "Project Lua"
version = "0.1.0"
runtime = "lua"
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "/reload\r")
        .await
        .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("project-lua"), "{frame}");
    assert!(frame.contains("Lua plugin entry is required"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_command_runs_loaded_lua_plugin_command() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-command");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-command"
name = "Lua Command"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.say_hello",
    description = "greets from lua command",
    run = function(input)
      return { content = "hello " .. input.name }
    end
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            (
                "/plugin-command lua.say_hello {\"name\":\"tui\"}\r",
                "plugin.load.completed",
            ),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.say_hello"), "{frame}");
    assert!(frame.contains("hello tui"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_command_can_read_lua_host_api_metadata() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-host-api");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-host-api"
name = "Lua Host API"
version = "2.3.4"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  local api = host:api_version()
  local plugin = host:plugin()
  host:command({
    id = "lua.host_info",
    description = "reports host API metadata",
    run = function(input)
      return {
        content = "api " .. api .. " plugin " .. plugin.id .. " " .. plugin.name .. " " .. plugin.version .. " " .. plugin.source
      }
    end
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            (
                "/plugin-command lua.host_info {}\r",
                "plugin.load.completed",
            ),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.host_info"), "{frame}");
    assert!(
        frame.contains("api 1 plugin lua-host-api Lua Host API 2.3.4 lua"),
        "{frame}"
    );
    assert!(
        !frame.contains("Lua plugin register(host) failed"),
        "{frame}"
    );
}

#[tokio::test]
async fn interactive_plugin_command_slash_alias_runs_loaded_lua_plugin_command() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-command");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-command"
name = "Lua Command"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.say_hello",
    description = "greets from lua command",
    run = function(input)
      return { content = "hello " .. input.name }
    end
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            (
                "/lua.say_hello {\"name\":\"alias\"}\r",
                "plugin.load.completed",
            ),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.say_hello"), "{frame}");
    assert!(frame.contains("hello alias"), "{frame}");
    assert!(!frame.contains("unknown command"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_keybinding_opens_loaded_lua_dialog() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog"
name = "Lua Dialog"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        default = "pi"
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(
        frame.contains("Plugin UI dialog dialog.open_panel"),
        "{frame}"
    );
    assert!(frame.contains("Lua panel"), "{frame}");
    assert!(frame.contains("Panel registered by Lua"), "{frame}");
    assert!(frame.contains("Name: Target name"), "{frame}");
    assert!(
        frame.contains("/plugin-command lua.submit_panel {\"name\":\"pi\"}"),
        "{frame}"
    );
}

#[tokio::test]
async fn interactive_plugin_dialog_form_submit_uses_edited_text_field() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-form");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-form"
name = "Lua Dialog Form"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "submitted " .. input.name }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("codex\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("submitted codex"), "{frame}");
    assert!(
        !frame.contains("Plugin dialog field Name is required"),
        "{frame}"
    );
    assert!(!frame.contains("Invalid plugin command args"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_tab_edits_second_field() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-form-tab");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-form-tab"
name = "Lua Dialog Form Tab"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "submitted " .. input.name .. " " .. input.note }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        required = true
      },
      {
        id = "note",
        label = "Note",
        description = "Submission note",
        type = "text",
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("codex\trocks\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("submitted codex rocks"), "{frame}");
    assert!(
        !frame.contains("Plugin dialog field Note is required"),
        "{frame}"
    );
    assert!(!frame.contains("Invalid plugin command args"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_integer_filters_invalid_chars_and_submits_number() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp
        .path()
        .join(".pi-rust/plugins/lua-dialog-integer-filter");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-integer-filter"
name = "Lua Dialog Integer Filter"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "count " .. tostring(input.count + 1) }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "count",
        label = "Count",
        description = "Count value",
        type = "integer",
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("12x3\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("count 124"), "{frame}");
    assert!(!frame.contains("must be integer"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_select_space_cycles_and_submits_option() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-select");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-select"
name = "Lua Dialog Select"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "mode " .. input.mode }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "mode",
        label = "Mode",
        description = "Execution mode",
        type = "select",
        default = "fast",
        options = { "fast", "safe" },
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            (" \r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("mode safe"), "{frame}");
    assert!(!frame.contains("must be one of"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_select_rejects_value_outside_options() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp
        .path()
        .join(".pi-rust/plugins/lua-dialog-select-invalid");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-select-invalid"
name = "Lua Dialog Select Invalid"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "mode " .. input.mode }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "mode",
        label = "Mode",
        description = "Execution mode",
        type = "select",
        default = "turbo",
        options = { "fast", "safe" },
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(
        frame.contains("Plugin dialog field Mode must be one of: fast, safe"),
        "{frame}"
    );
    assert!(
        frame.contains("  error: Plugin dialog field Mode must be one of: fast, safe"),
        "{frame}"
    );
    assert!(
        !frame.contains("Plugin command lua.submit_panel"),
        "{frame}"
    );
    assert!(!frame.contains("mode turbo"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_validation_focuses_required_error_field() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-focus-error");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-focus-error"
name = "Lua Dialog Focus Error"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "should not run" }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        required = true
      },
      {
        id = "note",
        label = "Note",
        description = "Submission note",
        type = "text",
        required = false
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\tnote\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(
        frame.contains("Plugin dialog field Name is required"),
        "{frame}"
    );
    assert!(frame.contains("> Name * [text]:"), "{frame}");
    assert!(
        frame.contains("  error: Plugin dialog field Name is required"),
        "{frame}"
    );
    assert!(frame.contains("  Note [text]: note"), "{frame}");
    assert!(
        !frame.contains("Plugin command lua.submit_panel"),
        "{frame}"
    );
    assert!(!frame.contains("should not run"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_form_boolean_space_toggles_and_submits_bool() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-bool-toggle");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-bool-toggle"
name = "Lua Dialog Bool Toggle"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      if input.confirmed then
        return { content = "confirmed true" }
      end
      return { content = "confirmed false" }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "confirmed",
        label = "Confirmed",
        description = "Confirm submission",
        type = "boolean",
        default = false
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            (" \r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("confirmed true"), "{frame}");
    assert!(!frame.contains("must be boolean"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_submit_rejects_invalid_field_type() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-type");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-type"
name = "Lua Dialog Type"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "should not run" }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "confirmed",
        label = "Confirmed",
        description = "Confirm submission",
        type = "boolean",
        default = "yes"
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(
        frame.contains("Plugin dialog field Confirmed must be boolean"),
        "{frame}"
    );
    assert!(
        !frame.contains("Plugin command lua.submit_panel"),
        "{frame}"
    );
    assert!(!frame.contains("should not run"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_submit_rejects_missing_required_field() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-required");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-required"
name = "Lua Dialog Required"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "should not run" }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        required = true
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(
        frame.contains("Plugin dialog field Name is required"),
        "{frame}"
    );
    assert!(
        !frame.contains("Plugin command lua.submit_panel"),
        "{frame}"
    );
    assert!(!frame.contains("should not run"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_submit_uses_field_defaults_as_lua_args() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-fields");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-fields"
name = "Lua Dialog Fields"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "submitted " .. input.name }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel",
    fields = {
      {
        id = "name",
        label = "Name",
        description = "Target name",
        type = "text",
        default = "pi"
      }
    }
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("submitted pi"), "{frame}");
}

#[tokio::test]
async fn interactive_plugin_dialog_submit_runs_prefilled_lua_command() {
    let temp = tempfile::tempdir().unwrap();
    let project_plugin = temp.path().join(".pi-rust/plugins/lua-dialog-submit");
    std::fs::create_dir_all(&project_plugin).unwrap();
    std::fs::write(
        project_plugin.join("plugin.toml"),
        r#"
id = "lua-dialog-submit"
name = "Lua Dialog Submit"
version = "0.1.0"
runtime = "lua"
entry = "plugin.lua"
"#,
    )
    .unwrap();
    std::fs::write(
        project_plugin.join("plugin.lua"),
        r#"
function register(host)
  host:command({
    id = "lua.submit_panel",
    description = "submits the Lua panel",
    run = function(input)
      return { content = "submitted dialog" }
    end
  })
  host:ui_action({
    id = "ui.open_panel",
    label = "Open panel",
    description = "opens a Lua panel",
    action_id = "dialog.open_panel"
  })
  host:dialog({
    id = "dialog.open_panel",
    title = "Lua panel",
    description = "Panel registered by Lua",
    action_id = "lua.submit_panel"
  })
  host:keybind({
    id = "keybind.open_panel",
    key = "ctrl+g",
    description = "opens the Lua panel",
    action_id = "dialog.open_panel"
  })
end
"#,
    )
    .unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir_and_waits(
        provider,
        temp.path(),
        vec![
            ("/reload\r", "plugin.load.completed"),
            ("\u{7}", "plugin.load.completed"),
            ("\r", "plugin.load.completed"),
        ],
    )
    .await
    .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("Plugin command lua.submit_panel"), "{frame}");
    assert!(frame.contains("submitted dialog"), "{frame}");
}

#[tokio::test]
async fn interactive_resume_ignores_legacy_jsonl_sessions() {
    let temp = tempfile::tempdir().unwrap();
    write_legacy_session(
        temp.path(),
        temp.path(),
        "previous prompt",
        "previous answer",
        Some("Resume Target"),
    );

    let args = CliArgs {
        resume: true,
        ..CliArgs::default()
    };
    let provider = FauxProvider::new(Vec::new());
    let result =
        run_scripted_interactive_with_args_and_session_dir(provider, args, temp.path(), "")
            .await
            .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(!frame.contains("previous prompt"), "{frame}");
    assert!(!frame.contains("previous answer"), "{frame}");
    assert!(!frame.contains("• Resume Target"), "{frame}");
}

#[tokio::test]
async fn interactive_resume_loads_existing_rust_native_session_messages() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("rust-native answer")]);
    run_scripted_interactive_with_session_dir(provider, temp.path(), "rust-native prompt\r")
        .await
        .unwrap();

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 1);
    let manifest = read_session_manifest(&sessions[0]);
    let session_id = manifest["session_id"].as_str().unwrap();

    let args = CliArgs {
        resume: true,
        ..CliArgs::default()
    };
    let provider = FauxProvider::new(Vec::new());
    let result =
        run_scripted_interactive_with_args_and_session_dir(provider, args, temp.path(), "")
            .await
            .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("rust-native prompt"), "{frame}");
    assert!(frame.contains("rust-native answer"), "{frame}");
    assert!(
        frame.contains(&format!("• {}", &session_id[..13])),
        "{frame}"
    );
}

#[tokio::test]
async fn interactive_resume_restores_rust_native_footer_usage() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("usage answer")]);
    run_scripted_interactive_with_session_dir(provider, temp.path(), "usage prompt\r")
        .await
        .unwrap();

    let args = CliArgs {
        resume: true,
        ..CliArgs::default()
    };
    let provider = FauxProvider::new(Vec::new());
    let result =
        run_scripted_interactive_with_args_and_session_dir(provider, args, temp.path(), "")
            .await
            .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("usage prompt"), "{frame}");
    assert!(frame.contains("usage answer"), "{frame}");
    assert!(
        frame.contains("↑10") && frame.contains("↓20"),
        "resume footer should restore cumulative usage from the persisted Rust-native session: {frame}"
    );
}

#[tokio::test]
async fn interactive_resume_command_ignores_legacy_jsonl_sessions() {
    let temp = tempfile::tempdir().unwrap();
    write_legacy_session(
        temp.path(),
        temp.path(),
        "selected prompt",
        "selected answer",
        Some("Picked"),
    );

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "/resume\r\r")
        .await
        .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(!frame.contains("selected prompt"), "{frame}");
    assert!(!frame.contains("selected answer"), "{frame}");
    assert!(!frame.contains("Session selected: Picked"), "{frame}");
    assert!(!frame.contains("• Picked"), "{frame}");
}

#[tokio::test]
async fn interactive_resume_command_loads_selected_rust_native_session_messages() {
    let temp = tempfile::tempdir().unwrap();
    let provider = FauxProvider::new(vec![text_response("selected rust answer")]);
    run_scripted_interactive_with_session_dir(provider, temp.path(), "selected rust prompt\r")
        .await
        .unwrap();

    let sessions = rust_session_dirs(temp.path());
    assert_eq!(sessions.len(), 1);
    let manifest = read_session_manifest(&sessions[0]);
    let session_id = manifest["session_id"].as_str().unwrap();

    let provider = FauxProvider::new(Vec::new());
    let result = run_scripted_interactive_with_session_dir(provider, temp.path(), "/resume\r\r")
        .await
        .unwrap();
    let frame = result.rendered_lines.join("\n");

    assert!(frame.contains("selected rust prompt"), "{frame}");
    assert!(frame.contains("selected rust answer"), "{frame}");
    assert!(
        frame.contains(&format!("Session selected: {session_id}")),
        "{frame}"
    );
    assert!(
        frame.contains(&format!("• {}", &session_id[..13])),
        "{frame}"
    );
}

fn jsonl_files(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_jsonl_files(root, &mut files);
    files.sort();
    files
}

fn rust_session_dirs(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut dirs = Vec::new();
    collect_rust_session_dirs(root, &mut dirs);
    dirs.sort();
    dirs
}

fn collect_rust_session_dirs(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.join("session.json").is_file() && path.join("events.jsonl").is_file() {
                    out.push(path);
                } else {
                    collect_rust_session_dirs(&path, out);
                }
            }
        }
    }
}

fn read_session_manifest(session_dir: &std::path::Path) -> serde_json::Value {
    let text = std::fs::read_to_string(session_dir.join("session.json")).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn write_legacy_session(
    root: &std::path::Path,
    cwd: &std::path::Path,
    user_text: &str,
    assistant_text: &str,
    name: Option<&str>,
) -> std::path::PathBuf {
    let timestamp = create_timestamp();
    let file = root.join("legacy-session.jsonl");
    let mut entries = vec![
        serde_json::to_string(&SessionHeader {
            entry_type: "session".into(),
            version: 3,
            id: "legacy-session".into(),
            timestamp: timestamp.clone(),
            cwd: cwd.display().to_string(),
            parent_session: None,
        })
        .unwrap(),
        serde_json::to_string(&SessionEntry::message(
            "entry-user".to_string(),
            None,
            timestamp.clone(),
            StoredAgentMessage::User {
                content: vec![pi_ai::api::ContentBlock::Text {
                    text: user_text.to_string(),
                    text_signature: None,
                }],
                timestamp: 0,
            },
        ))
        .unwrap(),
        serde_json::to_string(&SessionEntry::message(
            "entry-assistant".to_string(),
            Some("entry-user".to_string()),
            timestamp.clone(),
            StoredAgentMessage::Assistant {
                content: vec![pi_ai::api::ContentBlock::Text {
                    text: assistant_text.to_string(),
                    text_signature: None,
                }],
                api: "test".to_string(),
                provider: "test".to_string(),
                model: "faux-model".to_string(),
                response_model: None,
                response_id: None,
                usage: StoredUsage::default(),
                stop_reason: StopReason::Stop,
                error_message: None,
                timestamp: 0,
            },
        ))
        .unwrap(),
    ];
    if let Some(name) = name {
        entries.push(
            serde_json::to_string(&SessionEntry::session_info(
                "entry-name".to_string(),
                Some("entry-assistant".to_string()),
                timestamp,
                name.to_string(),
            ))
            .unwrap(),
        );
    }
    std::fs::write(&file, entries.join("\n") + "\n").unwrap();
    file
}

fn collect_jsonl_files(root: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_jsonl_files(&path, out);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                out.push(path);
            }
        }
    }
}
