# Instructions for agent

(Analysis Case) Workspace managemnt tool for reverse engineer and security analysis on binary, firmware and malware. 

## Repo layout

```
orev/
├── Cargo.toml
├── tests/               # test cases and fixtures
└── src/
    ├── main.rs          # 极简入口：仅负责错误捕获与命令分发
    ├── cli.rs           # 命令行参数结构定义 (clap)
    ├── config.rs        # orev.toml 结构体与反序列化
    ├── ops/             # 【核心业务引擎】(参考 Cargo ops)
    │   ├── mod.rs
    │   ├── init.rs      # 工作区生成逻辑
    │   ├── target.rs    # 涉及被分析的目标的逻辑
    │   ├── status.rs    # 状态读取逻辑
    │   └── doctor.rs    # 体检、系统探查与版本检测纯逻辑
    └── commands/        # 【命令行适配层】(参考 Cargo commands)
        ├── mod.rs
        ├── init.rs      # 解析 init 参数 -> 调用 ops::init
        ├── target.rs    # 解析 target 子命令及参数 -> 调用 ops::target
        ├── status.rs    # 解析 status 参数 -> 调用 ops::status
        └── doctor.rs    # 解析 doctor 参数 -> 调用 ops::doctor
```

## Dev cli

- `cargo fmt`
- `cargo clippy`

