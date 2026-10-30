// src/cli.rs
use clap::{Parser, Subcommand};
use std::ffi::OsString;
use std::path::PathBuf;

// ==========================================================
// 顶层命令行参数结构体
// ==========================================================
// 💡派生属性
// #[derive(...)] 是一个属性，用于调用编译时的代码生成器（宏）。这里的 Parser 是一个派生宏，它告诉编译器：
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
// 💡枚举（Enum）
// Enum 可以定义一组互斥的变体（Variant），非常适合用来做 CLI 的子命令设计。
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 初始化独立案件或多案件工作区骨架
    Init {
        // 💡Option<T>
        // Rust 没有 null。如果某个参数是可选的（用户可能传，也可能不传），
        // 必须用 Option 包裹。它有两个值：Some(T) 代表有值，None 代表没有值。
        /// 目标目录路径，不填则使用当前目录
        path: Option<String>,

        /// 创建多案件工作区及空案件登记表
        #[arg(long)]
        workspace: bool,
    },

    /// 管理工作区中的案件
    Case {
        #[command(subcommand)]
        command: CaseCommands,
    },

    /// 将样本复制到案件根目录下的 00_intake 子目录
    Sample {
        #[command(subcommand)]
        command: SampleCommands,
    },

    /// 显示当前案例或多案件工作区的状态和配置信息
    Status,

    /// 对案例或多案件工作区目录、系统环境以及外部逆向工具链进行健康诊断
    Doctor,

    /// 在 PTY 中执行工具并在当前目录记录输出与溯源信息（Linux）
    #[command(after_help = "参数隔离：orev run [OPTIONS] -- <TOOL> [ARGS]...\n\
        `--` 结束 orev 参数解析且不传给工具；工具名之后的所有参数原样传递。\n\
        示例：orev run --no-version-probe -- readelf --help")]
    Run {
        /// 手动指定工具版本，不自动探测
        #[arg(
            long,
            conflicts_with = "no_version_probe",
            value_parser = clap::builder::NonEmptyStringValueParser::new()
        )]
        tool_version: Option<String>,

        /// 不额外执行版本探测命令
        #[arg(long)]
        no_version_probe: bool,

        /// 工具及其参数；推荐用 -- 与 orev 参数隔离
        #[arg(required = true, trailing_var_arg = true)]
        command: Vec<OsString>,
    },
}

#[derive(Subcommand, Debug)]
pub enum CaseCommands {
    /// 新增案件骨架；独立案件会先转换为多案件工作区
    Add {
        /// 必填案件名称，同时用作 cases/ 下的单级目录名
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum SampleCommands {
    /// 将文件和目录复制到 00_intake，并记录其 SHA-256
    Ingest {
        /// 指定案件名称；省略时使用最近的案件
        #[arg(long, value_name = "NAME")]
        case: Option<String>,

        /// 源文件和目录；名称以 - 开头时，请在其前使用 --
        #[arg(required = true, num_args = 1..)]
        paths: Vec<PathBuf>,
    },
}
