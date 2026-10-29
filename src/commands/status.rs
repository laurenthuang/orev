// src/commands/status.rs
use crate::ops::status::run_status;

/// 命令适配层：接收 `orev status` 指令并交由 `ops::status` 业务引擎执行
pub fn execute() {
    // 这里未来可以增加命令层的参数预处理或前置校验
    run_status();
}
