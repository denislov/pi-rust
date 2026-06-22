# pi-ai 工程质量优化方案

本文档面向 `pi-rust/crates/pi-ai` 的后续重构和维护，目标是把当前“迁移期可运行、以 TS parity 为优先”的 crate，逐步提升为边界清晰、行为一致、可长期演进的 Rust 库。

结论先行：`pi-ai` 当前基础质量不错，provider 已按 `convert/process/wire` 拆分，fixture 测试和模型 registry 也比较完整。但它的横切能力还没有收敛：请求发送、headers、hooks、retry、timeout、cancel、错误结构、全局 registry 和 provider-specific options 都存在不同程度的不一致。优化不应从大规模重写 provider 开始，而应采用 strangler-style incremental refactor：先建立共享基础设施，再逐步迁移现有 provider。

## 目标

### 核心目标

- 保持现有 `pi-ai` 对 TS `pi/packages/ai` 的行为兼容，尤其是公开类型的 serde 形状、provider request mapping、stream event 语义和离线 fixture 测试。
- 收敛 provider 横切行为，确保同一组选项在不同 provider 中有一致语义。
- 减少 provider `mod.rs` 中重复的网络发送、错误处理、header 合并和 stream 桥接代码。
- 提高 Rust API 的类型安全，降低 `serde_json::Value` 和全局 option bag 的长期维护成本。
- 保留迁移速度，不做一次性重写，不引入不必要的框架或宏系统。

### 非目标

- 不改变 TS reference repo。
- 不在第一轮重构中改变上层 crate 的用户体验或模型选择逻辑。
- 不要求真实 provider key 参与测试。
- 不立即替换所有 `AssistantMessageEvent` 的事件形状；事件快照成本先通过测量和局部优化处理。
- 不把 provider 拆成多个新 crate。当前 `pi-ai` 的规模还可以通过模块边界优化解决。

## 当前设计的优点

`pi-ai` 已经具备几个值得保留的设计基础：

- Provider 目录结构基本正确：多数 provider 使用 `convert.rs` 负责内部类型到 provider request 的映射，`wire.rs` 负责协议 schema，`process.rs` 负责 stream response 到统一事件的处理。
- 公共类型贴近 TS schema：`Model`、`Message`、`ContentBlock`、`AssistantMessage`、`AssistantMessageEvent` 的 serde 形状便于和 TS fixture 对齐。
- 离线测试基础较强：SSE fixture、serde roundtrip、model registry、cost、env keys、Bedrock SigV4、faux provider 都已有覆盖。
- `process_framework.rs` 已经把 SSE 循环抽出一部分，说明代码已经开始朝共享 framework 方向演进。

这些基础应继续保留。优化重点不是推翻 provider 分层，而是把目前散在 provider `mod.rs` 中的横切行为收拢。

## 主要问题

### 1. `StreamOptions` 变成全局 option bag

文件：`crates/pi-ai/src/types/stream_opts.rs`

当前 `StreamOptions` 同时承载：

- 通用 generation 选项：`temperature`、`max_tokens`、`thinking`、`tool_choice`。
- credential/header 选项：`api_key`、`headers`。
- cache/session 选项：`cache_retention`、`session_id`。
- Azure 专属选项：`azure_api_version`、`azure_resource_name`、`azure_base_url`、`azure_deployment_name`。
- Bedrock 专属选项：`bedrock_region`、`bedrock_profile`、`bedrock_bearer_token`。
- transport 选项：`cancel`、`timeout_ms`、`max_retries`、`max_retry_delay_ms`。
- hook 选项：`hooks`。

这个结构迁移期很方便，但长期会带来三个问题：

- 任意 provider 都能收到无关字段，行为边界不清晰。
- 新增 provider-specific option 会继续污染公共 struct。
- `cache_retention`、`tool_choice`、`headers` 使用 `serde_json::Value`，调用方无法得到编译期约束。

### 2. transport 行为重复且不一致

文件示例：

- `crates/pi-ai/src/providers/openai/completions/mod.rs`
- `crates/pi-ai/src/providers/openai/responses/mod.rs`
- `crates/pi-ai/src/providers/google/mod.rs`
- `crates/pi-ai/src/providers/anthropic/mod.rs`
- `crates/pi-ai/src/providers/mistral/mod.rs`
- `crates/pi-ai/src/providers/azure_openai_responses.rs`

这些文件里重复出现以下流程：

1. resolve API key。
2. build request payload。
3. build URL。
4. construct `reqwest::RequestBuilder`。
5. append headers。
6. send request。
7. map network error。
8. map non-success HTTP status。
9. turn response into `bytes_stream()`。
10. hand off to `process::process(...)`。

重复本身不只是代码风格问题。它已经导致行为不一致：

- OpenAI/Google/Azure 读取 `RetryConfig.timeout_ms`，Anthropic/Mistral/Bedrock 等路径没有统一 timeout。
- `RetryConfig.max_retries` 和 `parse_retry_after_ms` 已存在，但多数发送路径没有实际 retry loop。
- `ProviderStreamHooks` 已暴露，却没有统一应用到 payload 和 response。
- `model.headers` 和 `opts.headers` 的合并规则不一致，Mistral/Azure/Codex 有合并逻辑，OpenAI/Anthropic/Google 主要只读 `opts.headers`。

### 3. hooks/retry API 的合约未兑现

文件：

- `crates/pi-ai/src/types/hooks.rs`
- `crates/pi-ai/src/types/stream_opts.rs`
- `crates/pi-ai/src/util/http.rs`

当前代码已经公开：

- `ProviderStreamHooks::apply_payload`
- `ProviderStreamHooks::emit_response`
- `StreamOptions.hooks`
- `StreamOptions.max_retries`
- `StreamOptions.max_retry_delay_ms`
- `util::http::is_retryable_status`
- `util::http::parse_retry_after_ms`

但主发送路径没有完整调用这些能力。公开 API 一旦存在，调用方会合理期待其生效；如果只是部分 provider 生效或完全不生效，维护成本会迅速增加。

### 4. 全局 mutable registry 降低组合性和测试隔离

文件：`crates/pi-ai/src/registry.rs`

当前 registry 是进程级全局：

```rust
static REGISTRY: LazyLock<RwLock<HashMap<String, Arc<dyn ApiProvider>>>> =
    LazyLock::new(|| RwLock::new(HashMap::new()));
```

这个设计简单，但有几个长期问题：

- 测试共享全局状态，容易出现顺序依赖或并发污染。
- 上层 crate 难以创建多个互不干扰的 provider 集合。
- `RwLock::write().unwrap()` / `read().unwrap()` 在锁 poisoned 时会 panic。
- `register_builtins()` 是全局副作用，不适合嵌入式或库式使用。

### 5. 错误仍以字符串为主

文件示例：

- `crates/pi-ai/src/stream.rs`
- `crates/pi-ai/src/providers/*/mod.rs`
- `crates/pi-ai/src/util/diagnostics.rs`

当前 `complete()` 返回 `Result<AssistantMessage, String>`，provider HTTP/network error 多数是 `format!("HTTP {} : {}", status, body)`。这不利于上层做：

- 限流和 retry-after 判断。
- context overflow 分类。
- auth missing 分类。
- provider transport failure diagnostics。
- TUI 中的友好错误显示。

crate 已经有结构化 diagnostics helper，但主错误路径还没有真正使用它。

### 6. stream event 快照成本可能随输出长度放大

文件示例：

- `crates/pi-ai/src/types/event.rs`
- `crates/pi-ai/src/providers/openai/completions/process.rs`
- `crates/pi-ai/src/providers/openai/responses/process.rs`

每个 delta event 都携带完整 `partial: AssistantMessage`，处理器在每次 delta 后 clone `partial`。这和 TS 引用对象的成本模型不同。Rust 中如果输出很长、工具参数很长，重复 clone 会造成明显内存和 CPU 成本。

这个问题不建议第一阶段改变公开事件协议，但应纳入后续测量和优化。

## 推荐总体路线

推荐采用 5 个阶段推进：

1. 建立 shared transport foundation。
2. 迁移 provider 到 shared transport。
3. 收紧 options 和 provider-specific 配置。
4. 引入显式 `Registry`，保留全局 convenience API。
5. 收敛错误、diagnostics、事件快照和 public API 暴露面。

每个阶段都应保持 `cargo test -p pi-ai` 通过。涉及跨 crate 调用时，再运行 `cargo test --workspace`。

## 阶段 0：建立重构护栏

### 目的

先补足横切行为的 characterization tests，避免后续重构改变兼容行为。

### 建议新增或扩展的测试

文件建议：

- `crates/pi-ai/tests/transport_contract.rs`
- `crates/pi-ai/tests/provider_options_contract.rs`
- `crates/pi-ai/tests/registry_contract.rs`

测试重点：

- `model.headers` 和 `opts.headers` 合并优先级：`opts.headers` 覆盖 `model.headers`，provider 自动 header 不被无意覆盖，除非明确允许。
- `timeout_ms` 对所有 HTTP provider 一致生效。
- `max_retries = 0` 不重试。
- `max_retries = 2` 对 retryable status 最多发起 3 次请求。
- `Retry-After` 大于 `max_retry_delay_ms` 时返回结构化错误，不静默等待过久。
- `hooks.on_payload` 能修改 payload。
- `hooks.on_response` 能收到 status 和 headers。
- hook error 会变成 error event，并保留 provider/model 信息。
- cancel token 在发送前、等待 retry sleep 时、stream 消费中都能中止。

### 验收标准

- 新测试先针对当前行为明确标注哪些是 expected failure 或分阶段启用。
- 不改变 provider 实现。
- `cargo test -p pi-ai` 通过。

## 阶段 1：建立 shared transport foundation

### 目的

把 provider 共享的 HTTP/SSE 发送骨架收敛到一个模块，provider 只负责 provider-specific 的 request 构造和 response stream 解析。

### 建议文件结构

新增：

```text
crates/pi-ai/src/transport/
  mod.rs
  headers.rs
  http.rs
  retry.rs
  error.rs
```

保留但逐步迁移：

```text
crates/pi-ai/src/util/http.rs
```

最终 `util::http` 可以 re-export `transport::retry` 的稳定 helper，避免一次性破坏测试路径。

### 核心类型草案

```rust
pub struct ProviderRequest {
    pub api_name: &'static str,
    pub method: reqwest::Method,
    pub url: String,
    pub auth: ProviderAuth,
    pub headers: HeaderMapSpec,
    pub payload: serde_json::Value,
}

pub enum ProviderAuth {
    Bearer(String),
    Header { name: &'static str, value: String },
    QueryParam { name: &'static str, value: String },
    None,
}

pub struct ProviderSendContext {
    pub model: Model,
    pub opts: Option<StreamOptions>,
}

pub struct ProviderHttpResponse {
    pub status: u16,
    pub headers: serde_json::Value,
    pub body: reqwest::Response,
}
```

注意：这只是第一版设计方向，不要求一开始把所有 provider 都改成返回这些类型。可以先做一个低侵入 helper：

```rust
pub async fn send_json_stream(
    client: &reqwest::Client,
    model: &Model,
    opts: Option<&StreamOptions>,
    api_name: &'static str,
    request: reqwest::RequestBuilder,
    payload: serde_json::Value,
) -> Result<reqwest::Response, ProviderError>
```

这样 provider 可以继续自己构造 URL/auth/header，但 send/hook/retry/error 先统一。

### 需要统一的行为

#### Payload hook

发送前调用：

```rust
let payload = match opts.and_then(|o| o.hooks.as_ref()) {
    Some(hooks) => hooks.apply_payload(model, payload).await?,
    None => payload,
};
```

`build_request` 当前多数返回 provider-specific typed request。为了 hook 支持，发送前应转成 `serde_json::Value`，hook 返回后再作为 JSON body 发送。这样与 TS `onPayload` 语义一致。

#### Response hook

收到 HTTP response 后，无论 status 是否 success，都应先构造 `ProviderResponseInfo` 并调用 `emit_response`。如果 hook 返回 error，则输出 error event。

建议语义：

- 网络错误：没有 response，不调用 `on_response`。
- HTTP response：调用 `on_response`，包括非 2xx。
- hook error：停止 stream，返回 error event。

#### Retry

Retry 应覆盖：

- network error。
- retryable HTTP status：408、409、429、5xx。
- provider-specific retryable usage limit 或 overloaded 文本，后续可扩展。

默认值建议参考 TS 行为：对于 OpenAI SDK 路径 TS 默认 `maxRetries: 0`，Rust 如果已经公开默认 2，需要明确做兼容决策。推荐选择：

- `StreamOptions.max_retries = None` 时默认 0，和 TS provider 显式传入 OpenAI SDK 的行为一致。
- 如果历史 Rust 行为已经依赖默认 2，则改为文档化的 crate-specific 默认，并补测试固定。

从迁移一致性角度，推荐默认 0，调用方显式传入才重试。这样可以避免 provider SDK 与上层 agent retry 叠加。

#### Retry-After

支持两种 header：

- `retry-after-ms`：毫秒。
- `retry-after`：数字秒；后续可支持 HTTP date。

如果 server 要求的 delay 大于 `max_retry_delay_ms`，不要 sleep，直接返回错误，错误中携带 requested delay 和 configured cap。

#### Cancel

取消要覆盖三个位置：

- 发请求前。
- retry sleep 期间。
- SSE stream 消费期间。

当前 `process_framework` 已经在 stream loop 中检查 cancel。transport 层还应在发送前和 retry sleep 时检查。

#### Headers

新增统一 helper：

```rust
pub fn merge_headers(
    model_headers: Option<&serde_json::Value>,
    option_headers: Option<&serde_json::Value>,
    generated_headers: impl IntoIterator<Item = (String, String)>,
) -> BTreeMap<String, String>
```

推荐优先级：

1. provider generated required headers。
2. `model.headers`。
3. `opts.headers`。

但安全相关 header 需要白名单策略。例如 `authorization`、`x-api-key`、`api-key` 是否允许用户覆盖，应按 provider 明确决定。默认不允许覆盖认证 header 更安全。

### 验收标准

- 至少 OpenAI completions 和 OpenAI responses 迁移到 shared send helper。
- `hooks.on_payload` 和 `hooks.on_response` 有离线测试。
- `max_retries` 和 `max_retry_delay_ms` 有发送路径测试，而不只是 util function 测试。
- headers 合并规则有 provider-agnostic 测试。
- `cargo test -p pi-ai` 通过。

## 阶段 2：逐步迁移 provider 到 shared transport

### 迁移顺序

推荐顺序：

1. OpenAI completions。
2. OpenAI responses。
3. Azure OpenAI responses。
4. Google Generative AI。
5. Anthropic。
6. Mistral。
7. DeepSeek。
8. Bedrock。
9. OpenAI Codex responses。
10. Images/OpenRouter。

原因：

- OpenAI completions/responses 代码相似，适合作为 shared helper 的第一个验证对象。
- Azure 可复用 responses parser，但 endpoint 和 headers 独立，适合第二批。
- Google 是普通 SSE HTTP，但 key 在 query param 中，可以验证 auth abstraction。
- Anthropic/Mistral 可以验证 header 兼容和 provider-specific request mapping。
- Bedrock/Codex 协议特殊，最后迁移，避免过早把 shared helper 设计得过重。

### 每个 provider 的迁移步骤

1. 在测试中固定当前 request body、URL、headers、error event。
2. 将 provider `mod.rs` 中的 send/status/bytes_stream 逻辑改为 shared transport helper。
3. 保留 `convert.rs` 和 `process.rs` 的行为不变。
4. 确认 missing key error event 文案和 provider/model 信息保持兼容。
5. 跑 provider-specific 测试和 `cargo test -p pi-ai`。

### 验收标准

- 所有 HTTP provider 使用同一个 send helper 或同一组 transport primitives。
- 没有 provider 自己重复实现 timeout/status/retry/hook 主逻辑，除非协议确实特殊且文档说明。
- `rg "request.send\\(\\)" crates/pi-ai/src/providers` 结果仅剩 transport helper 或特殊 provider。

## 阶段 3：收紧 options 类型边界

### 目标

在保持 serde 兼容的前提下，把 `StreamOptions` 拆成更清晰的层次。

### 推荐结构

```rust
pub struct StreamOptions {
    pub common: CommonStreamOptions,
    pub provider: ProviderOptions,
}

pub struct CommonStreamOptions {
    pub temperature: Option<f64>,
    pub max_tokens: Option<u32>,
    pub api_key: Option<String>,
    pub cache_retention: Option<CacheRetention>,
    pub thinking: Option<ThinkingConfig>,
    pub tool_choice: Option<ToolChoice>,
    pub session_id: Option<String>,
    pub headers: Option<HeaderMap>,
    pub cancel: Option<CancellationToken>,
    pub transport: TransportOptions,
    pub hooks: Option<ProviderStreamHooks>,
}

pub enum ProviderOptions {
    None,
    AzureOpenAIResponses(AzureOpenAIResponsesOptions),
    Bedrock(BedrockOptions),
    OpenAICodexResponses(OpenAICodexResponsesOptions),
    GoogleVertex(GoogleVertexOptions),
}
```

为了避免一次性破坏现有调用方，可以分两步：

#### 3A. 引入 typed helper，不改变 `StreamOptions`

新增类型：

```rust
pub enum CacheRetention {
    None,
    Short,
    Long,
}

pub enum ToolChoice {
    Auto,
    None,
    Required,
    Function { name: String },
    Raw(serde_json::Value),
}

pub struct HeaderMap(pub BTreeMap<String, String>);
```

在 `StreamOptions` 中先保留旧字段，但新增解析 helper：

```rust
impl StreamOptions {
    pub fn cache_retention_typed(&self) -> Result<Option<CacheRetention>, OptionsError>;
    pub fn tool_choice_typed(&self) -> Result<Option<ToolChoice>, OptionsError>;
    pub fn headers_typed(&self) -> Result<HeaderMap, OptionsError>;
}
```

Provider 先使用 helper，不直接解析 `serde_json::Value`。

#### 3B. 新增 builder API

提供 Rust 调用方友好的 builder：

```rust
let opts = StreamOptionsBuilder::new()
    .api_key(key)
    .max_tokens(4096)
    .tool_choice(ToolChoice::Auto)
    .azure(AzureOpenAIResponsesOptions {
        api_version: Some("2024-10-21".into()),
        deployment_name: Some("gpt-5".into()),
        ..Default::default()
    })
    .build();
```

旧 serde shape 继续支持，builder 是 Rust-native API。

### 验收标准

- Provider 内不再直接散落 `opts.as_ref().and_then(|o| o.tool_choice.clone())` 这类 untyped 解析。
- Azure/Bedrock/Codex 专属字段有明确 typed accessor。
- serde roundtrip 仍与 TS shape 兼容。
- `cargo test -p pi-ai` 通过。

## 阶段 4：引入显式 Registry

### 目标

保留现有 `register()` / `stream_model()` convenience API，同时支持显式 registry 实例，改善测试隔离和上层注入。

### 推荐 API

```rust
pub struct Registry {
    providers: RwLock<HashMap<String, Arc<dyn ApiProvider>>>,
}

impl Registry {
    pub fn new() -> Self;
    pub fn with_builtins() -> Self;
    pub fn register(&self, api: impl Into<String>, provider: Arc<dyn ApiProvider>) -> Result<(), RegistryError>;
    pub fn unregister(&self, api: &str) -> Result<(), RegistryError>;
    pub fn lookup(&self, api: &str) -> Result<Option<Arc<dyn ApiProvider>>, RegistryError>;
    pub fn stream_model(&self, model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream;
}

pub fn default_registry() -> &'static Registry;
pub fn register(api: &str, provider: Arc<dyn ApiProvider>);
pub fn stream_model(model: &Model, ctx: Context, opts: Option<StreamOptions>) -> EventStream;
```

### 迁移策略

1. 新增 `Registry` struct。
2. 现有全局 `REGISTRY` 改为 `DEFAULT_REGISTRY: LazyLock<Registry>`。
3. 保留现有函数，内部转发到 `default_registry()`。
4. 测试逐步改用 `Registry::new()`，避免互相污染。
5. `providers::register_builtins()` 改成：

```rust
pub fn register_builtins(registry: &Registry) -> Result<(), RegistryError>;
pub fn register_builtins_global();
```

为兼容现有调用，也可以暂时保留无参 `register_builtins()`，内部注册到 default registry。

### 验收标准

- 新测试不需要 `unregister()` 清理全局状态。
- 锁 poisoned 不 panic，而是返回 error event 或 `RegistryError`。
- 现有 `pi_ai::register` 和 `pi_ai::stream_model` 调用方不破。
- `cargo test -p pi-ai` 通过。

## 阶段 5：结构化错误和 diagnostics

### 目标

让 provider error 可以被上层可靠分类，同时保持 event stream 的兼容输出。

### 推荐类型

```rust
pub enum ProviderErrorKind {
    MissingCredentials,
    InvalidOptions,
    Network,
    Timeout,
    Cancelled,
    HttpStatus,
    RetryAfterTooLong,
    HookFailed,
    StreamParse,
    Registry,
}

pub struct ProviderError {
    pub kind: ProviderErrorKind,
    pub api: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<u16>,
    pub message: String,
    pub body: Option<String>,
    pub retry_after_ms: Option<u64>,
    pub source: Option<String>,
}
```

转换成 `AssistantMessageEvent::Error` 时：

- `message.error_message` 保留人类可读 message。
- `message.stop_reason = StopReason::Error` 或 `StopReason::Aborted`。
- `message.diagnostics` 附加结构化 details。

### 推荐 diagnostics details

```json
{
  "kind": "HttpStatus",
  "api": "openai-responses",
  "provider": "openai",
  "model": "gpt-5",
  "status": 429,
  "retryAfterMs": 120000,
  "bodyPreview": "..."
}
```

### 验收标准

- transport 层返回 `ProviderError`，provider 不再到处手写 HTTP error string。
- `complete()` 可以先保留 `Result<AssistantMessage, String>`，但新增 `complete_result()` 返回 typed error。
- 至少 missing credentials、timeout、HTTP status、hook failed、cancelled 有测试。

## 阶段 6：事件快照成本优化

### 目标

在不破坏 TS-compatible event shape 的前提下，减少长输出场景下的 clone 成本。

### 推荐步骤

#### 6A. 先测量

新增 benchmark 或 stress test：

- 10,000 个 text delta。
- 1 MB tool call arguments delta。
- 多个并行 tool calls。

度量：

- 总耗时。
- peak allocation。
- event count。

Rust workspace 当前没有 benchmark 框架时，可以先用 ignored test 或 example 做本地测量，不作为 CI 必跑项。

#### 6B. 抽出 event builder

新增内部 helper：

```rust
pub struct AssistantEventBuilder {
    partial: AssistantMessage,
}

impl AssistantEventBuilder {
    pub fn text_delta(&mut self, content_index: u32, delta: &str) -> AssistantMessageEvent;
    pub fn toolcall_delta(&mut self, content_index: u32, delta: &str) -> AssistantMessageEvent;
    pub fn done(self) -> AssistantMessageEvent;
}
```

第一版仍然 clone partial，但把 clone 点集中起来。

#### 6C. 可选兼容层

内部处理器可以先产生 lightweight event：

```rust
pub enum InternalStreamEvent {
    Start,
    TextDelta { content_index: u32, delta: String },
    ToolcallDelta { content_index: u32, delta: String },
    Done,
}
```

再由兼容 adapter 转成现有 `AssistantMessageEvent`。这样如果未来上层 Rust crate 愿意消费 lightweight events，可以新增 API，不必破坏旧 API。

### 验收标准

- 现有 serde event shape 不变。
- 长 delta stress test 有基线和优化后数据。
- process handlers 中直接 `partial.clone()` 的位置显著减少或集中到 builder。

## 阶段 7：公共 API 暴露面收敛

### 问题

当前 `lib.rs` 暴露：

```rust
pub mod compat;
pub mod images;
pub mod models;
pub mod providers;
pub mod registry;
pub mod stream;
pub mod types;
pub mod util;
```

这让 provider 内部 `wire` schema、process handler、compat heuristic、utility helper 都可能成为外部依赖。迁移期方便测试，但长期会限制重构。

### 推荐策略

不要立即改成 private，先分层：

- `pub mod providers` 保留，但明确哪些 provider constructor 是稳定 API。
- `providers::<name>::wire` 和 `providers::<name>::process` 逐步改为 `pub(crate)`。
- 测试需要访问 request builder 时，优先暴露 provider-level `build_request_for_test` 或将测试移到模块内。
- `util` 下只保留真正公共的 helper；内部 transport/helper 移到 `pub(crate)`。

### 文档约定

在 crate-level docs 中明确：

- 稳定公共 API：types、model lookup、stream/complete、registry、built-in provider constructors、faux provider。
- 内部 API：wire schema、process handlers、transport internals、compat detection details。

### 验收标准

- `cargo doc -p pi-ai --no-deps` 能清楚展示公共 API。
- 下游 crate 不需要依赖 provider wire schema。
- 测试仍能覆盖 request mapping 和 stream parsing。

## 推荐优先级

### P0：马上值得做

1. 新增 transport contract tests。
2. 新增 shared send helper。
3. OpenAI completions/responses 迁移到 shared send helper。
4. hook、timeout、retry、response hook 走同一条路径。

理由：这是收益最大、风险最低的改造。它不改变 provider request/stream 语义，却能消除最危险的不一致。

### P1：完成 provider 横切一致性

1. Azure、Google、Anthropic、Mistral、DeepSeek 迁移。
2. 统一 headers merge。
3. 统一 missing credentials 和 HTTP error 构造。
4. retry/cancel semantics 覆盖所有普通 HTTP provider。

理由：完成后，新增 provider 的成本会明显下降。

### P2：类型边界和 registry

1. typed options helper。
2. `Registry` struct。
3. 全局 registry 变成 default convenience API。
4. provider-specific options 从 `StreamOptions` 主体中逐步抽出。

理由：这是长期 API 质量的核心，但改动范围比 transport 更大，适合在横切发送路径稳定后推进。

### P3：错误、事件、public API

1. `ProviderError` 和 diagnostics 主路径。
2. `complete_result()` typed error API。
3. event builder 和 lightweight internal events。
4. 收敛 `pub mod` 暴露面。

理由：这些会影响上层 crate 的使用方式，需要更明确的迁移窗口。

## 具体实施计划草案

下面是可拆成后续 implementation plan 的任务序列。每个任务都应小步提交，并在提交前至少运行对应 provider 测试。

### Task 1：补 transport contract 测试

涉及文件：

- 新增 `crates/pi-ai/tests/transport_contract.rs`
- 可能新增测试辅助模块 `crates/pi-ai/tests/support/http_server.rs`

内容：

- 用本地 mock HTTP server 或 `reqwest` test transport 替代真实 provider。
- 固定 hook、retry、timeout、headers、non-success status 行为。
- 如果当前行为尚未实现，先写针对 shared transport helper 的单元测试，不直接挂到 provider。

验证：

```bash
cargo test -p pi-ai transport_contract
```

### Task 2：新增 `transport` 模块

涉及文件：

- 新增 `crates/pi-ai/src/transport/mod.rs`
- 新增 `crates/pi-ai/src/transport/headers.rs`
- 新增 `crates/pi-ai/src/transport/retry.rs`
- 新增 `crates/pi-ai/src/transport/error.rs`
- 修改 `crates/pi-ai/src/lib.rs`
- 修改 `crates/pi-ai/src/util/http.rs`

内容：

- 搬迁或 re-export retry helper。
- 新增 headers merge helper。
- 新增 `ProviderError`。
- 新增 shared `send_json_stream` helper。

验证：

```bash
cargo test -p pi-ai http_retry transport_contract
```

### Task 3：迁移 OpenAI completions

涉及文件：

- 修改 `crates/pi-ai/src/providers/openai/completions/mod.rs`
- 扩展 `crates/pi-ai/tests/openai_completions.rs`

内容：

- `build_request` 保持不变。
- 发送路径改为 shared helper。
- 补 payload hook 和 response hook 测试。
- 补 retryable status 测试。

验证：

```bash
cargo test -p pi-ai openai_completions
```

### Task 4：迁移 OpenAI responses

涉及文件：

- 修改 `crates/pi-ai/src/providers/openai/responses/mod.rs`
- 扩展 `crates/pi-ai/tests/openai_responses.rs`

内容：

- 复用 Task 3 的 shared helper。
- 确认 `prompt_cache_key`、tool call streaming、response id 行为不变。

验证：

```bash
cargo test -p pi-ai openai_responses
```

### Task 5：迁移 Azure 和 Google

涉及文件：

- 修改 `crates/pi-ai/src/providers/azure_openai_responses.rs`
- 修改 `crates/pi-ai/src/providers/google/mod.rs`
- 扩展 `crates/pi-ai/tests/azure_openai_responses.rs`
- 扩展 `crates/pi-ai/tests/google_generative_ai.rs`

内容：

- Azure 验证 endpoint resolution、api-key header、deployment name 不变。
- Google 验证 query param key 和 SSE parsing 不变。

验证：

```bash
cargo test -p pi-ai azure_openai_responses google_generative_ai
```

### Task 6：迁移 Anthropic、Mistral、DeepSeek

涉及文件：

- 修改 `crates/pi-ai/src/providers/anthropic/mod.rs`
- 修改 `crates/pi-ai/src/providers/mistral/mod.rs`
- 修改 `crates/pi-ai/src/providers/deepseek/mod.rs`
- 扩展对应 tests。

内容：

- Anthropic 验证 `anthropic-version`、cache control、thinking/tool use stream 不变。
- Mistral 验证 `x-affinity` 和 explicit header 优先级不变。
- DeepSeek 验证 reasoning/text mapping 不变。

验证：

```bash
cargo test -p pi-ai anthropic_mapping mistral deepseek
```

### Task 7：typed options helper

涉及文件：

- 修改 `crates/pi-ai/src/types/stream_opts.rs`
- 可能新增 `crates/pi-ai/src/types/options.rs`
- 扩展 `crates/pi-ai/tests/serde_roundtrip.rs`
- 新增或扩展 `crates/pi-ai/tests/provider_options_contract.rs`

内容：

- 新增 `CacheRetention`、`ToolChoice`、`HeaderMap`。
- 为旧字段提供 typed accessor。
- Provider 先使用 typed accessor，不改 serde shape。

验证：

```bash
cargo test -p pi-ai serde_roundtrip provider_options_contract request_building
```

### Task 8：显式 Registry

涉及文件：

- 修改 `crates/pi-ai/src/registry.rs`
- 修改 `crates/pi-ai/src/providers/mod.rs`
- 扩展 `crates/pi-ai/tests/model_registry.rs`
- 新增 `crates/pi-ai/tests/registry_contract.rs`

内容：

- 新增 `Registry` struct。
- 现有全局函数转发到 default registry。
- 测试从全局注册切换到 instance registry。

验证：

```bash
cargo test -p pi-ai registry model_registry
```

### Task 9：结构化错误和 diagnostics 主路径

涉及文件：

- 修改 `crates/pi-ai/src/transport/error.rs`
- 修改 `crates/pi-ai/src/stream.rs`
- 修改 `crates/pi-ai/src/util/diagnostics.rs`
- 扩展 provider error tests。

内容：

- 新增 `complete_result()` typed error API。
- `AssistantMessageEvent::Error` 自动附带 diagnostics。
- HTTP status、timeout、cancelled、missing credentials、hook failed 分类稳定。

验证：

```bash
cargo test -p pi-ai m8_utilities transport_contract
```

### Task 10：事件 builder 和 public API 收敛

涉及文件：

- 新增 `crates/pi-ai/src/events/builder.rs` 或 `crates/pi-ai/src/stream/event_builder.rs`
- 修改各 provider `process.rs`
- 修改 `crates/pi-ai/src/lib.rs`
- 调整模块可见性。

内容：

- 集中 event snapshot 生成。
- 保持 `AssistantMessageEvent` serde shape 不变。
- 把内部 `wire/process` 可见性降到 `pub(crate)`，测试按需迁移。

验证：

```bash
cargo test -p pi-ai
cargo doc -p pi-ai --no-deps
```

## 测试策略

### 必跑检查

每个 task 完成后至少运行：

```bash
cargo test -p pi-ai
```

涉及公共 API 或跨 crate 使用时运行：

```bash
cargo test --workspace
cargo check --workspace
```

格式检查：

```bash
cargo fmt --check
```

### 测试原则

- 离线优先，不需要真实 API key。
- request body 用 JSON snapshot 或结构化断言，不做脆弱字符串匹配。
- stream parsing 用 fixture SSE/AWS event-stream bytes。
- retry/timeout/cancel 用 mock server 或 deterministic stream，不依赖真实时间长等待。
- TS parity 相关行为在测试名里明确标注。

## 兼容策略

### 对 Rust 调用方

- 保留 `pi_ai::stream_model`、`pi_ai::complete`、`pi_ai::register`。
- 保留现有 `StreamOptions` 字段，先新增 typed helper 和 builder。
- 新 API 使用 additive migration，不在第一轮删除旧字段。

### 对 serde / TS shape

- `Model`、`Message`、`ContentBlock`、`AssistantMessageEvent` 的 serde 输出保持兼容。
- `cacheRetention`、`toolChoice` 等旧 JSON 形态继续能反序列化。
- 新 typed enum 需要能 roundtrip 到现有 JSON shape。

### 对 provider behavior

- Missing key error event 保持可读。
- 默认 retry 行为需要明确一次并写测试。推荐默认 0，以贴近 TS provider 使用 OpenAI SDK 时的显式设置。
- Header 覆盖规则需要文档化，避免用户意外覆盖认证 header。

## 风险和缓解

### 风险：shared transport 过度抽象

缓解：

- 第一版只抽 `send_json_stream`，不要一次性设计完整 provider DSL。
- Bedrock/Codex 暂时保留特殊路径，普通 HTTP provider 稳定后再迁移。

### 风险：typed options 破坏 serde 兼容

缓解：

- 先新增 typed accessor，不删除旧字段。
- 用 `serde_roundtrip` 和 TS fixture 形状测试保护。

### 风险：registry 改造影响上层 crate

缓解：

- 全局 API 保持不变。
- 显式 `Registry` 作为 additive API。
- 内部测试优先迁移，外部调用方后迁移。

### 风险：错误类型引入后 event 语义漂移

缓解：

- `AssistantMessageEvent::Error` shape 不变。
- typed error 先作为 internal source 和 `complete_result()` 新 API。
- `error_message` 继续保留人类可读字符串。

### 风险：事件 builder 改动 provider process 逻辑

缓解：

- 先集中 clone 逻辑，不改 event 内容。
- 每个 provider stream fixture 测试必须逐个通过。
- 长输出优化放在兼容层之后。

## 成功标准

完成 P0/P1 后应达到：

- 所有普通 HTTP provider 的 send/status/timeout/retry/hook/header/error 行为来自 shared transport。
- `max_retries`、`max_retry_delay_ms`、`timeout_ms`、`hooks` 在已迁移 provider 中真实生效。
- `model.headers` 与 `opts.headers` 合并规则一致并有测试。
- `rg "request.send\\(\\)" crates/pi-ai/src/providers` 只剩 transport helper 或明确特殊 provider。
- `cargo test -p pi-ai` 通过。

完成 P2/P3 后应达到：

- Rust 调用方有 typed options builder。
- Provider-specific options 不再继续污染全局 `StreamOptions` 主体。
- 上层可以使用显式 `Registry` 注入 provider。
- Provider error 有结构化分类和 diagnostics。
- 公共 API 文档清楚区分 stable API 与 internal module。
- `cargo test --workspace` 和 `cargo doc -p pi-ai --no-deps` 通过。

## 推荐下一步

建议先拆出一份实施计划，范围只覆盖 P0：

1. transport contract tests。
2. shared transport module。
3. OpenAI completions 迁移。
4. OpenAI responses 迁移。

这个范围足够小，可以在不影响 Bedrock/Codex 等复杂 provider 的情况下验证抽象是否合适。P0 完成后，再根据实际代码形态决定 P1 的 provider 批量迁移方式。
