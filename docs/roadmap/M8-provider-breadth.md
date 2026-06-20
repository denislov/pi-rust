# M8 — pi-ai provider 广度 + 认证

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)（auth 存储）· 解锁：`/login` 等
> 定位：**核心**。补齐 `pi-ai` 的 provider 广度与 API-key 之外的认证，让模型表里的模型真正可用。

## 目标
把 `pi-ai` 从"5 provider + 仅 API-key"推进到对标 pi 的 provider/认证广度。

## 实际推进状态

M8 已按离线可验证路径推进完成。当前落地范围：

- Mistral、Azure OpenAI Responses、AWS Bedrock、OpenAI Codex Responses 已接入 `pi-ai` provider registry。
- API-key、环境变量、`auth.toml` OAuth bearer token 均可进入 provider `StreamOptions.api_key` 路径。
- Bedrock 已支持 SigV4 / bearer token 认证路径。
- Cloudflare gateway、Copilot 动态头、PKCE/OAuth HTML 工具、diagnostics、short hash、OpenRouter images 已有 Rust 实现和测试。
- `Model.compat` / `thinkingLevelMap` 已从不透明 `serde_json::Value` 收敛为强类型 Rust 结构，同时保持序列化为 TS 兼容对象形态。
- 完整 provider 登录交互（浏览器 callback / device-code / `/login` UI）仍由后续 CLI/TUI auth flow 里程碑承接；M8 侧已提供 provider token 消费、OAuth token 存储和可复用 OAuth 工具。

## 待实现项（按建议顺序）

### 1. 补 4 个 provider
| Provider | TS 参考 | 难点 |
|---|---|---|
| Mistral | `ai/src/providers/mistral.ts`（633 行） | ✅ Rust provider 已接入；REST，reasoning effort / tool choice / `x-affinity` 头 |
| Azure OpenAI Responses | `ai/src/providers/azure-openai-responses.ts`（291 行） | ✅ Rust provider 已接入；REST，独立 endpoint 处理 |
| AWS Bedrock | `ai/src/providers/amazon-bedrock.ts`（1019 行） | ✅ Rust provider 已接入；SigV4 / bearer token、cache point、thinking、AWS event-stream 解析 |
| OpenAI Codex Responses | `ai/src/providers/openai-codex-responses.ts`（1488 行） | ✅ Rust provider 已接入；Codex request/headers/cache key、WebSocket frame mock、HTTP SSE fallback |
> 顺序：先 REST（Mistral/Azure），再 Bedrock（SigV4），最后 Codex（WebSocket）。

### 2. API-key 之外的认证
- ✅ OAuth 工具基础：PKCE challenge + OAuth 成功/失败 HTML 页（`utils/oauth/pkce.ts`、`oauth-page.ts` 对应）。
- ✅ Bedrock：SigV4 + `AWS_BEARER_TOKEN_BEDROCK` + env AWS 凭证（`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` / `AWS_SESSION_TOKEN`）。
- ✅ Cloudflare 网关：baseURL `{VAR}` 占位符替换（`ai/src/providers/cloudflare.ts` 对应）。
- ✅ Copilot 专属头：`inferCopilotInitiator` / `hasCopilotVisionInput` / 动态头（`github-copilot-headers.ts` 对应）。
- ✅ `auth.toml` OAuth bearer token 消费：`type = "oauth"` 支持 `access` / `access_token`，解析优先级仍为 `--api-key` > env > `auth.toml`。
- ✅ `auth.toml` OAuth token 持久化：支持 `AuthStore::set_oauth_access_token()` + TOML round-trip，Unix 保存权限仍为 `0600`。
- ⏭️ Provider OAuth 交互流（Anthropic Pro/Max、GitHub Copilot device-code、OpenAI Codex 浏览器/device-code）需要 CLI/TUI `/login` 入口、浏览器打开和回调服务编排；M8 侧已完成可复用工具、存储格式和 provider 侧 token/header 消费。

### 3. 兼容 / 路由矩阵（消除不透明 `compat`）
- ✅ 把 `Model.compat: Option<serde_json::Value>` 升级为**强类型** compat 层：
  `OpenAICompletionsCompat`/`OpenAIResponsesCompat`/`AnthropicMessagesCompat`（`ai/src/types.ts:373-425`）。
- ✅ OpenRouter 路由（含 snake_case 与兼容 camelCase alias）、Vercel AI Gateway 路由。
- ✅ `thinkingFormat`（openai/openrouter/deepseek/zai/qwen…）、`thinkingLevelMap` 强类型化。

### 4. 协议补强
- ✅ **session-affinity 头注入**：Azure/Mistral/Codex 覆盖 affinity 头与 `prompt_cache_key`，OpenAI Responses 复用 `prompt_cache_key`。
- ✅ prompt cache：Anthropic 既有 `cache_control` TTL 标记保留，OpenAI/Azure Responses 支持 `prompt_cache_key` / retention 字段。

### 5. 图像生成 + diagnostics
- ✅ 图像生成 API：`ai/src/images.ts` 对应 Rust types + OpenRouter 图像 request/response helper。
- ✅ 结构化 diagnostics：`ai/src/utils/diagnostics.ts` 对应 Rust helper，`AssistantMessage` 支持结构化 diagnostics。
- ✅ 内容签名 hash：`ai/src/utils/hash.ts` 对应 `short_hash`。

## 验收 / 测试（离线优先）
- ✅ 每个 provider 用 **faux/fixture** 做请求体与流式解析断言，**不**用真实 key。
- ✅ SigV4 用固定向量做签名单测；Bedrock 用 mock AWS event-stream 帧；Codex 用 mock WebSocket frame。
- ✅ compat 矩阵用表驱动/fixture 单测覆盖关键标志组合。
- ✅ 全局注册表用唯一 api id 隔离（见 [cross-cutting.md](cross-cutting.md) 风险项）。

## 本轮落地
- `pi-ai` 新增/补强：Mistral、Azure OpenAI Responses、AWS Bedrock、OpenAI Codex Responses、Cloudflare、Copilot headers、OpenRouter images。
- `pi-coding-agent` auth 补强：OAuth bearer token 读取/写入，供 M8 provider 作为 bearer token 消费。
- 工具与类型：PKCE/OAuth pages、diagnostics、content hash、typed compat、typed thinking level map、image generation structs。
- 已验证：`cargo test -p pi-ai`、`cargo test -p pi-coding-agent config::auth::tests` 通过；最终 workspace 验证见提交记录。
