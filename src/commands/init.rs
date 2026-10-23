// src/commands/init.rs
use crate::ops::init::run_init;
use std::io;

/// 命令适配层：解析 `orev init` 的参数并交由 `ops::init` 业务引擎执行
pub fn execute(path: Option<String>, name: Option<String>) -> io::Result<()> {
    // 💡所有权转移（Move）
    // 这里把从命令行解析出来的 path、name 所有权直接传给 run_init 函数。
    run_init(path, name)
}
