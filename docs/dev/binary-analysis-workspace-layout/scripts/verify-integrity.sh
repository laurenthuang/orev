# 二进制分析项目目录规范 v1.7 —— ../03-operations/01-保留分级与清理.md
# permanent 必须建基线，required 建议建基线

# 生成基线（permanent 必须，required 建议）
orev verify --snapshot

# 校验（每次恢复备份后、每季度）
orev verify --check
