# 二进制分析项目目录规范 v1.7 —— ../03-operations/01-保留分级与清理.md
# 删除 rebuildable 之前必须确认可重建

# 只清 temp，永远安全
rm -rf .cache

# 清 rebuildable 前：确认每个待删目录的 stage.yaml 存在、status=ok、tool.cmd 完整
fd stage.yaml cases/ | xargs grep -l "status: ok"
