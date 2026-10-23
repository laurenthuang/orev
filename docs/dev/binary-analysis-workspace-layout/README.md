# 二进制分析项目目录规范 v1.7

本规范适用于以下场景：

- 分析对象为固件镜像、单文件可执行文件与库、恶意样本
- 使用者为单人
- 运行环境为本地文件系统

本规范达成四个目标：

- **有序**：阶段可编排
- **高效**：分析过程可增量执行，产物可重建
- **可查询**：索引机器可读
- **易执行**：依赖约定，不依赖记忆

> **与单文件版的关系**：上级目录的 `binary-analysis-workspace-layout.md` 是同一份规范的历史归档，内容完整但不再更新。本目录是现行版本的唯一维护对象。

## 阅读顺序

按下面的顺序读。

1. [§15 完整示例](02-workflow/06-完整示例.md)。先看一个真实案件的目录长什么样。
2. [§0 编号体系速查](00-overview/00-编号体系速查.md)。遇到看不懂的记号，回来查这张表。
3. [§16 落地检查清单](04-appendix/01-落地检查清单.md)。照着勾选即可开工。

需要论证时，读 [§1 设计原则与要求总表](00-overview/01-设计原则.md) 与 [§17 Ghidra 12.1.4 对照](04-appendix/02-Ghidra-12.1.4-对照.md)。前者说明每条约定为什么存在，后者用 Ghidra 12.1.4 逐项核对了落点。

## 五个分块

| 目录 | 主题 | 解决的问题 |
| ------- | ---------- | -------------------------- |
| `00-overview/` | 记号与总纲 | 先读这里，确定要不要用、用到多深 |
| `01-layout/` | 静态约定 | 目录怎么排，名字怎么起 |
| `02-workflow/` | 分析过程 | 从入档到封存，一次调查要做的事 |
| `03-operations/` | 日常运维 | 备份、清理、检索、隔离 |
| `04-appendix/` | 附录 | 照抄的清单与工具对照 |
| `schemas/` | 文件格式 | 本规范定义的全部格式，可直接复制使用 |
| `scripts/` | 命令行 | 正文引用的查询与运维命令 |

## 章节索引

| 节 | 内容 | 文件 |
| ------ | -------------------------- | -------------------------- |
| **[§0](00-overview/00-编号体系速查.md)** | **编号体系速查（★）** | [`00-编号体系速查.md`](00-overview/00-编号体系速查.md) |
| **[§1](00-overview/01-设计原则.md)** | **设计原则与要求总表** | [`01-设计原则.md`](00-overview/01-设计原则.md) |
| **[§2](01-layout/01-顶层布局.md)** | **顶层布局（冷区 store/ 与工具资产区 tools/）** | [`01-顶层布局.md`](01-layout/01-顶层布局.md) |
| **[§3](01-layout/02-目标标识.md)** | **目标标识（target-id）** | [`02-目标标识.md`](01-layout/02-目标标识.md) |
| **[§4](01-layout/03-阶段目录契约.md)** | **阶段目录契约（stage.yaml）** | [`03-阶段目录契约.md`](01-layout/03-阶段目录契约.md) |
| **[§5](01-layout/04-阶段产物清单.md)** | **阶段 × 产物清单** | [`04-阶段产物清单.md`](01-layout/04-阶段产物清单.md) |
| **[§6](02-workflow/01-证据契约.md)** | **证据契约（findings.jsonl）** | [`01-证据契约.md`](02-workflow/01-证据契约.md) |
| **[§7](02-workflow/02-调查线索.md)** | **纵轴：调查线索（investigation.jsonl）** | [`02-调查线索.md`](02-workflow/02-调查线索.md) |
| **[§8](03-operations/01-保留分级与清理.md)** | **保留分级与清理** | [`01-保留分级与清理.md`](03-operations/01-保留分级与清理.md) |
| **[§9](03-operations/02-索引与查询.md)** | **索引与查询** | [`02-索引与查询.md`](03-operations/02-索引与查询.md) |
| **[§10](02-workflow/03-知识库与语料库.md)** | **知识库与语料库** | [`03-知识库与语料库.md`](02-workflow/03-知识库与语料库.md) |
| **[§11](02-workflow/04-时间旅行与版本控制.md)** | **时间旅行与版本控制（T6 落点）** | [`04-时间旅行与版本控制.md`](02-workflow/04-时间旅行与版本控制.md) |
| **[§12](02-workflow/05-结项封存.md)** | **结项封存（S4 落点）** | [`05-结项封存.md`](02-workflow/05-结项封存.md) |
| **[§13](03-operations/03-安全与隔离.md)** | **安全与隔离（恶意样本）** | [`03-安全与隔离.md`](03-operations/03-安全与隔离.md) |
| **[§14](00-overview/02-启用档位.md)** | **启用档位（D0–D2）** | [`02-启用档位.md`](00-overview/02-启用档位.md) |
| **[§15](02-workflow/06-完整示例.md)** | **完整示例** | [`06-完整示例.md`](02-workflow/06-完整示例.md) |
| **[§16](04-appendix/01-落地检查清单.md)** | **落地检查清单** | [`01-落地检查清单.md`](04-appendix/01-落地检查清单.md) |
| **[§17](04-appendix/02-Ghidra-12.1.4-对照.md)** | **Ghidra 12.1.4 对照（★）** | [`02-Ghidra-12.1.4-对照.md`](04-appendix/02-Ghidra-12.1.4-对照.md) |

## `schemas/` 清单

| 文件 | 用途 | 定义位置 |
| ----------------------- | ---------------------------------------- | ------------ |
| [`orev.toml`](schemas/orev.toml) | 工作区配置：阶段启用、**对比组的 `name`**、样本保留、存储根 | [§2.1](01-layout/01-顶层布局.md#21-冷区-store-与工具资产区-tools)、[§4.2](01-layout/03-阶段目录契约.md#42-阶段编号) |
| [`stage.yaml`](schemas/stage.yaml) | 阶段元数据：输入输出、工具溯源、自检判据 | [§4.3](01-layout/03-阶段目录契约.md#43-stageyaml-字段定义) |
| [`provenance.json`](schemas/provenance.json) | 来源与授权：外部语境 | [§5.2](01-layout/04-阶段产物清单.md#52-00_inbox-两个元数据文件的职责边界) |
| [`findings.jsonl`](schemas/findings.jsonl) | 结论与证据：一行一条结论 | [§6.1](02-workflow/01-证据契约.md#61-schema) |
| [`investigation.jsonl`](schemas/investigation.jsonl) | 调查线索台账：纵轴 | [§7](02-workflow/02-调查线索.md) |
| [`checksums.jsonl`](schemas/checksums.jsonl) | 完整性台账：巡检基线 | [§8.3](03-operations/01-保留分级与清理.md#83-完整性校验) |
| [`index.jsonl`](schemas/index.jsonl) | 产物索引：一行一件产物 | [§9.1](03-operations/02-索引与查询.md#91-stateindexjsonl) |
| [`manifest.json`](schemas/manifest.json) | 封存清单：交付与取证的总入口 | [§12](02-workflow/05-结项封存.md) |
| [`vtsession.json`](schemas/vtsession.json) | Ghidra Version Tracking 会话元数据 | [§17.3](04-appendix/02-Ghidra-12.1.4-对照.md#173-60_diff--version-tracking) |
| [`env-snapshot.md`](schemas/env-snapshot.md) | 环境快照：自动探测加 TTL 缓存 | [§4.4](01-layout/03-阶段目录契约.md#44-环境快照-stateenvmd) |
| [`kb-article-template.md`](schemas/kb-article-template.md) | 知识库条目模板 | [§10](02-workflow/03-知识库与语料库.md) |
| [`workspace.gitignore`](schemas/workspace.gitignore) | gitignore 模板：使用时改名为 `.gitignore` | [§8.2](03-operations/01-保留分级与清理.md#82-gitignore) |

规则：这些文件中的注释即字段说明。复制后按需删减，不要凭记忆重写字段名。

## `scripts/` 清单

| 文件 | 用途 | 定义位置 |
| ---------------------------- | -------------------------- | ------------ |
| [`export-evidence.sh`](scripts/export-evidence.sh) | 导出某条结论的自包含证据包 | [§6.5](02-workflow/01-证据契约.md#65-证据导出吸收-reark-的-copy-evidence) |
| [`query-investigation.sh`](scripts/query-investigation.sh) | 线索台账的三条查询 | [§7](02-workflow/02-调查线索.md) |
| [`verify-integrity.sh`](scripts/verify-integrity.sh) | 生成完整性基线并执行校验 | [§8.3](03-operations/01-保留分级与清理.md#83-完整性校验) |
| [`pre-clean-check.sh`](scripts/pre-clean-check.sh) | 清理前的安全检查 | [§8.4](03-operations/01-保留分级与清理.md#84-清理前的安全检查) |
| [`query-index.sh`](scripts/query-index.sh) | 索引的四条高频查询 | [§9.2](03-operations/02-索引与查询.md#92-高频查询) |
| [`git-tagging.sh`](scripts/git-tagging.sh) | 关键节点打案件级 tag | [§11](02-workflow/04-时间旅行与版本控制.md) |

规则：这些片段是摘录，不是可直接运行的生产脚本。使用前先确认命令中的路径与工具版本。

## 样例

| 文件 | 内容 |
| ------ | ---------- |
| [`examples/gnuture-elf.md`](examples/gnuture-elf.md) | 单文件 ELF 全流程：从入档到研判的可复跑样例，全部哈希为真实运行结果 |

[§15 完整示例](02-workflow/06-完整示例.md) 是结构示意，样例是可复跑的真实流水线。读规范看前者，动手看后者。

## 两条通用约定

1. **章节编号沿用单文件版本**。拆分只改变了内容的组织方式，没有引入新编号。
2. **正文中的 `§N` 全部是可点击的相对链接**。跨块跳转不需要回总览。
