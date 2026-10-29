use std::io;

pub fn add(name: &str) -> io::Result<()> {
    crate::ops::case::run_add(name)
}
