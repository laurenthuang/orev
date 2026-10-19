// src/cli.rs
use clap::{Parser, Subcommand};

// ==========================================================
// 顶层命令行参数结构体
// ==========================================================
// 【Rust 基础概念：属性宏 #[derive(...)]】
// 宏（Macro）是 Rust 编译时的代码生成器。#[derive(Parser)] 告诉编译器：
// “请自动为这个结构体生成解析命令行参数的底层代码，我不用自己写样板代码。”
#[derive(Parser)]
#[command(
    name = "orev",
    version, // <-- 这样写，clap 会在编译期自动读取 Cargo.toml 中的 version
    about = "A modern binary analysis case manager for reverse engineering and security analysis"
)]
// Rust 函数默认是私有的，如果希望在模块外部可见，需要加上 pub 关键字。
// 同样地，枚举和结构体也是默认私有的，所以我们在这里使用 pub modifier 使其在模块外部可见。
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

// ==========================================================
// 子命令枚举定义
// ==========================================================
// 【Rust 基础概念：枚举（Enum）】
// Enum 可以定义一组互斥的变体（Variant），非常适合用来做 CLI 的子命令设计。
#[derive(Subcommand)]
pub enum Commands {
    /// 初始化一个新的二进制分析案例工作区
    Init {
        // 【Rust 基础概念：Option<T>】
        // Rust 没有 null。如果某个参数是可选的（用户可能传，也可能不传），
        // 必须用 Option 包裹。它有两个值：Some(T) 代表有值，None 代表没有值。
        /// 可选的目标案例工作区路径（例如：`orev init ./crackme_dir`，不填则默认在当前目录）
        path: Option<String>,

        /// 可选的目标案例工作区名称（例如：`orev init ./crackme_dir -n my_project`，不填则默认使用目录名）
        #[arg(long)]
        name: Option<String>,

        // 目标文件路径列表（Vec<String> 类似于动态数组）
        /// 要分析的目标二进制文件路径（可以通过多次传入 -t 指定多个）
        #[arg(short, long)]
        target: Vec<String>,
    },

    /// 显示当前分析案例工作区的状态和配置信息
    Status,

    /// 对分析案例工作区完整性、系统环境以及外部逆向工具链进行健康诊断
    Doctor,
}
