use std::env;
use std::fs;
use std::path::Path;

/// 核心业务函数：在指定路径初始化二进制分析工作区
///
/// # 参数
/// - `path`: 可选的目标工作区路径。如果为 `None`，则使用当前进程目录。
/// - `name`: 可选的项目名称。如果为 `None`，则从路径或当前目录名自动推导。
/// - `target`: 需要导入到工作区 `samples/` 目录下的二进制文件路径列表。
pub fn run_init(path: Option<String>, name: Option<String>, target: Vec<String>) {
    // 1. 确定工作区的根目录
    let current_dir = env::current_dir().expect("Failed to get current directory");

    // 【Rust 概念：路径组合与解析】
    // 如果用户传了 path，就用 current_dir.join(path) 拼接成绝对或相对目标路径；否则直接用当前目录。
    let workspace_dir = match path {
        Some(ref p) => current_dir.join(p),
        None => current_dir.clone(),
    };

    // 如果目标工作区目录不存在，递归创建它
    if !workspace_dir.exists() {
        fs::create_dir_all(&workspace_dir).expect("Failed to create workspace directory");
    }

    // 2. 确定项目名称
    // 优先级：用户通过 --name 传入的显式名称 > 目标路径文件夹名 > 默认名称
    // 【Rust 基础概念：闭包与 unwrap_or_else】
    // 如果 name 是 None，就执行闭包(|| ...) 里的备用逻辑（获取当前文件夹名）。
    let project_name = name.unwrap_or_else(|| {
        workspace_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("binary_project")
            .to_string()
    });

    println!("Initializing binary analysis project: `{}`", project_name);
    println!("Workspace path: {}", workspace_dir.display());

    // 3. 创建标准逆向工作区子目录
    let dirs = ["samples", "notes", "patches", ".orev"];
    for dir in &dirs {
        let dir_path = workspace_dir.join(dir);
        if !dir_path.exists() {
            // fs::create_dir_all 可以递归创建多级目录
            fs::create_dir_all(&dir_path).expect("Failed to create subdirectory");
            println!("  [created] {}/", dir);
        } else {
            println!("  [exists]  {}/", dir);
        }
    }

    // 4. 导入并复制目标二进制文件到 samples/ 目录
    let mut copied_targets = Vec::new();
    let samples_dir = workspace_dir.join("samples");

    // 【Rust 基础概念：& 借用（Borrowing）】
    // &target 表示对 target 数组进行“只读借用”，不抢夺它的所有权，
    // 这样循环结束后 target 变量依然可以继续使用。
    for src_str in &target {
        let src_path = Path::new(src_str);
        if src_path.exists() {
            if let Some(file_name) = src_path.file_name() {
                let dest_path = samples_dir.join(file_name);
                fs::copy(src_path, &dest_path).expect("Failed to copy target binary");

                let rel_path = format!("samples/{}", file_name.to_string_lossy());
                println!("  [imported] {} -> {}", src_str, rel_path);
                copied_targets.push(rel_path);
            }
        } else {
            eprintln!("  [warning] Target file not found, skipped: {}", src_str);
        }
    }

    if copied_targets.is_empty() {
        copied_targets.push("samples/target_binary".to_string());
    }

    // 5. 生成默认的 orev.toml 配置文件
    let config_path = workspace_dir.join("orev.toml");
    if !config_path.exists() {
        let targets_toml_array = copied_targets
            .iter()
            .map(|t| format!("    \"{}\",", t))
            .collect::<Vec<_>>()
            .join("\n");

        let config_content = format!(
            r#"[project]
name = "{}"
version = "0.1.0"
authors = []

[workspace]
notes_dir = "notes"
patches_dir = "patches"
scripts_dir = "scripts"

[targets]
files = [
{}
]
"#,
            project_name, targets_toml_array
        );

        fs::write(&config_path, config_content).expect("Failed to write orev.toml");
        println!("  [created] orev.toml");
    } else {
        println!("  [exists]  orev.toml");
    }

    println!("\nSuccess! Workspace is ready.");
}
