# 样例：单文件 ELF 全流程（`gnuture`）

本文演示一个**单文件 ELF 可执行目标**从入档到研判的完整流程。所有哈希与产物均为真实执行结果，命令可直接复跑。

与 [§15](../02-workflow/06-完整示例.md) 的关系：[§15](../02-workflow/06-完整示例.md) 是结构示意，本文是可复跑的真实样例。**读规范时先看 [§15](../02-workflow/06-完整示例.md)，要实际动手时看本文。**

## 场景

| 项 | 取值 |
| --- | --- |
| 案件问题 | gnuture 是否存在命令注入或不安全字符串操作 |
| case-id | `gnuture-audit-2026q4` |
| target 类型 | `binary`（流水线 B，单文件可执行文件） |
| 原始文件 | `/usr/bin/sha256sum` 的副本，交付时的文件名为 `gnuture`，全程不改名 |
| 目标形态 | ELF 64-bit LSB **pie executable**, x86-64, dynamically linked, **stripped** |
| 启用档位 | **D0**（工具可得性受限，见文末边界） |

## Step 0 建 case 骨架

```bash
export CID=gnuture-audit-2026q4
mkdir -p cases/$CID/{state,scripts} tools/ghidra/scripts store kb corpus
```

## Step 1 `00_inbox` 入档

**冷启动顺序不能反**：target-id 含 sha256，而 sha256 要先把文件读出来才能算。因此第一步在 **case 之外**进行。

```bash
# 1) 先在临时位置取哈希 —— 此时还不知道目录名
sha256sum incoming/gnuture
# e484c36c0613879fbd11058f7a2e194783bced59d4805d64c3d4b0d7f664094d  incoming/gnuture

# 2) 前 12 位定 target-id：gnuture_e484c36c0613
export TID=gnuture_e484c36c0613
mkdir -p cases/$CID/binary/$TID/00_inbox

# 3) 复制。文件名不改写，仍是 gnuture
cp incoming/gnuture cases/$CID/binary/$TID/00_inbox/gnuture
chmod 644 cases/$CID/binary/$TID/00_inbox/gnuture

# 4) 生成 checksums.sha256，行内写实际文件名
cd cases/$CID/binary/$TID/00_inbox && sha256sum gnuture > checksums.sha256

# 5) 在 stage.yaml 中声明主输入是哪一个文件
#    input_file: gnuture
#    后续阶段一律用 $TARGET_INPUT 引用它，不在命令里写死文件名
```

产物：

```
00_inbox/
├── gnuture                # 39336 字节；原始文件名，未改写
├── checksums.sha256       # 权威指纹；行内写 gnuture
├── provenance.json        # 来源、授权、替代哈希（md5/sha1 对接情报平台）
└── stage.yaml             # input_file: gnuture；status: ok
```

> **为什么 `checksums.sha256` 里写实际文件名**：该文件由 `sha256sum -c` 就地消费，行内文件名必须与磁盘一致。哈希与文件名无关，因此不改名不影响指纹。

**`00_inbox/stage.yaml` 在本例中的关键字段**：

```yaml
schema: orev/stage@v1.2
stage: 00_inbox
target_id: gnuture_e484c36c0613
input_file: gnuture          # 主输入的实际文件名，保留原名
status: ok
outputs:
  - path: binary/gnuture_e484c36c0613/00_inbox/gnuture
    sha256: e484c36c0613879fbd11058f7a2e194783bced59d4805d64c3d4b0d7f664094d
    bytes: 39336
    retention: permanent
    tags: ["role=input"]
```

后续阶段的命令引用该输入时写 `-import "$TARGET_INPUT"`。runner 在运行前把它展开为 `…/00_inbox/gnuture` 的绝对路径。**文件名可变，命令模板不变** —— 这是不改名之后仍能保住模板化的机制。

**`provenance.json` 在本例中的取值**：`source.declared_version` 记为 `8.32`（来源自称）；`headers.json` 的 `identified_version` 为 `null`（文件已 strip）。**两处的差异是常态**，不是错误——见 [§3.2](../01-layout/02-目标标识.md#32-四条硬规则) 第 2 条。

## Step 2 `20_static`

```bash
cd cases/$CID/binary/$TID
mkdir -p 20_static/{out,log}
readelf -hW 00_inbox/gnuture > 20_static/log/elf-header.txt   # 脚本内写 "$TARGET_INPUT"，不写死文件名
# 其余产物由采集脚本从 readelf / strings 的输出抽取
```

产物（全部**平铺**在 `out/`，单文件目标不建 `<slug>/` 桶，见 [§3.3](../01-layout/02-目标标识.md#33-固件内部组件)）：

```
20_static/out/
├── headers.json      # format=elf / class=ELF64 / etype=DYN / entry=0x3a40
├── sections.json     # 32 个节；standard=true 表示已截断
├── symbols.json      # present=false, reason="stripped"
├── imports.json      # 69 条 UND 符号（动态依赖）
├── checksec.json     # relro=full, pie=true, nx=true, canary=true, fortify=1
├── strings.txt       # strings -n 6 的输出
└── sbom.spdx.json    # 单组件 SBOM
```

**本步引出的两条纪律**：

1. **空结果必须落成文件**。`symbols.json` 内容为 `{"present": false, "reason": "stripped", ...}`，而不是省略该文件。省略会让「分析过但为空」与「尚未分析」不可区分。这与 [§6.2](../02-workflow/01-证据契约.md#62-字段约束) 中 `unknowns[]` 可为空但字段必须存在是同一条原则。
2. **工具输出解析必须留痕**。首次实现把 `readelf -l -W` 的第 6 列当作 `Flg`，实为第 7 列（列序 `Type Offset VirtAddr PhysAddr FileSiz MemSiz Flg Align`）。当时 `nx` 结论碰巧正确，**但依据是错的**。因此 `checksec.json` 保留了 `_method` 字段记录取数列与实际取值，便于复核。

## Step 3 `30_decompile`（导出型，D0 档位）

本环境的 Ghidra / rizin / radare2 均不可用。按 [§17.2](../04-appendix/02-Ghidra-12.1.4-对照.md#172-30_decompile-的两种执行方式) 的**导出型**方式执行：不建工具项目，只产出导出物，工具游标为 `null`。

```bash
mkdir -p 30_decompile/{out,log}
objdump -d --no-show-raw-insn 00_inbox/gnuture > 30_decompile/out/assembly.asm   # 脚本内写 "$TARGET_INPUT"，不写死文件名
```

```
30_decompile/out/
├── assembly.asm        # 164739 字节
└── functions.json      # 61 个函数候选（来自 PLT 与符号名）
```

对应 `stage.yaml` 的差异：`tool.name = objdump`、`tool.project_dir = null`、`tool.selfcheck` 改为 `analyzed`（此处指本节完成而非 Ghidra 的 `ANALYZED` 属性）。保留级别为 **rebuildable**。

> 若后续装上 Ghidra，同一 `30_decompile` 目录会多出 `tools/ghidra/working/gnuture-audit-2026q4/binary/gnuture_e484c36c0613/{p.gpr, p.rep/}`，保留级别升级为 **required**。目录结构不变，只增加工作区。

## Step 4 `50_findings`

`findings.jsonl` 由人工撰写。本例只有一条结论，且是**否定结论**：

```json
{"claim_id":"F-001","claim":"gnuture 的动态符号表中不存在 system / execve / popen / execl 等命令执行与 strcpy / sprintf / gets 等不安全字符串函数","status":"inferred","confidence":0.7,"evidence":[{"type":"symbol","path":"binary/gnuture_e484c36c0613/20_static/out/imports.json","query":{"task":"scan-dynamic-symbols","params":{"names":["system","execve","popen","execl","strcpy","sprintf","gets","scanf"],"scope":"UND"},"tool":"readelf --dyn-syms -W"}}],"limitations":"仅覆盖动态符号可见的外部调用……","unknowns":["是否存在通过函数指针间接调用上述函数"],"thread":"TH1","ts":"2026-10-22T14:35:10+08:00"}
```

要点：

- `status` 为 `inferred` 而非 `confirmed`。符号表为空不足以证明行为不存在，因此 `confidence` 取 0.7，落在 [§6.4](../02-workflow/01-证据契约.md#64-status-与-confidence-的区间约束) 的 `0.5–0.8` 区间内。
- `evidence[].query` 记录了完整的查询参数。这条声明可以被别人重跑，不必相信作者的记录。
- 线索台账 `state/investigation.jsonl` 中 `blocked_by` 填写 `"tools: ghidra unavailable"`。**工具缺失是一种真实的阻塞状态**，记下来才知道下次从哪继续。

## 跳过的阶段

`orev.toml` 的 `stages` 声明本次执行范围，未声明的阶段**不建目录**：

```toml
stages = ["00_inbox", "20_static", "30_decompile", "50_findings"]
```

| 阶段 | 为何跳过 |
| --- | --- |
| `10_unpack` | 目标是单个 ELF，未经打包或加壳 |
| `15_patch` | 本次未修改目标 |
| `40_dynamic` | 无仿真与真机环境，见 `50_findings` 的 `next_actions` |
| `60_diff` | 只有一个版本的 target，无可对比的另一端 |
| `90_report` | 尚未结项 |

## 最终布局

```
cases/gnuture-audit-2026q4/
├── CASE.md
├── orev.toml
├── scripts/                          # 本案件专用脚本（git）
├── state/
│   ├── index.jsonl                   # 13 条产物索引
│   ├── investigation.jsonl           # TH1 一条线索
│   └── env.md                        # 环境快照（temp）
└── binary/gnuture_e484c36c0613/
    ├── 00_inbox/{gnuture, checksums.sha256, provenance.json, stage.yaml}
    ├── 20_static/{stage.yaml, out/, log/}
    ├── 30_decompile/{stage.yaml, out/, log/}
    └── 50_findings/{stage.yaml, out/, log/}
```

`index.jsonl` 的样例条目（`path` 相对案件根）：

```json
{"target_id":"gnuture_e484c36c0613","type":"binary","stage":"20_static","path":"binary/gnuture_e484c36c0613/20_static/out/headers.json","sha256":"…","bytes":705,"mime":"application/json","tool":"binutils@2.38","ts":"2026-10-22T14:35:10+08:00","retention":"rebuildable","tags":["thread=TH1"]}
```

## 本样例的边界

- `30_decompile` 只到**汇编层**，未产出伪代码。升级到 D1 需要 Ghidra 或同类工具。
- 未执行 `40_dynamic`，因此 F-001 缺少运行时一侧的证据。
- `sbom.spdx.json` 只有 1 个 package。**对单文件目标，SBOM 是 [§5](../01-layout/04-阶段产物清单.md) 清单里信息量最低的一项**，因为组件数恒为 1。
