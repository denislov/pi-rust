# External Integrations

**Analysis Date:** 2026-07-10

## APIs & External Services

**AI model streaming protocols:**
- Anthropic Messages - Native Anthropic plus compatible Fireworks, GitHub Copilot, Kimi Coding, MiniMax, OpenCode, Vercel AI Gateway, and Cloudflare AI Gateway models from `crates/pi-ai/src/models_generated.json` are sent through `crates/pi-ai/src/providers/anthropic/`.
  - SDK/Client: Custom Reqwest 0.12 client with `x-api-key`, Anthropic version headers, JSON payload conversion, and SSE parsing in `crates/pi-ai/src/providers/anthropic/mod.rs` and `crates/pi-ai/src/providers/anthropic/sse.rs`.
  - Auth: Provider-specific keys mapped in `crates/pi-ai/src/util/env_keys.rs`, primarily `ANTHROPIC_API_KEY`, `FIREWORKS_API_KEY`, `COPILOT_GITHUB_TOKEN`, `KIMI_API_KEY`, `MINIMAX_API_KEY`, `OPENCODE_API_KEY`, `AI_GATEWAY_API_KEY`, and `CLOUDFLARE_API_KEY`.
- OpenAI-compatible Chat Completions - DeepSeek, Groq, Cerebras, xAI, OpenRouter, Moonshot, Hugging Face, Together, Z.ai, Xiaomi, OpenCode, GitHub Copilot, Cloudflare, and other catalog entries in `crates/pi-ai/src/models_generated.json` use `crates/pi-ai/src/providers/openai/completions/`.
  - SDK/Client: Custom Reqwest bearer-auth client posting to `/v1/chat/completions`, with provider-specific compatibility conversion and streamed response processing in `crates/pi-ai/src/providers/openai/completions/mod.rs` and `crates/pi-ai/src/providers/openai/completions/process.rs`.
  - Auth: Environment mappings such as `DEEPSEEK_API_KEY`, `GROQ_API_KEY`, `CEREBRAS_API_KEY`, `XAI_API_KEY`, `OPENROUTER_API_KEY`, `MOONSHOT_API_KEY`, `HF_TOKEN`, `TOGETHER_API_KEY`, `ZAI_API_KEY`, and Xiaomi/OpenCode/Cloudflare keys are centralized in `crates/pi-ai/src/util/env_keys.rs`.
- OpenAI Responses - OpenAI, OpenCode, GitHub Copilot, and Cloudflare AI Gateway catalog entries in `crates/pi-ai/src/models_generated.json` use `crates/pi-ai/src/providers/openai/responses/`.
  - SDK/Client: Custom Reqwest bearer-auth client posting to `/v1/responses`, with SSE event normalization in `crates/pi-ai/src/providers/openai/responses/mod.rs` and `crates/pi-ai/src/providers/openai/responses/process.rs`.
  - Auth: `OPENAI_API_KEY` or the provider-specific mapped token from `crates/pi-ai/src/util/env_keys.rs`; static GitHub Copilot compatibility headers come from `crates/pi-ai/src/models_generated.json` and dynamic request headers from `crates/pi-ai/src/providers/github_copilot_headers.rs`.
- OpenAI Codex Responses - ChatGPT Codex backend requests target `https://chatgpt.com/backend-api/codex/responses` by default in `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`.
  - SDK/Client: Custom Reqwest POST/SSE implementation in `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`; WebSocket URL and frame helpers exist in the same file, but the active provider stream sends HTTP POST and consumes SSE.
  - Auth: `OPENAI_CODEX_API_KEY`, falling back to `OPENAI_API_KEY`, mapped in `crates/pi-ai/src/util/env_keys.rs`; the bearer JWT is decoded for `chatgpt_account_id` and sent with Codex-specific headers by `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`.
- Google Generative AI - Gemini and compatible OpenCode catalog entries target Google-style `streamGenerateContent` endpoints through `crates/pi-ai/src/providers/google/`.
  - SDK/Client: Custom Reqwest streaming client and Google wire conversion in `crates/pi-ai/src/providers/google/mod.rs`, `crates/pi-ai/src/providers/google/convert.rs`, and `crates/pi-ai/src/providers/google/process.rs`.
  - Auth: `GEMINI_API_KEY` or `GOOGLE_API_KEY` from `crates/pi-ai/src/util/env_keys.rs`, placed in the request URL by `crates/pi-ai/src/providers/google/mod.rs`.
- Mistral Conversations - Mistral catalog models target `https://api.mistral.ai` through `crates/pi-ai/src/providers/mistral/`.
  - SDK/Client: Custom Reqwest bearer-auth client and streamed conversation conversion in `crates/pi-ai/src/providers/mistral/mod.rs` and `crates/pi-ai/src/providers/mistral/process.rs`.
  - Auth: `MISTRAL_API_KEY` from `crates/pi-ai/src/util/env_keys.rs`.
- Azure OpenAI Responses - Azure resource/deployment requests are built in `crates/pi-ai/src/providers/azure_openai_responses.rs` and have no fixed base URL in `crates/pi-ai/src/models_generated.json`.
  - SDK/Client: Custom Reqwest client posting to normalized Azure `/openai/v1/responses?api-version=...` URLs in `crates/pi-ai/src/providers/azure_openai_responses.rs`.
  - Auth: `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_API_VERSION`, `AZURE_OPENAI_BASE_URL` or `AZURE_OPENAI_RESOURCE_NAME`, and `AZURE_OPENAI_DEPLOYMENT_NAME_MAP` resolved by `crates/pi-ai/src/registry.rs` and `crates/pi-ai/src/util/env_keys.rs`.
- Amazon Bedrock Converse Stream - Bedrock runtime endpoints and model IDs come from `crates/pi-ai/src/models_generated.json` and are invoked by `crates/pi-ai/src/providers/bedrock/mod.rs`.
  - SDK/Client: Custom Reqwest event-stream client with in-repo AWS SigV4 signing in `crates/pi-ai/src/providers/bedrock/auth.rs`, `crates/pi-ai/src/providers/bedrock/sigv4.rs`, and `crates/pi-ai/src/providers/bedrock/process.rs`.
  - Auth: `AWS_BEARER_TOKEN_BEDROCK` or direct `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and optional `AWS_SESSION_TOKEN`; region comes from `AWS_REGION`/`AWS_DEFAULT_REGION` or the endpoint, as resolved in `crates/pi-ai/src/registry.rs` and `crates/pi-ai/src/providers/bedrock/mod.rs`.
- Dedicated DeepSeek Chat adapter - A separate `deepseek-chat-completions` API is registered in `crates/pi-ai/src/providers/mod.rs`, although current generated DeepSeek catalog entries route through the generic OpenAI-compatible adapter in `crates/pi-ai/src/models_generated.json`.
  - SDK/Client: Custom Reqwest bearer-auth client in `crates/pi-ai/src/providers/deepseek/mod.rs`.
  - Auth: `DEEPSEEK_API_KEY` or `DEEPSEEK_KEY` from `crates/pi-ai/src/util/env_keys.rs`.

**Provider catalog and gateways:**
- The checked-in catalog contains 921 models across 31 provider labels and eight catalog protocol families in `crates/pi-ai/src/models_generated.json`; built-in runtime registration adds nine API adapters in `crates/pi-ai/src/providers/mod.rs`.
  - SDK/Client: Catalog lookup and scoped provider dispatch use `AiClient` and `ProviderRegistry` in `crates/pi-ai/src/registry.rs` and `crates/pi-ai/src/models.rs`.
  - Auth: Authentication material is injected into `StreamOptions` by `ProviderAuthResolver` in `crates/pi-ai/src/registry.rs` before a provider adapter is called.
- Cloudflare endpoint templates substitute `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_GATEWAY_ID` into catalog base URLs in `crates/pi-ai/src/providers/cloudflare.rs` and `crates/pi-ai/src/models_generated.json`.
  - SDK/Client: Template expansion plus the applicable Anthropic/OpenAI Reqwest adapter in `crates/pi-ai/src/providers/cloudflare.rs` and `crates/pi-ai/src/providers/`.
  - Auth: `CLOUDFLARE_API_KEY`, `CLOUDFLARE_ACCOUNT_ID`, and `CLOUDFLARE_GATEWAY_ID` as mapped or substituted by `crates/pi-ai/src/util/env_keys.rs` and `crates/pi-ai/src/providers/cloudflare.rs`.

**Local extension and automation interfaces:**
- Lua plugins - Project plugins under `<cwd>/.pi-rust/plugins` and user plugins under `${PI_RUST_DIR:-~/.pi-rust}/plugins` are discovered from `plugin.toml` manifests by `crates/pi-coding-agent/src/coding_session/mod.rs` and `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.
  - SDK/Client: Vendored Lua 5.4 through mlua, exposing tools, commands, prompt hooks, UI actions/dialogs, keybindings, and flow extensions through `crates/pi-coding-agent/src/plugins/` and `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.
  - Auth: Not applicable; Lua starts with only table/string/math/UTF-8 standard libraries in `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`, and capabilities are registered through the host API.
- Stdio RPC/JSONL - External harnesses can drive the coding agent over stdin/stdout through `crates/pi-coding-agent/src/protocol/rpc.rs` and `crates/pi-coding-agent/src/protocol/jsonl.rs`.
  - SDK/Client: Newline-delimited Serde JSON command/response protocol with product-event streaming in `crates/pi-coding-agent/src/protocol/rpc/`.
  - Auth: None; access control is inherited from the local process boundary in `crates/pi-coding-agent/src/main.rs`.
- Operating-system tools - Shell execution, clipboard commands, Git branch discovery, and theme file watching integrate with the host OS through `crates/pi-coding-agent/src/tools/bash.rs`, `crates/pi-coding-agent/src/interactive/clipboard.rs`, `crates/pi-coding-agent/src/interactive/git_branch.rs`, and `crates/pi-coding-agent/src/theme/reload.rs`.
  - SDK/Client: Tokio/std process APIs, platform clipboard programs (`pbcopy`, PowerShell, `wl-copy`, `xclip`, `xsel`, or Termux), `git`, and notify 7.0 in the referenced files.
  - Auth: Local OS user permissions and the operation capability snapshot in `crates/pi-coding-agent/src/coding_session/capability_snapshot.rs`.

## Data Storage

**Databases:**
- None - No database driver or ORM appears in `Cargo.toml` or any `crates/*/Cargo.toml`; durable state is filesystem-based in `crates/pi-coding-agent/src/coding_session/session_log/`.
  - Connection: Not applicable; session roots are filesystem paths resolved by `crates/pi-coding-agent/src/session.rs`.
  - Client: Rust standard-library filesystem APIs in `crates/pi-coding-agent/src/coding_session/session_log/store.rs`.

**File Storage:**
- Local filesystem only - Each session uses `session.json`, `events.jsonl`, `blobs/`, and `index/` below `${PI_RUST_DIR:-~/.pi-rust}/sessions` or an overridden session directory, as defined in `crates/pi-coding-agent/src/session.rs`, `crates/pi-coding-agent/src/coding_session/session_log/manifest.rs`, and `crates/pi-coding-agent/src/coding_session/session_log/store.rs`.
- Global and project settings are TOML files resolved by `crates/pi-coding-agent/src/config/paths.rs` and merged by `crates/pi-coding-agent/src/config/settings.rs`.
- Skills, prompt templates, JSON themes, profiles, and plugin manifests/scripts are loaded from user/project resource directories by `crates/pi-coding-agent/src/resources.rs`, `crates/pi-coding-agent/src/coding_session/profiles.rs`, and `crates/pi-coding-agent/src/coding_session/plugin_load_flow.rs`.

**Caching:**
- No external cache - In-memory provider registries, model data, event broadcasts, retained product events, and TUI state are process-local in `crates/pi-ai/src/registry.rs`, `crates/pi-ai/src/models.rs`, and `crates/pi-coding-agent/src/coding_session/event_service.rs`.

## Authentication & Identity

**Auth Provider:**
- Custom local provider-credential store - Global `auth.toml` supports API-key and OAuth-access-token entries in `crates/pi-coding-agent/src/config/auth.rs`; no application user-account or session-login service is present in `crates/pi-coding-agent/src/`.
  - Implementation: Credential precedence is explicit CLI key, provider environment variable, then global auth-file API key or OAuth access token in `crates/pi-coding-agent/src/config/auth.rs`; environment aliases are mapped in `crates/pi-ai/src/util/env_keys.rs`.
  - Implementation: Auth values may reference `$VAR`/`${VAR}` rather than embedding a value, and Unix saves enforce mode `0600`, in `crates/pi-coding-agent/src/config/auth.rs`.
  - Implementation: Generic PKCE challenge and escaped callback-page helpers exist in `crates/pi-ai/src/util/oauth.rs`, while provider-specific bearer/header handling is implemented in `crates/pi-ai/src/providers/`.
  - Implementation: OpenAI Codex derives the ChatGPT account ID from a bearer JWT in `crates/pi-ai/src/providers/openai_codex_responses/mod.rs`; GitHub Copilot adds integration and initiator headers in `crates/pi-ai/src/providers/github_copilot_headers.rs` and `crates/pi-ai/src/models_generated.json`.

## Monitoring & Observability

**Error Tracking:**
- None external - No tracing, Sentry, OpenTelemetry, or hosted error-tracking dependency is declared in `Cargo.toml` or `crates/*/Cargo.toml`; provider errors are normalized in `crates/pi-ai/src/transport/error.rs` and product diagnostics in `crates/pi-coding-agent/src/coding_session/event_service.rs`.

**Logs:**
- CLI/configuration failures and warnings are rendered to stderr by `crates/pi-coding-agent/src/main.rs`, `crates/pi-coding-agent/src/config/mod.rs`, and `crates/pi-coding-agent/src/request.rs`.
- Runtime observability uses structured `CodingAgentEvent` and `ProductEvent` broadcast channels with retained replay in `crates/pi-coding-agent/src/coding_session/event_service.rs`.
- Durable session history is newline-delimited JSON in `events.jsonl`, with a versioned `session.json` manifest, in `crates/pi-coding-agent/src/coding_session/session_log/manifest.rs` and `crates/pi-coding-agent/src/coding_session/session_log/store.rs`.
- Provider response hooks and auth-source diagnostics are exposed by `crates/pi-ai/src/transport/http.rs`, `crates/pi-ai/src/types/hooks.rs`, and `crates/pi-ai/src/registry.rs` rather than sent to an external telemetry backend.

## CI/CD & Deployment

**Hosting:**
- Local native executable - The deployable product entry point is `crates/pi-coding-agent/src/main.rs`; `crates/pi-web-ui/` is an empty placeholder and no server/hosting package is configured in `crates/pi-web-ui/Cargo.toml`.

**CI Pipeline:**
- None detected - The repository is Cargo-driven through `Cargo.toml`, `Cargo.lock`, and `crates/*/Cargo.toml`, with no `.github/workflows`, GitLab pipeline, container manifest, or other CI/deployment file alongside them.
- Manual TUI validation is provided by `scripts/tui-smoke.sh`, which builds the debug binary and captures tmux output under `target/tui-smoke/`.

## Environment Configuration

**Required env vars:**
- Core path overrides: `PI_RUST_DIR` and optional `PI_SESSION_DIR` in `crates/pi-coding-agent/src/config/paths.rs` and `crates/pi-coding-agent/src/session.rs`.
- Anthropic/OpenAI family: `ANTHROPIC_API_KEY`, `CLAUDE_API_KEY`, `ANTHROPIC_KEY`, `OPENAI_API_KEY`, `OPENAI_CODEX_API_KEY`, `DEEPSEEK_API_KEY`, and `DEEPSEEK_KEY` in `crates/pi-ai/src/util/env_keys.rs`.
- Azure OpenAI: `AZURE_OPENAI_API_KEY`, `AZURE_OPENAI_API_VERSION`, `AZURE_OPENAI_BASE_URL`, `AZURE_OPENAI_RESOURCE_NAME`, and `AZURE_OPENAI_DEPLOYMENT_NAME_MAP` in `crates/pi-ai/src/registry.rs` and `crates/pi-ai/src/util/env_keys.rs`.
- AWS Bedrock: `AWS_REGION` or `AWS_DEFAULT_REGION`, `AWS_BEARER_TOKEN_BEDROCK`, `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, and optional `AWS_SESSION_TOKEN` in `crates/pi-ai/src/registry.rs`; `AWS_PROFILE` is only detected as a credential-presence hint in `crates/pi-ai/src/util/env_keys.rs`.
- Google/Mistral: `GEMINI_API_KEY`, `GOOGLE_API_KEY`, and `MISTRAL_API_KEY` in `crates/pi-ai/src/util/env_keys.rs`.
- Compatible providers: `GROQ_API_KEY`, `CEREBRAS_API_KEY`, `XAI_API_KEY`, `OPENROUTER_API_KEY`, `AI_GATEWAY_API_KEY`, `ZAI_API_KEY`, `MOONSHOT_API_KEY`, `HF_TOKEN`, `FIREWORKS_API_KEY`, `TOGETHER_API_KEY`, `OPENCODE_API_KEY`, `KIMI_API_KEY`, `CLOUDFLARE_API_KEY`, `MINIMAX_API_KEY`, and `MINIMAX_CN_API_KEY` in `crates/pi-ai/src/util/env_keys.rs`.
- Xiaomi and Copilot: `XIAOMI_API_KEY`, `XIAOMI_TOKEN_PLAN_CN_API_KEY`, `XIAOMI_TOKEN_PLAN_AMS_API_KEY`, `XIAOMI_TOKEN_PLAN_SGP_API_KEY`, and `COPILOT_GITHUB_TOKEN` in `crates/pi-ai/src/util/env_keys.rs`.
- Cloudflare URL placeholders additionally require `CLOUDFLARE_ACCOUNT_ID` and `CLOUDFLARE_GATEWAY_ID` as consumed by `crates/pi-ai/src/providers/cloudflare.rs` and embedded in `crates/pi-ai/src/models_generated.json`.
- Optional TUI smoke controls are `PI_RUST_TUI_SMOKE_REAL_PROMPT`, `PI_RUST_TUI_SMOKE_REAL_WAIT`, and `OUT_DIR` in `scripts/tui-smoke.sh`.

**Secrets location:**
- Provider secrets are expected in the process environment or the global auth file resolved as `${PI_RUST_DIR:-~/.pi-rust}/auth.toml` by `crates/pi-coding-agent/src/config/paths.rs` and loaded by `crates/pi-coding-agent/src/config/auth.rs`.
- Auth-file entries can store environment references instead of literal values, and Unix writes are restricted to `0600`, in `crates/pi-coding-agent/src/config/auth.rs`.
- No repository `.env` file is tracked or detected; secret loading is implemented through process environment lookup in `crates/pi-ai/src/util/env_keys.rs` and `crates/pi-coding-agent/src/config/auth.rs`.

## Webhooks & Callbacks

**Incoming:**
- None - No HTTP listener or webhook route exists in `crates/pi-coding-agent/src/` or `crates/pi-ai/src/`; the only machine-facing command ingress is local JSONL over stdin in `crates/pi-coding-agent/src/protocol/rpc.rs`.
- OAuth success/error HTML helpers exist in `crates/pi-ai/src/util/oauth.rs`, but no callback server or bound network port is implemented in that module.

**Outgoing:**
- No outgoing webhooks - External traffic consists of direct model-provider HTTPS requests and streamed SSE or Bedrock event-stream responses from `crates/pi-ai/src/providers/` and `crates/pi-ai/src/transport/http.rs`.
- Provider payload and response hooks are in-process extension callbacks, not network webhooks, and are defined in `crates/pi-ai/src/types/hooks.rs` and invoked by `crates/pi-ai/src/transport/http.rs`.

---

*Integration audit: 2026-07-10*
