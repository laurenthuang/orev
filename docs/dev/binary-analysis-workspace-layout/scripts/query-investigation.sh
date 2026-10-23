# 二进制分析项目目录规范 v1.7 —— ../02-workflow/02-调查线索.md
# state/investigation.jsonl 的三条常用查询

# 横轴：某目标全部产物
jq -c 'select(.target_id | test("nginx-1"))' state/index.jsonl

# 纵轴：某线索关联的全部产物
jq -c 'select(.tags | index("thread=TH2"))' state/index.jsonl

# 重建线索 → 结论的反向索引
jq -r '"\(.thread)\t\(.claim_id)"' */50_findings/out/findings.jsonl
