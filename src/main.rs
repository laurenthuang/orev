// ==========================================================
// 【Rust 基础概念：外部依赖导入（Crates）】
// 在 Cargo.toml 中配置好依赖后，这里使用 use 关键字把库引入当前作用域。
// ==========================================================
// mod 声明外部模块，引入后可以使用模块中的公有内容。不同于 python 的 import 机制，Rust 的 mod 是在编译期静态解析的, 并且 mod 和 use 是不同的概念。mod 声明模块，use 引入模块中的内容。这样设计，是为了在编译期就能确定模块结构，从而提高编译器优化能力和程序执行效率。
mod cli;
mod commands;
mod config;
mod ops;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    // 解析命令行输入
    let cli = Cli::parse();

    // 【Rust 基础概念：match 模式匹配】
    // Rust 的 match 非常严格，必须穷尽（Cover）枚举的所有可能情况（分支），
    // 少写一个都会导致编译不通过，从而彻底避免漏掉分支的 Bug。
    match cli.command {
        // 子命令：init
        Commands::Init { path, name, target } => {
            commands::init::execute(path, name, target);
        }

        // 子命令：status
        Commands::Status => {
            commands::status::execute();
        }

        // 子命令：doctor
        Commands::Doctor => {
            commands::doctor::execute();
        }
    }
}
