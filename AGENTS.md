## Repo layout

```
orev/
├── Cargo.toml
└── src/
    ├── main.rs          # 极简入口：仅负责错误捕获与命令分发
    ├── cli.rs           # 命令行参数结构定义 (clap)
    ├── config.rs        # orev.toml 结构体与反序列化
    ├── ops/             # 【核心业务引擎】(参考 Cargo ops)
    │   ├── mod.rs
    │   ├── init.rs      # 工作区生成逻辑
    │   ├── status.rs    # 状态读取逻辑
    │   └── doctor.rs    # 体检、系统探查与版本检测纯逻辑
    └── commands/        # 【命令行适配层】(参考 Cargo commands)
        ├── mod.rs
        ├── init.rs      # 解析 init 参数 -> 调用 ops::init
        ├── status.rs    # 解析 status 参数 -> 调用 ops::status
        └── doctor.rs    # 解析 doctor 参数 -> 调用 ops::doctor
```

## Dev cli

