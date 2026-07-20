//! Owner-crate behavior tests for private product adapters and built-in tools.

mod support;

pub(crate) mod cli_fixture {
    pub(crate) mod command {
        pub(crate) use crate::app::cli::args::*;
        pub(crate) use crate::app::cli::error::*;
        pub(crate) use crate::app::cli::request::*;
        pub(crate) use crate::app::session::*;
    }

    pub(crate) mod configuration {
        pub(crate) use crate::app::bootstrap::*;
        pub(crate) use crate::app::cli::models::*;
        pub(crate) use crate::config::auth::*;
        pub(crate) use crate::config::settings::*;
        pub(crate) use crate::config::*;
    }

    pub(crate) mod input {
        pub(crate) use crate::app::cli::input::*;
    }

    pub(crate) mod resources {
        pub(crate) use crate::resources::*;
        pub(crate) use crate::tools::*;
    }

    pub(crate) mod theme {
        pub(crate) use crate::theme::*;
    }
}

#[path = "../../tests/config_request/args.rs"]
mod cli_args;
#[path = "../../tests/config_request/config_wiring.rs"]
mod config_wiring;
#[path = "../../tests/rpc/protocol_args.rs"]
mod protocol_args;
#[path = "../../tests/config_request/request_resolution.rs"]
mod request_resolution;
#[path = "../../tests/config_request/runtime_configuration.rs"]
mod runtime_configuration;
#[path = "../../tests/config_request/session_args.rs"]
mod session_args;
#[path = "../../tests/config_request/theme.rs"]
mod theme;
#[path = "../../tests/tools/e2e.rs"]
mod tool_e2e;

mod file_mutation_queue;
mod interactive_abort;
mod interactive_event_bridge;
mod interactive_mode;
mod interactive_sessions;
mod interactive_transcript;
mod m10_resources_input;
mod tool_bash;
mod tool_edit;
mod tool_find;
mod tool_grep;
mod tool_ls;
mod tool_operations;
mod tool_read;
mod tool_write;
