const THEME_RELOAD_SOURCE: &str = include_str!("../src/theme/reload.rs");
const THEME_TEST_SOURCE: &str = include_str!("theme.rs");
const TOOL_BASH_TEST_SOURCE: &str = include_str!("tool_bash.rs");
const INTERACTIVE_APP_SOURCE: &str = include_str!("../src/interactive/app.rs");
const INTERACTIVE_LOOP_SOURCE: &str = include_str!("../src/interactive/loop.rs");
const RPC_MODE_TEST_SOURCE: &str = include_str!("rpc_mode.rs");
const PROTOCOL_SESSIONS_TEST_SOURCE: &str = include_str!("protocol_sessions.rs");
const INTERACTIVE_MODE_TEST_SOURCE: &str = include_str!("interactive_mode.rs");
const INTERACTIVE_ABORT_TEST_SOURCE: &str = include_str!("interactive_abort.rs");
const FILE_MUTATION_QUEUE_TEST_SOURCE: &str = include_str!("file_mutation_queue.rs");

#[test]
fn theme_reload_worker_does_not_poll_with_thread_sleep() {
    assert!(
        !THEME_RELOAD_SOURCE.contains("std::thread::sleep"),
        "theme reload worker should use condition-based waiting instead of fixed thread sleeps"
    );
}

#[test]
fn tool_bash_timeout_tests_do_not_poll_with_fixed_sleep() {
    assert!(
        !TOOL_BASH_TEST_SOURCE.contains("tokio::time::sleep"),
        "tool_bash timeout tests should use process-exit observation instead of fixed sleep polling"
    );
}

#[test]
fn tool_bash_hang_guards_use_named_timeout_helper() {
    let mut violations = Vec::new();
    let lines: Vec<_> = TOOL_BASH_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(") {
            continue;
        }
        let prefix = lines[index.saturating_sub(8)..index].join("\n");
        if prefix.contains("async fn run_bash_with_hang_guard") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("bash_execute(") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "tool_bash command hang tests should route bash_execute timeouts through a named helper instead of scattering fixed-duration harness waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn tool_bash_pid_exit_waits_use_named_timeout_constants() {
    let mut violations = Vec::new();
    let lines: Vec<_> = TOOL_BASH_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("wait_for_pid_to_exit(") || line.contains("async fn wait_for_pid_to_exit")
        {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 4, lines.len())].join("\n");
        if window.contains("Duration::from_") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "tool_bash PID-exit observation waits should use named timeout constants instead of inline fixed durations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn tool_bash_shell_timing_literals_use_named_constants() {
    let mut violations = Vec::new();

    for (index, line) in TOOL_BASH_TEST_SOURCE.lines().enumerate() {
        if contains_numeric_literal_after(line, "sleep ")
            || contains_numeric_literal_after(line, "\"timeout\":")
            || contains_numeric_literal_after(line, "Command timed out after ")
        {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "tool_bash timing-sensitive shell sleeps, command timeouts, and timeout assertions should use named constants/helpers instead of inline fixed values:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_loop_shutdown_drain_uses_named_durations() {
    let mut violations = Vec::new();
    let lines: Vec<_> = INTERACTIVE_LOOP_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains(".drain_input(") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 4, lines.len())].join("\n");
        if window.contains("Duration::from_") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "interactive loop shutdown drain should use named duration constants instead of inline fixed durations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_harness_session_wait_does_not_poll_with_fixed_sleep() {
    assert!(
        !INTERACTIVE_APP_SOURCE
            .contains("tokio::time::sleep(tokio::time::Duration::from_millis(10))"),
        "interactive harness session-log waits should use cooperative observation instead of fixed sleep polling"
    );
}

#[test]
fn interactive_idle_flush_harness_advances_paused_time_instead_of_sleeping() {
    assert!(
        !INTERACTIVE_APP_SOURCE.contains("tokio::time::sleep(delay).await"),
        "interactive idle-flush tests should advance paused Tokio time instead of sleeping real time"
    );
}

#[test]
fn rpc_mode_line_reads_use_named_timeout_helpers() {
    let mut violations = Vec::new();
    let lines: Vec<_> = RPC_MODE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(Duration::from") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("lines.next_line()") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "rpc_mode tests should route line reads through named timeout helpers instead of scattering fixed-duration next_line waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn rpc_mode_non_line_waits_use_named_timeout_helpers() {
    let mut violations = Vec::new();
    let lines: Vec<_> = RPC_MODE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(Duration::from") {
            continue;
        }
        let prefix = lines[index.saturating_sub(8)..index].join("\n");
        if prefix.contains("async fn read_rpc_output_bytes")
            || prefix.contains("async fn await_rpc_task_completion")
            || prefix.contains("async fn wait_for_rpc_provider_start")
        {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 6, lines.len())].join("\n");
        if window.contains("output_reader.read(")
            || window.contains(", task)")
            || window.contains(", started_rx)")
        {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "rpc_mode tests should route non-line synchronization waits through named timeout helpers instead of scattering fixed-duration waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn protocol_session_line_reads_use_named_timeout_helpers() {
    let mut violations = Vec::new();
    let lines: Vec<_> = PROTOCOL_SESSIONS_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(Duration::from") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("lines.next_line()") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "protocol session tests should route line reads through named timeout helpers instead of scattering fixed-duration next_line waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_mode_observed_driver_tests_use_named_harness_timeout_helper() {
    let mut violations = Vec::new();
    let lines: Vec<_> = INTERACTIVE_MODE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(") {
            continue;
        }
        let prefix = lines[index.saturating_sub(16)..index].join("\n");
        if prefix.contains("async fn run_observed_interactive_with_timeout") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("run_scripted_interactive_with_observed_provider_driver") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "interactive_mode observed-driver tests should route harness timeouts through a named helper instead of scattering fixed-duration observed-driver waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_mode_observed_driver_timeouts_use_named_constants() {
    let mut violations = Vec::new();
    let lines: Vec<_> = INTERACTIVE_MODE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("run_observed_interactive_with_timeout(")
            || line.contains("async fn run_observed_interactive_with_timeout")
        {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 32, lines.len())].join("\n");
        if window.contains("Duration::from_") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "interactive_mode observed-driver timeout values should be named constants instead of per-call fixed durations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_mode_idle_input_delays_use_named_constants() {
    let mut violations = Vec::new();
    let lines: Vec<_> = INTERACTIVE_MODE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("run_scripted_idle_interactive_with_delays(") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 10, lines.len())].join("\n");
        if window.contains("Duration::from_millis") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "interactive_mode idle input delays should use named constants instead of inline fixed durations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn file_mutation_queue_tests_use_named_channel_timeout_helper() {
    let mut violations = Vec::new();
    let lines: Vec<_> = FILE_MUTATION_QUEUE_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(Duration::from") {
            continue;
        }
        let prefix = lines[index.saturating_sub(6)..index].join("\n");
        if prefix.contains("async fn recv_file_mutation_signal") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 4, lines.len())].join("\n");
        if window.contains("entered_rx") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "file_mutation_queue tests should route channel waits through a named timeout helper instead of scattering fixed-duration entered_rx waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn interactive_abort_tests_use_named_harness_timeout_helper() {
    let mut violations = Vec::new();
    let lines: Vec<_> = INTERACTIVE_ABORT_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(") {
            continue;
        }
        let prefix = lines[index.saturating_sub(6)..index].join("\n");
        if prefix.contains("async fn run_abort_harness_with_timeout") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("run_scripted_interactive") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "interactive abort tests should route harness run timeouts through a named helper instead of scattering fixed-duration run_scripted_interactive waits:\n{}",
        violations.join("\n")
    );
}

#[test]
fn theme_reload_unit_tests_use_named_time_constants() {
    let mut violations = Vec::new();
    let lines: Vec<_> = THEME_RELOAD_SOURCE.lines().collect();
    let start_index = source_tests_start_index(&lines, "theme reload");

    for index in start_index..lines.len() {
        let line = lines[index];
        if !line.contains("Duration::from_millis") {
            continue;
        }
        if line.trim_start().starts_with("const THEME_RELOAD_TEST_") {
            continue;
        }
        violations.push(format!("{}: {}", index + 1, line.trim()));
    }

    assert!(
        violations.is_empty(),
        "theme reload unit tests should use named timing constants instead of inline fixed durations:\n{}",
        violations.join("\n")
    );
}

#[test]
fn theme_reload_unit_tests_use_named_clock_anchor() {
    let mut violations = Vec::new();
    let lines: Vec<_> = THEME_RELOAD_SOURCE.lines().collect();
    let start_index = source_tests_start_index(&lines, "theme reload");

    for index in start_index..lines.len() {
        let line = lines[index];
        if !line.contains("Instant::now()") {
            continue;
        }
        let prefix = lines[index.saturating_sub(2)..=index].join("\n");
        if prefix.contains("fn theme_reload_test_clock_anchor") {
            continue;
        }
        violations.push(format!("{}: {}", index + 1, line.trim()));
    }

    assert!(
        violations.is_empty(),
        "theme reload unit tests should use a named clock anchor helper instead of scattering Instant::now():\n{}",
        violations.join("\n")
    );
}

#[test]
fn theme_watcher_tests_use_named_debounce_durations() {
    let mut violations = Vec::new();
    let lines: Vec<_> = THEME_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("ThemeWatcher::start(") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 8, lines.len())].join("\n");
        if window.contains("Duration::from_millis") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "theme watcher tests should use named debounce duration constants instead of inline fixed durations:\n{}",
        violations.join("\n")
    );
}

fn contains_numeric_literal_after(line: &str, marker: &str) -> bool {
    line.split_once(marker)
        .and_then(|(_, suffix)| suffix.trim_start().chars().next())
        .is_some_and(|character| character.is_ascii_digit())
}

fn source_tests_start_index(lines: &[&str], source_name: &str) -> usize {
    lines
        .iter()
        .position(|line| line.trim() == "#[cfg(test)]")
        .unwrap_or_else(|| panic!("{source_name} source should contain a #[cfg(test)] module"))
}

#[test]
fn theme_watcher_tests_use_named_signal_timeout_helper() {
    let mut violations = Vec::new();
    let lines: Vec<_> = THEME_TEST_SOURCE.lines().collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.contains("tokio::time::timeout(Duration::from") {
            continue;
        }
        let window = lines[index..std::cmp::min(index + 4, lines.len())].join("\n");
        if window.contains("signal.recv()") {
            violations.push(format!("{}: {}", index + 1, line.trim()));
        }
    }

    assert!(
        violations.is_empty(),
        "theme watcher tests should route reload signal waits through a named timeout helper instead of scattering fixed-duration signal.recv waits:\n{}",
        violations.join("\n")
    );
}
