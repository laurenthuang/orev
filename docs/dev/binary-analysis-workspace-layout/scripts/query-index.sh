# 二进制分析项目目录规范 v1.7 —— ../03-operations/02-索引与查询.md
# state/index.jsonl 的四条常用查询

# 体积 Top 20
jq -s 'sort_by(-.bytes) | .[0:20] | .[] | "\(.bytes)\t\(.path)"' -r state/index.jsonl

# 全文检索
rg -l "AES_set_encrypt_key" --glob "*/30_decompile/out/*"

# 需要补证据的弱结论
jq -c 'select(.status=="assumed")' firmware/*/50_findings/out/findings.jsonl

# 按保留级别统计（按危险度排序，而非字母序）
for lvl in permanent required rebuildable temp; do
  printf "%-12s %s\n" "$lvl" "$(jq -r --arg l "$lvl" 'select(.retention==$l)' state/index.jsonl | wc -l)"
done
