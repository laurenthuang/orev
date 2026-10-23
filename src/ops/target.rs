// src/ops/target.rs
use sha2::{Digest, Sha256};
use std::env;
use std::fmt::Write;
use std::fs;
use std::path::Path;

// 💡条件编译属性
// #[cfg(...)] 是 Rust 的内置属性，用于条件编译。
// #[cfg(unix)] 确保了以下代码（引入 Unix 特有的文件权限扩展）仅在 Unix 系统上才会参与编译。
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

/// Target 流水线类型受控枚举
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TargetType {
    Binary,
    Firmware,
    Sample,
}

impl TargetType {
    // 💡静态字符串切片
    // 返回静态字符串切片 `&'static str` ，硬编码于 .rodata 只读数据段，无堆分配开销，生命周期贯穿程序始末。
    pub fn as_str(&self) -> &'static str {
        match self {
            TargetType::Binary => "binary",
            TargetType::Firmware => "firmware",
            TargetType::Sample => "sample",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "firmware" | "fw" => TargetType::Firmware,
            "sample" | "malware" => TargetType::Sample,
            _ => TargetType::Binary,
        }
    }
}

// 💡借用字符串切片
// 使用借用字符串切片 `&'a str` 代替 `String` 可以有效避免不必要的内存分配。
// 💡显示生命周期
// 显示生命周期标记 `'a` 告知编译器：`TargetAddOptions` 结构体实例的存活时间
// 不能长于传入的具体字符串引用，保证了零内存分配且无悬挂指针 (Dangling Pointers)。
/// 目标导入元数据选项结构体
pub struct TargetAddOptions<'a> {
    pub target_type: TargetType,
    pub custom_name: Option<&'a str>,
    pub declared_version: Option<&'a str>,
}

// 依据 v1.6 规范 §3.1 将文件名进行 Slug 化转换：
// 1. 去掉扩展名
// 2. 转小写
// 3. 空白（含连续）→ `-`
// 4. 删除非法字符与控制字符：`/ \ : * ? " < > |`
// 5. 首尾 `-` `_` 修剪
// 6. 超 48 字符截断（取头部）
pub fn slugify_filename(file_name: &str) -> String {
    // 1. 去掉扩展名
    let base_name = Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);

    // 2. 转小写并替换/过滤非法字符
    let mut slug = String::new();
    let mut prev_was_space = false;

    for c in base_name.chars() {
        if c.is_ascii_whitespace() {
            if !prev_was_space {
                slug.push('-');
                prev_was_space = true;
            }
        } else {
            prev_was_space = false;
            match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => {
                    // 过滤非法字符
                }
                _ if c.is_control() => {
                    // 过滤控制字符
                }
                _ => {
                    slug.push(c.to_ascii_lowercase());
                }
            }
        }
    }

    // 5. 首尾 `-` `_` 修剪
    let trimmed = slug.trim_matches(|c| c == '-' || c == '_');

    // 如果 slug 变空，回退为 "target"
    let safe_slug = if trimmed.is_empty() {
        "target"
    } else {
        trimmed
    };

    // 6. 超 48 字符截断
    if safe_slug.chars().count() > 48 {
        safe_slug.chars().take(48).collect()
    } else {
        safe_slug.to_string()
    }
}

/// 专职业务函数：将外部目标引入到工作区 00_inbox 并创建完整的 Target ID 与 7 阶段槽位
/// 依据 v1.6 规范 §3 与 §5 契约
pub fn run_target_add(target_path: &str, options: TargetAddOptions) -> Option<String> {
    let src_path = Path::new(target_path);
    if !src_path.exists() {
        eprintln!("Error: Target file not found: {}", target_path);
        return None;
    }

    let abs_src_path = fs::canonicalize(src_path).unwrap_or_else(|_| src_path.to_path_buf());
    let raw_file_name = src_path.file_name()?.to_string_lossy();
    let file_bytes = match fs::read(&abs_src_path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("Error: Failed to read target file `{}`: {}", target_path, e);
            return None;
        }
    };

    // 1. 计算完整 SHA256 与前 12 位短哈希 (§3.1)
    let digest = Sha256::digest(&file_bytes);
    // 将 SHA256 摘要转换为十六进制字符串
    let mut full_sha256 = String::with_capacity(64);
    for byte in digest {
        write!(&mut full_sha256, "{byte:02x}").unwrap();
    }
    let short_sha256 = &full_sha256[..12];

    // 2. 构建符合 v1.6 规范 §3.1 的 target-id: <文件名slug>_<sha256[:12]>
    let file_slug = options
        .custom_name
        .map(slugify_filename)
        .unwrap_or_else(|| slugify_filename(&raw_file_name));

    let target_id = format!("{}_{}", file_slug, short_sha256);

    let workspace_dir = env::current_dir().ok()?;
    let pipeline_type = options.target_type.as_str();
    let target_base = workspace_dir.join(pipeline_type).join(&target_id);

    // 如果是恶意样本，追加 chmod 700 物理隔离 (§13)
    if options.target_type == TargetType::Sample {
        set_sample_isolation_permissions(&target_base);
    }

    // 3. 自动初始化标准 7 阶段槽位 ( out / log) (§4.1)
    let stages = [
        "00_inbox",
        "10_unpack",
        "15_patch",
        "20_static",
        "30_decompile",
        "40_dynamic",
        "50_findings",
    ];
    for stage in stages {
        let stage_path = target_base.join(stage);
        let _ = fs::create_dir_all(stage_path.join("out"));
        let _ = fs::create_dir_all(stage_path.join("log"));
    }

    // 4. 落地 00_inbox 产物实体与元数据 (§5)
    let inbox_dir = target_base.join("00_inbox"); // full_sha256 不再作为子目录
    let _ = fs::create_dir_all(&inbox_dir);

    // 写入 payload.bin
    let payload_path = inbox_dir.join("payload.bin");
    if let Err(e) = fs::write(&payload_path, &file_bytes) {
        eprintln!("Error: Failed to write payload file: {}", e);
        return None;
    }

    // 获取当前的 UTC 时间并格式化为标准 ISO 8601 字符串
    let imported_at = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    // 写入 provenance.json (schema: orev/provenance@v1)
    let declared_ver = options.declared_version.unwrap_or("unknown");
    let provenance_json = format!(
        r#"{{
  "schema": "orev/provenance@v1",
  "original_path": "{}",
  "imported_at": "{}",
  "bytes": {},
  "declared_version": "{}"
}}
"#,
        abs_src_path.display(),
        imported_at,
        file_bytes.len(),
        declared_ver
    );
    let _ = fs::write(inbox_dir.join("provenance.json"), provenance_json);

    // 写入 checksums.sha256
    let checksums_txt = format!("{}  payload.bin\n", full_sha256);
    let _ = fs::write(inbox_dir.join("checksums.sha256"), checksums_txt);

    let relative_ref = format!("{}/{}/00_inbox/payload.bin", pipeline_type, target_id);
    println!("  [target added] {} -> {}", target_path, relative_ref);
    println!("  [provenance] 00_inbox/provenance.json generated.");

    Some(relative_ref)
}

// 设置样本目录 chmod 700 物理隔离 (仅 Unix 效能，满足 §13 规范)
fn set_sample_isolation_permissions(path: &Path) {
    #[cfg(unix)]
    {
        if path.exists() {
            let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o700));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
