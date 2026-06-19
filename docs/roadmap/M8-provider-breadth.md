# M8 — pi-ai provider 广度 + 认证

> 返回索引：[../../ROADMAP.md](../../ROADMAP.md) · 依赖：[M7](M7-config-auth.md)（auth 存储）· 解锁：`/login` 等
> 定位：**核心**。补齐 `pi-ai` 的 provider 广度与 API-key 之外的认证，让模型表里的模型真正可用。

## 目标
把 `pi-ai` 从"5 provider + 仅 API-key"推进到对标 pi 的 provider/认证广度。

## 待实现项（按建议顺序）

### 1. 补 4 个 provider
| Provider | TS 参考 | 难点 |
|---|---|---|
| Mistral | `ai/src/providers/mistral.ts`（633 行） | REST，reasoning effort / tool choice / `x-affinity` 头 |
| Azure OpenAI Responses | `ai/src/providers/azure-openai-responses.ts`（291 行） | REST，独立 endpoint 处理 |
| AWS Bedrock | `ai/src/providers/amazon-bedrock.ts`（1019 行） | **SigV4** 签名、cache point、thinking |
| OpenAI Codex Responses | `ai/src/providers/openai-codex-responses.ts`（1488 行） | **WebSocket** 协议 |
> 顺序：先 REST（Mistral/Azure），再 Bedrock（SigV4），最后 Codex（WebSocket）。

### 2. API-key 之外的认证
- OAuth：Anthropic（Pro/Max，PKCE + 回调 server）、GitHub Copilot（device-code 兜底）、OpenAI Codex（浏览器/device-code）。TS：`ai/src/utils/oauth/`（8 文件）。
- Bedrock：SigV4 + `AWS_BEARER_TOKEN_BEDROCK` + AWS 凭证链（profile/env/容器/IRSA）。
- Cloudflare 网关：baseURL `{VAR}` 占位符替换（`ai/src/providers/cloudflare.ts`）。
- Copilot 专属头：`inferCopilotInitiator` / `hasCopilotVisionInput` / 动态头（`github-copilot-headers.ts`）。
- PKCE 工具 + OAuth 成功/失败 HTML 页（`utils/oauth/pkce.ts`、`oauth-page.ts`）。

### 3. 兼容 / 路由矩阵（消除不透明 `compat`）
- 把 `Model.compat: Option<serde_json::Value>` 升级为**强类型** compat 层：
  `OpenAICompletionsCompat`/`OpenAIResponsesCompat`/`AnthropicMessagesCompat`（`ai/src/types.ts:373-425`）。
- OpenRouter 路由（`types.ts:480-546`）、Vercel AI Gateway 路由（`:554-559`）。
- `thinkingFormat`（openai/openrouter/deepseek/zai/qwen…）、`thinkingLevelMap` 强类型化。

### 4. 协议补强
- **session-affinity 头注入**：当前选项已解析但请求里从不发送。覆盖 OpenAI/Anthropic/Azure/Mistral 各自的 affinity 头与 `prompt_cache_key`。
- prompt cache：`cache_control` TTL 标记（Anthropic）、`prompt_cache_retention`（OpenAI/Azure）。

### 5. 图像生成 + diagnostics
- 图像生成 API：`ai/src/images.ts` + OpenRouter 图像 provider + `image-models.generated.ts`。
- 结构化 diagnostics：`ai/src/utils/diagnostics.ts`（收集/格式化，接入事件流）。
- 内容签名 hash：`ai/src/utils/hash.ts`。

## 验收 / 测试（离线优先）
- 每个 provider 用 **faux/fixture** 做请求体与流式解析的字节级断言，**不**用真实 key。
- SigV4 用已知向量做签名单测；WebSocket 用 mock 帧。
- compat 矩阵：表驱动单测覆盖各标志组合。
- 全局注册表用唯一 api id 隔离（见 [cross-cutting.md](cross-cutting.md) 风险项）。
