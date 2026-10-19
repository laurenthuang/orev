// src/ops/init.rs
use std::env;
use std::fs;
use std::path::Path;

/// 核心业务函数：在指定路径初始化二进制分析案例工作区
///
/// # 参数
/// - `path`: 可选的目标案例工作区路径。如果为 `None`，则使用当前进程目录。
/// - `name`: 可选的项目名称。如果为 `None`，则从路径或当前目录名自动推导。
/// - `target`: 需要导入到案例工作区 `samples/` 目录下的二进制文件路径列表。
pub fn run_init(path: Option<String>, name: Option<String>, target: Vec<String>) {
    // 1. 确定案例工作区的根目录
    let current_dir = env::current_dir().expect("Failed to get current directory");

    // 【Rust 概念：路径组合与解析】
    // 如果用户传了 path，就用 current_dir.join(path) 拼接成绝对或相对目标路径；否则直接用当前目录。
    let workspace_dir = match path {
        Some(ref p) => current_dir.join(p),
        None => current_dir.clone(),
    };

    // 如果目标案例工作区目录不存在，递归创建它
    if !workspace_dir.exists() {
        fs::create_dir_all(&workspace_dir).expect("Failed to create case workspace directory");
    }

    // 2. 确定案例名称
    // 优先级：用户通过 --name 传入的显式名称 > 目标路径文件夹名 > 默认名称
    // 【Rust 基础概念：闭包与 unwrap_or_else】
    // 如果 name 是 None，就执行闭包(|| ...) 里的备用逻辑（获取当前文件夹名）。
    let case_name = name.unwrap_or_else(|| {
        workspace_dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("binary_project")
            .to_string()
    });

    println!("Initializing binary analysis project: `{}`", case_name);
    println!("Case workspace path: {}", workspace_dir.display());

    // 3. 创建标准逆向工作区子目录
    let dirs = ["samples", "notes", "patches", "reports", ".orev"];
    for dir in &dirs {
        let dir_path = workspace_dir.join(dir);
        if !dir_path.exists() {
            // fs::create_dir_all 可以递归创建多级目录
            if let Err(e) = fs::create_dir_all(&dir_path) {
                eprintln!("Error: Failed to create directory `{}/`: {}", dir, e);
                return;
            }
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
            r#"[case]
name = "{}"
version = "0.1.0"
analysts = []

[workspace]
notes_dir = "notes"
patches_dir = "patches"
reports_dir = "reports"
scripts_dir = "scripts"

[targets]
files = [
{}
]
"#,
            case_name, targets_toml_array
        );

        if let Err(e) = fs::write(&config_path, config_content) {
            eprintln!("Error: Failed to write `orev.toml`: {}", e);
            return;
        }
        println!("  [created] orev.toml");
    } else {
        println!("  [exists]  orev.toml");
    }

    // 6. 生成逆向专属的 .gitignore
    let gitignore_path = workspace_dir.join(".gitignore");
    if !gitignore_path.exists() {
        let gitignore_content = r#"# Orev Case Workspace Ignores
.orev/
*.dmp
*.log
*.tmp
ghidra_projects/
.ida/
"#;
        let _ = fs::write(&gitignore_path, gitignore_content);
        println!("  [created] .gitignore");
    }

    // 7. 生成标准的案例模板笔记 (CASE_NOTES.md)
    let notes_template_path = workspace_dir.join("notes").join("CASE_NOTES.md");
    if !notes_template_path.exists() {
        let template_content = format!(
            r#"# Analysis Case: {}

## 1. Case Metadata
- **Status**: In Progress
- **Analysts**: 
- **Created Date**: 

## 2. Target Samples & Hashes
| Sample Path | MD5 / SHA256 | Architecture | Description |
| :--- | :--- | :--- | :--- |
| `samples/` | TBD | x86_64 / ARM | Primary Target |

## 3. Analysis Milestones & Findings
- [ ] Sample Triage & Static Assessment
- [ ] Decompilation & Symbol Recovery
- [ ] Dynamic Debugging & Unpacking
- [ ] Key Algorithms / C2 Protocol Analysis

## 4. Indicators of Compromise (IOCs)
- **C2 Servers**: 
- **Dropped Files / Reg Keys**: 

## 5. Notes & Function References
*Add analysis notes and reverse engineering progress here...*
"#,
            case_name
        );
        let _ = fs::write(&notes_template_path, template_content);
        println!("  [created] notes/CASE_NOTES.md");
    }

    println!("\nSuccess! Case workspace is ready.");
}
