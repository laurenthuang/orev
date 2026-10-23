// src/commands/target.rs
use crate::ops::target::{TargetAddOptions, TargetType, run_target_add};

pub fn handle_target_add(
    path: &str,
    target_type_str: &str,
    name: Option<&str>,
    version: Option<&str>,
) {
    let target_type = TargetType::from_str(target_type_str);
    let options = TargetAddOptions {
        target_type,
        custom_name: name,
        declared_version: version,
    };

    if run_target_add(path, options).is_some() {
        println!("Successfully imported target into case workspace (v1.6 spec).");
    } else {
        eprintln!("Failed to import target.");
    }
}
