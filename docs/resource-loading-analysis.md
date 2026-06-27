# pi-rust 外部资源加载逻辑 — 对标 pi (TypeScript)

## 目录

1. [整体架构对比](#1-整体架构对比)
2. [Skills 加载](#2-skills-加载)
3. [Prompt Templates 加载](#3-prompt-templates-加载)
4. [Agent Context Files (AGENTS.md)](#4-agent-context-files-agentsmd)
5. [System Prompt 构建](#5-system-prompt-构建)
6. [Theme 加载](#6-theme-加载)
7. [Frontmatter 解析](#7-frontmatter-解析)
8. [Substitute Args / 模板变量替换](#8-substitute-args--模板变量替换)
9. [差异与待补项](#9-差异与待补项)

---

## 1. 整体架构对比

| 维度 | TypeScript pi | Rust pi-rust |
|------|:---|:---|
| **资源层在哪** | `packages/agent/src/harness/` + `packages/coding-agent/src/core/` | `crates/pi-agent-core/src/resources/` + `crates/pi-coding-agent/src/resources.rs` |
| **设计模式** | 分层: `agent` (harness) 提供基础加载器, `coding-agent` (core) 提供 CLI 整合 | 分层: `pi-agent-core` 提供基础加载, `pi-coding-agent` 提供 CLI 整合 |
| **路径发现** | `PackageManager.resolve()` 解析 `package.json` → 技能/提示/主题/扩展 | `resources.rs` 直接硬编码默认目录列表 |
| **Source provenance** | `SourceInfo { path, source, scope, origin, baseDir }` 追踪每个资源的来源 | 无对应机制 (`SourceTag` 仅在 sourced 变体中简单标记) |
| **Skill skill 名称校验** | 严格: 只允许 `[a-z0-9-]+`, 不允许 `--` 或 `-` 开头/结尾, 名称必须匹配父目录 | 无校验 (仅长度截断到 64 字符) |
| **Skill description 校验** | desc 为空 → skill 被完全拒绝 (返回 null) | desc 为空 → 用 `fallback_description` 填, skill 仍被接受 |

---

## 2. Skills 加载

### 2.1. TS 调用链

```
coding-agent/core/resource-loader.ts
  → reload()
    → updateSkillsFromPaths(skillPaths)
      → coding-agent/core/skills.ts :: loadSkills({ skillPaths })
        → agent/harness/skills.ts :: loadSkills(env, dirs)  [异步, 通过 ExecutionEnv]
          → loadSkillsFromDirInternal(env, dir, includeRootFiles=true, ...)
            → 扫描每个目录:
              1. SKILL.md (优先, 非递归)
              2. 子目录递归查找 SKILL.md
              3. 顶层直接 .md 文件 (includeRootFiles=true 时)
            → 尊重 .gitignore (.ignore, .fdignore)
            → 过滤: 以 `.` 开头的目录、`node_modules`
          → loadSkillFromFile(env, filePath)
            → 解析 frontmatter (自定义 parser, `---` YAML)
            → 校验 name / description (严格规则)
            → description 为空 → 拒绝该 skill
```

### 2.2. Rust 调用链

```
coding-agent/resources.rs :: load_cli_resources_with_options()
  → pi-agent-core/resources/skills.rs :: load_skills(paths)
    → load_skills_from_dir(root, ...)
      → 扫描每个目录:
        1. SKILL.md (根目录)
        2. 子目录递归查找 SKILL.md (使用 ignore crate 的 WalkBuilder, .gitignore)
        3. 顶层直接 .md 文件
      → load_skill_file(path, ...)
        → 解析 frontmatter (serde_yaml, --- YAML)
        → 提取 name (max 64 chars, 无其他校验)
        → 提取 description (max 1024 chars, 空时用 body 首行填充)
        → 提取 disable_model_invocation
```

### 2.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| SKILL.md 发现 | ✅ | ✅ | — |
| 子目录递归 | ✅ | ✅ | — |
| 直接 .md 文件 | ✅ | ✅ | — |
| .gitignore 过滤 | ✅ | ✅ (ignore crate) | — |
| name 校验规则 | ✅ 严格 | ❌ | **需要添加** |
| description 为空拒绝 | ✅ | ❌ | **需要添加** |
| 异步 I/O | ✅ | — | Rust 同步 ok |
| disable_model_invocation | ✅ | ✅ | — |
| sourced 来源标记 | ✅ SourceInfo | ⚠️ SourceTag 简化 | 可接受 |

### 2.4. 默认扫描路径

- **TS:** `agentDir/skills/`, `cwd/.pi/skills/`, 以及扩展贡献的 skill 路径
- **Rust:** `agentDir/skills/`, `cwd/.pi-rust/skills/`, settings 中的 `skills` 列表, CLI `--skills` 参数

---

## 3. Prompt Templates 加载

### 3.1. TS 调用链

```
coding-agent/core/resource-loader.ts
  → updatePromptsFromPaths(promptPaths)
    → coding-agent/core/prompt-templates.ts :: loadPromptTemplates({ promptPaths })
      → 从文件 .md 加载:
        - name: 文件名 (去 .md)
        - description: frontmatter 或首行 (max 60 chars)
        - argumentHint: frontmatter (可选)
        - content: body
      → 去重: 同名冲突 → 警告, 先到先得
      → sourceInfo: 标记来源 (user/project/temp)

agent/harness/prompt-templates.ts 提供更底层的异步版本:
  - parseCommandArgs (引号解析)
  - substituteArgs ($1, $@, $ARGUMENTS, ${N:-default}, ${@:N:L})
  - formatPromptTemplateInvocation
```

### 3.2. Rust 调用链

```
coding-agent/resources.rs :: load_cli_resources_with_options()
  → pi-agent-core/resources/prompt_templates.rs :: load_prompt_templates(paths)
    → 从文件 .md 加载:
      - name: frontmatter 或文件 stem
      - description: frontmatter 或首行 (max 60 chars + "...")
      - content: body
    → 无去重

pi-agent-core/resources/system_prompt.rs:
  - parse_command_args (引号解析)
  - substitute_args ($1, $@, $ARGUMENTS, ${N:-default}, ${@:N:L})
  - format_prompt_template_invocation
```

### 3.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| .md 文件加载 | ✅ | ✅ | — |
| 目录扫描 (非递归) | ✅ | ✅ | — |
| frontmatter 解析 | ✅ | ✅ | — |
| argumentHint | ✅ | ❌ | 低优先级, 前端特性 |
| 去重 | ✅ | ❌ | **需要添加** |
| parseCommandArgs | ✅ | ✅ | — |
| substituteArgs | ✅ | ✅ (全面) | — |
| formatPromptTemplateInvocation | ✅ | ✅ | — |
| sourceInfo 溯源 | ✅ | ❌ | 可接受 |

### 3.4. 默认扫描路径

- **TS:** `agentDir/prompts/`, `cwd/.pi/prompts/`, CLI `--prompt` 路径, 扩展贡献
- **Rust:** `agentDir/prompts/`, `agentDir/prompt-templates/`, `cwd/.pi-rust/prompts/`, `cwd/.pi-rust/prompt-templates/`, settings `prompts`, CLI `--prompt-templates`

---

## 4. Agent Context Files (AGENTS.md)

### 4.1. TS 行为

```
coding-agent/core/resource-loader.ts
  → loadProjectContextFiles({ cwd, agentDir })
    → 候选文件名: AGENTS.md, AGENTS.MD, CLAUDE.md, CLAUDE.MD
    → 扫描:
      1. agentDir (全局) → 最先
      2. cwd 到根目录逐级向上 → ancestors (反转后追加)
    → 去重: 同路径只加载一次
```

在 `system-prompt.ts :: buildSystemPrompt()` 中以 `<project_instructions path="...">` 形式嵌入 system prompt。

### 4.2. Rust 行为

```
coding-agent/resources.rs :: discover_context_files(cwd, agentDir, disabled)
  → 候选文件名: AGENTS.md, AGENTS.MD, CLAUDE.md, CLAUDE.MD
  → 扫描:
    1. agentDir (全局) → 最先
    2. cwd 到根目录逐级向上 → ancestors (反转后追加)
  → 去重: BTreeSet<PathBuf>
```

在 `request.rs :: resolve_system_prompt()` 中作为 system prompt 部分拼接。

### 4.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| 候选文件名 | ✅ | ✅ | — |
| agentDir 全局 | ✅ | ✅ | — |
| cwd→root 逐级 | ✅ | ✅ | — |
| 去重 | ✅ | ✅ | — |
| noContextFiles 禁用 | ✅ | ✅ | — |

---

## 5. System Prompt 构建

### 5.1. TS

```
coding-agent/core/system-prompt.ts :: buildSystemPrompt()
  → 两种模式:
    1. customPrompt 存在:
       - 使用 customPrompt 作为基础
       - 追加 appendSystemPrompt
       - 追加 contextFiles (以 <project_instructions> 包裹)
       - 追加 skills (formatSkillsForPrompt → <available_skills> XML)
       - 追加日期 + cwd
    2. 无 customPrompt:
       - 构建完整的默认系统提示
         - 工具列表 + 代码片段
         - 指南 (guidelines)
         - pi 文档路径
       - 同上追加 context + skills
```

### 5.2. Rust

```
coding-agent/request.rs :: resolve_system_prompt()
  → 如果 parsed.system_prompt 存在:
      - 使用它作为基础
      - 追加 contextFiles (无 <project_instructions> 包裹!)
      - 追加 parsed.append_system_prompt
    → 如果只有 append_system_prompt 没有 system_prompt:
      - 使用 DEFAULT_SYSTEM_PROMPT ("You are a helpful coding assistant.") 作为基础
      - 追加 append_system_prompt

pi-agent-core/convert.rs :: assemble_context()
  → 在 context 组装时将 skills 作为 <available_skills> XML 块追加到 system prompt
    (format_skills_for_system_prompt)
```

### 5.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| 自定义 system prompt | ✅ | ✅ | — |
| 追加 prompt | ✅ | ✅ | — |
| 默认 system prompt | ✅ 详尽 (含工具/指南/pi 文档) | ✅ 简陋 ("helpful coding assistant") | **需要补全** |
| context files 嵌入 | ✅ `<project_instructions>` 包裹 | ❌ 直接拼接 | **需要添加** |
| skills 嵌入 | ✅ `<available_skills>` | ✅ | — |
| 日期 + cwd | ✅ | ❌ | **需要添加** |
| 工具特定指南 | ✅ dynamic | ❌ | 低优先级 |

---

## 6. Theme 加载

### 6.1. TS

```
coding-agent/core/resource-loader.ts
  → updateThemesFromPaths(themePaths)
    → loadThemes(paths)
      → agentDir/themes/ (全局, *.json)
      → cwd/.pi/themes/ (项目, *.json)
      → CLI explicit paths
      → 去重: 同名先到先得
      → 排序: 按名称
```

### 6.2. Rust

```
coding-agent/resources.rs :: load_themes(paths)
  → agentDir/themes/ (全局, *.json)
  → cwd/.pi-rust/themes/ (项目, *.json)
  → CLI explicit paths
  → 去重: BTreeSet<name>, 先到先得
  → 排序: 按名称
  → 校验: 缺失颜色 token 报告 warning
```

### 6.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| *.json 加载 | ✅ | ✅ | — |
| 目录递归 | ❌ (平铺) | ❌ (平铺) | 对齐 |
| 去重 | ✅ | ✅ | — |
| 缺失 token 校验 | ✅ | ✅ | — |
| sorted by name | ✅ | ✅ | — |
| 符号链接解析 | ✅ | ❌ | 低优先级 |
| sourceInfo 溯源 | ✅ | ❌ | 可接受 |

---

## 7. Frontmatter 解析

### 7.1. TS

```
agent/harness/skills.ts :: parseFrontmatter<T>()
  → 自实现解析器 (不依赖 yaml 库的完整解析)
  → 先 normalize: \r\n → \n
  → 查找第二个 --- 分隔符
  → YAML 部分用 yaml.parse() (js-yaml 或等效)
  → 返回 { frontmatter, body }

agent/harness/prompt-templates.ts :: parseFrontmatter<T>()
  → 同样自实现
```

### 7.2. Rust

```
pi-agent-core/resources/frontmatter.rs :: parse_frontmatter()
  → normalize: \r\n → \n
  → 查找第二个 --- 分隔符
  → YAML 部分用 serde_yaml 解析
  → 返回 (Value, String, Vec<ResourceDiagnostic>)
  → 错误处理更严格: 没有闭合 --- 会报 diagnostic
```

### 7.3. 对标状态

| 特性 | TS | Rust | 缺口 |
|------|:---:|:---:|------|
| YAML 解析 | ✅ | ✅ | — |
| CRLF 标准化 | ✅ | ✅ | — |
| 错误诊断 | ⚠️ 部分 | ✅ 完善 | 对齐良好 |

---

## 8. Substitute Args / 模板变量替换

两个实现在功能上完全对齐:

| 功能 | TS Regex | Rust Regex |
|------|------|------|
| `$1`, `$2`, ... | ✅ | ✅ |
| `$@` | ✅ | ✅ |
| `$ARGUMENTS` | ✅ | ✅ |
| `${N:-default}` | ✅ | ✅ |
| `${@:N}` | ✅ | ✅ |
| `${@:N:L}` | ✅ | ✅ |
| 无递归替换 | ✅ | ✅ (by design) |
| `${N}` (无 `:-`) 不替换 | ✅ | ✅ |
| `parseCommandArgs` | ✅ | ✅ |

**结论: 模板替换 100% 对标。**

---

## 9. 差异与待补项

### 9.1. 高优先级 (影响行为正确性)

| # | 项 | 说明 | 状态 |
|---|-----|------|------|
| 1 | **System Prompt 太简陋** | TS 构建完整的默认提示 (工具列表、指南、pi 文档路径), Rust 只有 `"You are a helpful coding assistant."` | ✅ 已修复 |
| 2 | **Context files 无 `<project_instructions>` 包裹** | TS 在自定义 prompt 模式下仍会以结构化的 XML 格式嵌入 context files | ✅ 已修复 |
| 3 | **Skill name 无校验** | TS 拒绝不符合 `[a-z0-9-]+` 的名称、名称不匹配目录名、`--` 连续连字符等 | ✅ 已修复 |
| 4 | **空 description 应拒绝 Skill** | TS 立即拒绝, Rust 用 body 首行填充 | ✅ 已修复 |
| 5 | **Prompt Template 无去重** | TS 同名模板产生 collision 诊断 | ✅ 已修复 |

### 9.2. 中优先级 (功能完整性)

| # | 项 | 说明 | 位置 |
|---|-----|------|------|
| 6 | **Source provenance 缺失** | TS 为每个资源追踪 `SourceInfo` (来源、作用域等), Rust 只在 sourced 路径中使用简化版 `SourceTag` | 多个文件 |
| 7 | **System Prompt 无日期/cwd 后缀** | TS 在末尾加 `Current date: ...` 和 `Current working directory: ...` | `request.rs:327` |
| 8 | **PackageManager 缺失** | TS 可以解析 `package.json` 声明依赖来自动发现扩展, Rust 无此概念 | — |

### 9.3. 低优先级 (可选增强)

| # | 项 | 说明 | 位置 |
|---|-----|------|------|
| 9 | **argumentHint** | prompt template 的 frontmatter 字段, 前端使用, 可暂不实现 | `prompt_templates.rs` |
| 10 | **符号链接跟随** | TS theme/skill 加载支持 symlink, Rust 不支持 | `resources.rs` |
| 11 | **Extension 系统** | TS 有完整的 extension 加载机制, Rust 仅占位 | — |

---

## 总结

Rust 的核心加载逻辑（文件发现、frontmatter 解析、模板变量替换）已经对标 TS 到位。主要差距在 **System Prompt 质量**（太简陋）和 **Skill 数据校验**（太宽松）。建议优先修复高优先级项中的 1-5，这些直接影响 Agent 行为正确性和用户体验。
