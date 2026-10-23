# 17. Ghidra 12.1.4 对照

> **目的**：验证本规范能覆盖 Ghidra 分析流程的**输入、输出、中间产物、元数据**四类物件。
> **参照版本**：**Ghidra 12.1.4**（2026-09-21 发布，当前最新 PUBLIC 版；12.2 仍为 DEV 版）。

## 17.1 四类物件映射总表 ★

| 类别 | Ghidra 侧物件 | 本规范落点 | retention |
| -------- | ------------------------------ | ---------------------------------------------------------- | --------------- |
| **输入** | 原始二进制 | `00_inbox/<原始文件名>`（`input_file`） | **permanent** |
| | `FSRL`（来源定位） | `00_inbox/provenance.json` → `source_locator` | **permanent** |
| | 附加导入 / 依赖库（跨模块解析用） | `corpus/reference-bins/`（[§10](../02-workflow/03-知识库与语料库.md)，跨案件复用） | **permanent** |
| **中间产物** | `.gpr` + 同名 `.rep/`（ProgramDB） | `tools/ghidra/working/<case-id>/<type>/<target-id>/`（[§2.1](../01-layout/01-顶层布局.md#21-冷区-store-与工具资产区-tools)） | **required** |
| | `.rep/versioned/` 快照 | 同上（工具级时间旅行，[§11](../02-workflow/04-时间旅行与版本控制.md)） | **required** |
| | `.rep/idata/` 跨 program 共享数据类型 | 同上 | **required** |
| | 反编译缓存、临时数据库 | `.cache/` | **temp** |
| **输出** | 反编译 C、函数清单、字符串、调用图、符号、类型导出 | `30_decompile/out/`，多组件时按 `<slug>/` 分桶（[§3.3](../01-layout/02-目标标识.md#33-固件内部组件)） | **rebuildable** |
| | BSim 签名、相似度匹配结果 | `30_decompile/out/` | **rebuildable** |
| | VT session、markup 应用记录 | `60_diff/` | **permanent** |
| **元数据** | Program Information 属性集 | `stage.yaml`（[§17.4](#174-元数据采集program-information--落点)） | **permanent** |
| | 人工批注（重命名 / 注释 / 类型 / 书签） | 存于 `.rep` 内，随项目备份 | **required** |
| | 脚本与分析日志 | `<stage>/log/` | **temp** |

> **Ghidra headless 导出脚本的典型产出**包括：`{name}_summary.txt`、`{name}_decompiled.c`、`{name}_functions.json`、`{name}_strings.txt`、`{name}_calls.json`、`{name}_symbols.json`。这些文件全部进入 `out/`。**仅多组件固件才按 `<slug>/` 分桶**（[§3.3](../01-layout/02-目标标识.md#33-固件内部组件)）。文件名不需要改写。

## 17.2 `30_decompile` 的两种执行方式

同一个 `30_decompile` 阶段，Ghidra 支持两种执行方式。两种方式的性质不同。混用这两种方式会丢失数据：

| | **导出型**（批量 / 粗筛） | **认知型**（深度 / 人工） |
| --------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| 命令 | `analyzeHeadless … -import "$TARGET_INPUT" -readOnly -postScript ExportAll.java` | `analyzeHeadless … -import "$TARGET_INPUT" -postScript …`（**不加** `-readOnly`/`-deleteProject`） |
| 项目 | 临时的，跑完即弃 | 落在 `tools/ghidra/working/<case-id>/…`，长期保留 |
| 产物 | 只有 `out/` 里的导出物 | 项目本身 + 导出物 |
| retention | **rebuildable**（删了再跑就有） | **required**（人工认知，不可再生） |
| 幂等 | ✅ 天然幂等 | ❌ 人工状态不可逆 |

**四条硬规则**：

1. **认知型禁止 `-readOnly`**。该参数在 `-process` 模式下会丢弃脚本与 analyzer 的**全部**改动，在 `-import` 模式下**隐含 `-deleteProject`**。很多流水线为避免「污染项目」默认添加该参数，结果是人工批注静默消失。
2. **`-deleteProject` 仅对本次会话新建的项目生效**，已有项目永不被删除。因此「复用同一项目反复执行」是安全的。危险的是只读模式。
3. **超时截断必须可被发现**。`-analysisTimeoutPerFile` 触发时，已完成的分析器结果**仍会被保存**，程序处于「分析过但未完成」的状态。这正是 [§8.4](../03-operations/01-保留分级与清理.md#84-清理前的安全检查) 原子写机制要防止的半截产物。**判据是 `ANALYZED` 属性**，不是退出码。
4. **用 `<project_location>` 参数把项目位置直接指定到 `tools/`，不要使用软链接。** `analyzeHeadless` 的语法是 `<project_location> <project_name>`。让工具直接写入 `tools_root/ghidra/working/<case-id>/<type>/<target-id>/`（[§2.1](../01-layout/01-顶层布局.md#21-冷区-store-与工具资产区-tools)），再把 `project_dir` 记入 `stage.yaml`。工具支持显式路径，因此不需要用符号链接改写路径。

**一个 target 对应一个 project**。固件解包出上百个组件时，**不要为每个组件建立独立 project**：

- 同一项目内的多个 program 能解析**跨模块导入 / 导出**（`httpd` ↔ `libssl.so` ↔ `libc`）。反编译结果中可以直接看到跨模块调用，这是固件分析的核心价值。
- 自定义数据类型存储在 `.rep/idata/`，**跨 program 共享**。拆分成多个项目后无法复用。
- 组件的区分体现在**项目内的 folder** 与**导出物的 `<slug>/` 分桶**上（**仅多组件时才有 `<slug>/`**，单文件 target 直接平铺在 `out/`，见 [§3.3](../01-layout/02-目标标识.md#33-固件内部组件)）。不需要用项目数量表达组件数量。

## 17.3 `60_diff` × Version Tracking

VT 的数据模型分为四层：**session → association → match → markup item**

| 层 | 含义 |
| --------------- | --------------------------------------------------- |
| **session** | 一次对比会话，**存于 Ghidra Project 内** |
| **association** | 一对函数 / 数据的关联候选 |
| **match** | 确认的匹配（带 Score / Length / Type / Status / Algorithm） |
| **markup item** | 可搬迁的标注项：函数名、标签、注释、数据类型、函数签名、引用 |

**correlators** 决定如何查找关联，必须记录。可选值包括：Exact Function Bytes / Exact Function Instructions / Exact Function Mnemonics / Exact Symbol Name / Symbol Name / Combined Function and Data / Data Reference / Function Reference / Duplicate Function Instructions / Manual Match。

### 落点裁决：VT session 放在哪里

**裁决**：VT 项目放 `tools/`，导出物留在 `60_diff/`。

**难点**：session 存储在 Ghidra Project 内，而 project 需要按案件归属。diff 的导出物（报告、CSV）属于案件级阶段产物。本规范的统一规则是：**工具私有数据一律放 `tools/<tool>/working/`，人类可读的导出物一律放阶段目录**。

```
60_diff/                              # 案件级阶段目录：只有导出物与元数据
├── stage.yaml
├── out/
│   └── <diff-name>/                    # 一组对比一个桶；只有一组时也要建（[§3.3](../01-layout/02-目标标识.md#33-固件内部组件)）
│       ├── vtsession.json              # 导出的会话记录（可读、可入 git）
│       ├── markup-applied.md           # 人工记录了哪些、应用到哪一端
│       ├── changed-functions.csv
│       └── diff-report.html
└── log/

tools/ghidra/working/<case-id>/ghidra-vt/<diff-name>/   # 工具私有：VT 项目本身（required）
└── {vt.gpr, vt.rep/}
```

会话元数据样例见 [`schemas/vtsession.json`](../schemas/vtsession.json)。

规则：`vtsession.json` 的 `session_name` 与 `orev.toml` 中该组 `[[diff]]` 的 `name` **取值必须相同**。两处表达同一件事，取值不同就会互相矛盾。

**四条硬约束**（来自 VT 的实际机制）：

1. **session 独占检出，不支持合并冲突**。单人场景无影响；多人协作存在硬性限制 —— 同一时刻只能有一个人打开一个 session。
2. **VT 比较的是「程序」而不是「文件」**。两端必须都已 import 且分析完成（`ANALYZED=true`），否则 correlator 结果不可信。因此 `60_diff` **依赖** `30_decompile` 的 `ok` 状态。
3. **markup 应用是单向的**（把 source 的标注搬到 destination，或反向）。**必须记录搬运了哪些项、搬到哪一端**。缺少记录时，下次重跑无法复现 destination 的状态。
4. **correlator 组合要记全**。更换 correlator 组合会得到不同结果。不记录就无法复现，这与 [§6](../02-workflow/01-证据契约.md) 中 `evidence[].query` 的要求同理。

> 使用 BinDiff / Diaphora 的用户沿用 [§5](../01-layout/04-阶段产物清单.md) 的 `60_diff/out/<diff-name>/` 即可，**不强制建立 `ghidra-vt/`**。本节只为 Ghidra 用户提供落地路径。

## 17.4 元数据采集：Program Information → 落点

Ghidra 每个 Program 自带一组属性。这些属性的采集成本最低，也最容易被忽略：

| Ghidra 属性 | 含义 | 本规范落点 |
| ---------------------------------------------------------------------------- | -------- | -------------------------------------------------- |
| `executable MD5` / `executable SHA256` | 文件哈希 | **与 `00_inbox/checksums.sha256` 交叉验证**（[§5.2](../01-layout/04-阶段产物清单.md#52-00_inbox-两个元数据文件的职责边界)）；不一致 = 导错文件 |
| `FSRL` | 来源定位 | `00_inbox/provenance.json` 的 `source_locator` |
| Program 名 | 由导入文件名决定 | **等于原始文件名**。这是 `00_inbox` 不改名（[§5.2](../01-layout/04-阶段产物清单.md#52-00_inbox-两个元数据文件的职责边界)）的直接收益：项目内显示的名字符合实际来源 |
| `created with ghidra version` | 建库版本 | `stage.yaml` → `tool.version` |
| **`ANALYZED`** | 自动分析是否完成 | **`tool.selfcheck.analyzed`** ← `status: ok` 的客观判据 |
| `ANALYSIS_START_DATE` / `DATE_CREATED` | 时间戳 | `stage.yaml` 的 `started_at` / `finished_at` |
| `executable format` / `language ID` / `compiler ID` / `processor` / `endian` | 目标画像 | `20_static/out/headers.json`（与 readelf 结果互校） |
| `# of functions / symbols / data types` | 规模指标 | `log/metrics.json`，用于跨版本对比时判断分析是否退化 |

ProgramDB 的各大管理器（`getFunctionManager` / `getDataTypeManager` / `getReferenceManager` / `getBookmarkManager` / `getEquateTable` / `getRelocationTable` / `getProgramContext` / `getExternalManager`）**全部位于 `.rep/` 内，整体属于 required，不单独导出**。`getSourceFileManager`（自 11.4.2 起）用于源码关联；使用时应导出 `20_static/out/source-map.json`（rebuildable）。

> ⚠️ **一条纪律**：上述属性属于**工具内部状态**。本规范只把它们**抄录为文本**（`stage.yaml` 或 JSON），**从不把 `.rep` 作为唯一记录**。工具数据库的格式会随版本升级改变，文本格式不会。**凡是不能在文本中查到的元数据，等于不存在。**

## 17.5 项目文件结构与备份（呼应 [§8.1](../03-operations/01-保留分级与清理.md#81-保留分级)）

> 物理位置由 `tools_root` 决定（[§2.1](../01-layout/01-顶层布局.md#21-冷区-store-与工具资产区-tools)）。路径为 `tools/ghidra/working/<case-id>/<type>/<target-id>/`。以下结构不变。

```
tools/ghidra/working/<case-id>/<type>/<target-id>/
├── p.gpr                       # 项目文件（小）
└── p.rep/                      # 项目仓库 —— 是目录，不是文件
    ├── project.prp             # 项目属性（身份卡）
    ├── projectState            # 会话状态（最近打开、窗口布局）
    ├── idata/                  # 非版本化主存储 + 检出文件；跨 program 共享类型在此
    ├── versioned/              # 版本历史 —— 仅私有非共享项目
    ├── user/                   # 用户特定数据
    ├── ~index.dat / ~index.bak / ~index.tmp   # 索引（含备份、临时）
    ├── ~journal.dat / ~journal.bak            # 变更日志
    ├── ~index.lock                            # 锁
    └── ~*.db/                                 # ProgramDB 实体存储
```

- **备份三原则**：**关闭项目** → **整包拷**（`.gpr` + `.rep/` 成对）→ **换路径打开验证**。
- 索引损坏后可以重建。重建时必须**先关闭项目或停止 Ghidra Server**，再手工删除 `~index.*` 以触发重建。在项目打开状态下执行拷贝，得到的副本可能不完整。
- `.rep` 是目录，`.gitignore` 中写 `*.rep/`（带斜杠）。
- **共享项目**（Ghidra Server）的 `versioned/` 位于**服务端**，不在本地。单人本地场景使用私有项目即可，不需要启动 Server。

## 17.6 覆盖度结论

| 类别 | 是否覆盖 | 落点 |
| -------- | ---------- | -------------------------------------------------------------------- |
| **输入** | ✅ | `00_inbox`（二进制、FSRL）+ `corpus/reference-bins/`（依赖库） |
| **输出** | ✅ | `30_decompile/out/`（反编译、函数、字符串、调用图、符号；多组件按 `<slug>/` 分桶） |
| **中间产物** | ✅ | `tools/ghidra/working/<case-id>/…` + `corpus/bsim/`（项目、快照、共享类型、BSim） |
| **元数据** | ✅ | `stage.yaml` / `log/metrics.json` / `manifest.json` |
| **差分** | ✅ | `60_diff/` + `tools/ghidra/working/<case-id>/ghidra-vt/` |

## 17.7 已知边界

1. **协作**：VT session 独占检出、不支持合并冲突。本规范面向**单人本地**场景，多人协作需要额外的锁协议 —— **不在本规范范围内**。
2. **Ghidra Server**：共享项目、服务端版本库、用户与权限管理。单人场景不需要，未纳入本规范。
3. **BSim 运维**：BSim 数据库是**运行中的服务**（PostgreSQL / Elasticsearch），不是目录中的文件。本规范只记录其**位置与构建命令**，不承担生命周期管理。
4. **SLEIGH / 处理器扩展**：自研 `.sla` / `.ldefs` 属于**工具链资产**，应放 `kb/techniques/` 或独立的工具链仓库，不在案件目录内。
5. **`projectState` / `user/`**：纯 GUI 状态，无分析价值，不建立索引。随 `.rep` 一并拷走也不产生副作用。

## 17.8 结论

Ghidra 的「以项目为中心」与本规范的「以阶段为中心」**不冲突**。Ghidra 项目是 `30_decompile` 阶段的一个槽位内容，不是目录结构的竞争方案。

真正需要对齐的是两个问题：

1. **哪些数据只能留在工具内**：无法单独取出的认知资产。
2. **哪些数据必须抄录为文本**：否则工具升级后即丢失。

本规范的答案：

- 前者按整体 **required** 级别执行备份。
- 后者写入 `stage.yaml`、`out/` 与 `index.jsonl`。

---

上一节：[16. 落地检查清单](../04-appendix/01-落地检查清单.md) ｜ [返回总览](../README.md)
