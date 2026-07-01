**当前定位更新（2026-07-02）**

这份文档保留为 `pi-ai` 早期对照 TypeScript `pi/packages/ai` 的历史评估。它不再是 TS parity checklist，也不再定义 `pi-rust` 的推进目标。

当前项目方向以 Flow-centered runtime 为准：TypeScript `pi` 仍是 provider 行为、wire protocol、产品体验和测试 fixture 的参考，但 `pi-rust` 不追求成为 `@earendil-works/pi-ai` 的同构替代。`pi-ai` 后续应服务于 Rust-native 产品运行时：提供模型目录、provider request/response/streaming、scoped provider runtime、auth resolution、transport hooks 和 provider capability 信息；不得反向依赖 `CodingAgentSession`、session log、CLI/RPC/TUI 或产品 Flow。

因此，本文中仍值得推进的主线是：

- 用 Rust-native scoped `AiClient` / provider runtime 取代“全局 registry 是主运行时”的默认形态；
- 建立统一 provider auth resolver，集中处理 API key、bearer token、OAuth access/refresh、headers、base URL 和 auth source；
- 明确 global `register()` / `stream_model()` 的兼容边界，避免继续扩大事实公共 API；
- 为模型目录与内置 provider 注册关系增加 invariant 规划，避免 catalog 暴露不可运行模型或注册无 catalog 来源的 API；
- 保持 provider wire JSON 与上游协议一致，用 serde/fixture/offline tests 守护。

本文中不再作为目标推进的内容是：TS session/config/auth 兼容、TS `Models` 原样移植、TS SDK root export parity、按 TS provider 数量机械补齐 provider、以及扩大 TS `compat.ts` 风格的全局 lazy API。

**结论**

`pi-rust/crates/pi-ai` 已经不是空壳，核心 chat 数据结构、静态模型目录、主要 API 的请求/流式解析、重试/headers/hooks、faux provider 和一批离线测试都已落地；但它还没有达到 TS `pi/packages/ai` 的完整产品级边界。当前更像“Rust 运行时 PoC/核心子集”，不是 `@earendil-works/pi-ai` 的等价替代。

我按当前代码判断：chat 核心协议完成度中等偏高，provider 运行时完成度中等，auth/OAuth/动态 provider/images/compat 公共面完成度偏低。`cargo test -p pi-ai` 已通过：70 个单元测试 + 所有集成测试均通过。

**功能完整度**

| 维度 | TS `packages/ai` | Rust `pi-ai` | 评估 |
|---|---|---|---|
| 核心类型 | `KnownApi`/`KnownProvider`、`StreamOptions`、`Usage`、事件协议完整，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/types.ts:15) | 基本类型已移植，见 [types/mod.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/types/mod.rs:1) | 形状大体对齐，但字段有缺口 |
| Chat API | TS 有 9 个：含 `google-vertex`，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/types.ts:15) | Rust 模型目录有 8 个，少 `google-vertex`；内置注册另有 `deepseek-chat-completions`，见 [providers/mod.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/providers/mod.rs:20) | 主要路径已覆盖，Vertex 缺失，DeepSeek API 命名需收敛 |
| Provider 数量 | 35 个 chat provider，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/types.ts:32) | 31 个 provider 出现在 Rust 模型目录；少 `ant-ling`、`google-vertex`、`nvidia`、`zai-coding-cn` | 目录覆盖约九成，但不是全量 |
| 模型目录 | 约 1019 个 chat 模型；另有 37 个 image 模型 | 921 个 chat 模型，见 [models.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/models.rs:39)；未看到 image 模型目录 | chat 接近但不完整；images 明显不足 |
| Models 运行时 | TS `Models` 负责 provider 集合、auth、刷新、stream/complete，见 [models.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/models.ts:79) | Rust 是全局 registry + 静态 catalog，见 [registry.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/registry.rs:13) | Rust 缺实例化集合、动态刷新和 auth 生命周期 |
| Provider 创建 | TS `createProvider()` 支持单 API/多 API、动态刷新、typed stream，见 [models.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/models.ts:323) | Rust 每个 API 实现 `ApiProvider` trait，见 [registry.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/registry.rs:9) | Rust 简洁，但表达力少 |
| Auth/OAuth | TS 有 `CredentialStore`、`ProviderAuth`、OAuth refresh、env/baseUrl/header/env 合并，见 [auth/types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/auth/types.ts:30) | Rust 主要是 env key + 少量 Bedrock/Codex 辅助；没有等价 credential store/OAuth runtime | 这是最大缺口之一 |
| 图片生成 | TS 有 `ImagesModels`、image provider、auth 合并、`generateImages`，见 [images-models.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/images-models.ts:49) | Rust 有 image 类型和 OpenRouter request/response 转换，见 [images.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/images.rs:34)、[openrouter.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/providers/images/openrouter.rs:16) | 数据模型/转换完成，运行时不完整 |
| Compat/global API | TS `compat.ts` 保留旧全局 API、registry、lazy API、faux，见 [compat.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/compat.ts:1) | Rust `compat` 主要是兼容配置类型；全局 `stream_model` 是新形态 | 语义不等价 |
| 测试 | TS 有约 94 个 test 文件，覆盖大量 provider 特例/OAuth/images/e2e guard | Rust 有 25 个 test 文件，覆盖核心 provider 转换、SSE、retry、fixtures、faux | Rust 已实现部分测试质量不错，但覆盖面少 |

**关键差距**

1. **TS 的核心抽象是 `Models`/`Provider`，Rust 的核心抽象是全局 `REGISTRY`。**
   TS 中 `Models` 是可实例化对象，持有 provider、credential store、auth context，并负责 `getAuth()`、`refresh()`、`stream()`、`complete()`，见 [models.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/models.ts:79)。Rust 目前是全局 `LazyLock<RwLock<HashMap<...>>>`，通过 `stream_model()` 按 `model.api` dispatch，见 [registry.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/registry.rs:13)。这对简单调用方便，但对多实例配置、测试隔离、不同 credential store、插件卸载和并发环境不够干净。

2. **auth 语义没有移植完整。**
   TS 的 auth 是一等职责：stored API key、OAuth credential、refresh、auth source、baseUrl/header/env 合并、错误分类都在模型运行时里。Rust 目前多数 provider 自己读 env key，`stream_model()` 也会注入 env key，见 [registry.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/registry.rs:31)。这会导致职责分散：registry、provider、util 都在处理 auth 的一部分，但没有统一的 `AuthResult`/`ProviderAuth` 等价物。

3. **事件和 usage 类型不是完全等价。**
   TS `Usage` 有 `reasoning?`、`cacheWrite1h?`、`cost.total`，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/types.ts:352)。Rust `Usage` 少这些字段，见 [usage.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/types/usage.rs:14)。TS 的 `text_end` 携带 `content`，`toolcall_end` 携带 `toolCall`，见 [types.ts](/home/whai/dev_wkspace/pi2rust/pi/packages/ai/src/types.ts:453)；Rust 事件终止事件只带 `partial`，见 [event.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/types/event.rs:28)。如果上层期望 TS 事件协议，这些是实质兼容风险。

4. **Provider 层实现不一致。**
   Rust 已有不错的通用 `transport::http`，支持 hooks、retry、timeout、headers 和取消；OpenAI Responses/Completions 已使用该路径。但 Anthropic、DeepSeek 等仍有自己的发送/错误处理逻辑。结果是同类功能在不同 provider 上行为可能不一致，例如 retry、hooks、response headers、timeout 的覆盖面不一样。

5. **模型目录接近，但生成策略和形状不同。**
   TS 是按 provider 分组的 `MODELS` 对象，并能 `getBuiltinModel(provider, id)`；Rust 是 flat `Vec<Model>`，再按 provider/id 扫描，见 [models.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/models.rs:6)。这在 Rust 里可用，但语义上丢了“provider 为第一维”的强约束，也更难做 typed provider catalog。

6. **DeepSeek API 命名存在边界混乱。**
   Rust 注册了 `deepseek-chat-completions`，见 [model_registry.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/tests/model_registry.rs:126)，但 Rust 生成模型中的 DeepSeek 模型实际 `api` 是 `openai-completions`。这意味着内置 DeepSeekProvider 不是当前目录模型的默认路径。建议后续明确：DeepSeek 是作为 OpenAI-compatible provider 处理，还是保留独立 API；不要两套并存暴露为公共契约。

**设计合理性**

好的部分：

- Rust 类型按 TS JSON 形状做了大量 serde 对齐，命名和序列化方向正确。
- provider 转换/处理拆为 `convert`、`wire`、`process`，例如 Anthropic/OpenAI/Google/Mistral/Bedrock 都有相对清晰的模块结构。
- `transport::http` 抽出的 retry、hooks、timeout、header merge 是正确方向。
- 离线 fixtures 和 faux provider 让测试不依赖真实模型 key，符合迁移要求。
- `Cargo.toml` 依赖克制，主要是 `tokio/futures/reqwest/serde`，没有过早引入大 SDK。

问题部分：

- `lib.rs` 直接 `pub mod` 暴露所有内部模块，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/lib.rs:1)。这会把尚未稳定的内部结构变成事实公共 API。
- `Model`、`StreamOptions`、`Usage` 等 public struct 全字段公开，且没有 `#[non_exhaustive]` 或 builder；后续补 TS 字段会造成破坏性变更。
- registry 是全局可变状态，缺少 source id、scope、reset/override 策略，测试和长期运行进程里容易互相影响。
- auth、headers、env、base_url 解析分散在 provider 和 util 中，没有 TS 那种中心化 resolver。
- image side 和 chat side 没有对等架构，后续若直接补 HTTP 函数，容易形成第二套不一致运行时。

**职责边界清晰度**

TS 边界较清晰：

- `types.ts`：协议类型。
- `models.ts`：provider 集合、auth 应用、stream/complete。
- `providers/*`：provider 元数据、模型目录、auth 声明。
- `api/*`：具体 API wire protocol。
- `auth/*` 和 `utils/oauth/*`：credential/OAuth。
- `compat.ts`：旧全局 API 兼容层。
- `images-models.ts`/`images.ts`：图片侧对等 runtime。

Rust 当前边界是“有雏形但未完全分层”：

- `types`、`models`、`providers`、`transport` 分层方向合理。
- `registry` 同时承担 provider dispatch、全局生命周期、env API key 注入、unknown api error，这些职责偏多。
- provider 模块同时负责 auth 解析、request 构造、HTTP 发送、错误归一、stream process；部分 provider 已借助 `transport::http`，但未统一。
- `compat` 只是 compat config 类型，不是 TS `compat.ts` 的兼容运行层，命名容易误导。

**公共接口稳定性**

当前 Rust 公共接口稳定性我评为 **低到中**。

原因：

- crate 版本仍是 `0.1.0`，且 `lib.rs` 暴露面很宽。
- `pub mod providers`、`pub mod transport`、`pub mod util` 等把内部实现细节公开了。
- public struct 字段直接暴露，后续补齐 TS 字段、改字段类型或隐藏内部配置都会破坏下游。
- 事件协议和 TS 不完全一致，若现在被 `pi-agent-core`/`pi-tui` 依赖，会把不完整协议固化。
- 全局 `register()`/`stream_model()` 作为顶层 re-export，见 [lib.rs](/home/whai/dev_wkspace/pi2rust/pi-rust/crates/pi-ai/src/lib.rs:11)，一旦后续改成实例化 `Models`，迁移成本会比较高。

**建议优先级（按当前 Flow-centered 方向重述）**

1. **先定 Rust-native scoped provider runtime 和公共 API 边界。**
   建议新增 `AiClient` 或同等 scoped runtime 类型，承载 provider collection、model catalog view、auth resolver、stream/complete。保留 global registry 作为兼容、测试或启动期辅助，而不是长期主 API。该 runtime 只属于 `pi-ai` provider/model 层，不接触 `CodingAgentSession`、session persistence、CLI/RPC/TUI 或 product Flow。

2. **建立 Rust-native provider auth resolver。**
   目标不是原样移植 TS `CredentialStore`，而是在 Rust-native `auth.toml`、env、CLI override 和 provider-specific auth 之间定义统一解析边界。优先规划 `ProviderAuth` / `AuthResolution` / auth source / header merge / base URL override / OAuth access-refresh 生命周期；OAuth 交互流可以分阶段，但 provider runtime 的接口边界应先定下来。

3. **收敛事件和 usage 协议。**
   补齐 `Usage.reasoning`、`cacheWrite1h`、`cost.total`，并决定 Rust `AssistantMessageEvent` 是否严格兼容 TS 的 `text_end.content`、`toolcall_end.toolCall`。这应在更多上层 crate 依赖前完成。

4. **统一 provider transport。**
   让 Anthropic、DeepSeek、Google、Mistral、Bedrock 尽量走同一套发送/错误/hook/retry/timeout框架。provider 应只负责 request/response protocol，而不是重复 HTTP 控制流。

5. **清理 API/provider 命名偏差，并增加 catalog/register invariant。**
   明确 `deepseek-chat-completions` 是否保留、隐藏或删除；只有在产品需要或 catalog 已暴露不可运行模型时才补 `google-vertex` 等 provider。模型目录和内置注册表应互相校验，避免有模型无 provider 或有 provider 无模型。

6. **把图片侧做成 chat 侧对等架构。**
   Rust 现在只有类型和 OpenRouter 转换，下一步应补 image model catalog、image provider trait、auth 合并和 `generate_images()` runtime，而不是只加一个 ad hoc HTTP 函数。

7. **缩窄 public exports。**
   `lib.rs` 应只 re-export 稳定入口和核心类型；内部模块可以先 `pub(crate)` 或保留但标记 unstable。对 public structs 考虑 builder、constructor 或 `#[non_exhaustive]`。

**总体判断**

`pi-ai` 的 Rust 移植目前最强的是“核心 chat streaming 协议 + 主要 provider 的离线可测转换/解析 + 静态模型目录”。它已经适合支撑 Rust PoC 和部分上层集成，但还不适合作为 TS `@earendil-works/pi-ai` 的完整替代。

下一阶段最应该补的不是更多 provider，而是先把 `Models/Auth/Public API` 这条主干定稳。否则 provider 越补越多，后面再从全局 registry/auth 分散实现迁移到 TS 对等架构，改动面会明显变大。
