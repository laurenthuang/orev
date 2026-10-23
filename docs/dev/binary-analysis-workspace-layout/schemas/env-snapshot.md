# env snapshot
generated_at: 2026-10-14T14:20:00+08:00
protocol_version: 1
ttl_days: 7

## host
os: Ubuntu 22.04 / linux / x86_64
cores: 16
mem_gb: 64

## toolchain
ghidra: 12.1.4 (/opt/ghidra_12.1.4)
radare2: 6.2.4
rizin: 0.9.1
python: 3.11.0
node: 22.13.0

## env vars (sanitized)
REA_GHIDRA_STARTUP_TIMEOUT_MS: 120000
API_KEY: SET

## notes
- Ghidra 12.1.4 的 JDK 要求以随版 Getting Started 为准（示例环境 JDK 25）；headless 脚本为 support/analyzeHeadless
