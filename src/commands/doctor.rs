use crate::ops::doctor::run_doctor;
use std::io;

pub fn execute() -> io::Result<()> {
    // 这里未来可以加一些命令层的预处理或参数校验
    run_doctor()
}
