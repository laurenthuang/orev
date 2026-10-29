// src/commands/init.rs
use crate::ops::init::{run_init, run_workspace_init};
use std::io;

/// 命令适配层：解析 `orev init` 的参数并交由 `ops::init` 业务引擎执行
pub fn execute(path: Option<String>, workspace: bool) -> io::Result<()> {
    if workspace {
        return run_workspace_init(path);
    }
    run_init(path)
}
