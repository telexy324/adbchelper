use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::Utc;

pub fn log_path(base_dir: &Path) -> PathBuf {
    base_dir.join("adbchelper.log")
}

pub fn append_log(base_dir: &Path, level: &str, context: &str, message: &str) -> Result<(), std::io::Error> {
    fs::create_dir_all(base_dir)?;
    let path = log_path(base_dir);
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(
        file,
        "{} [{}] {} :: {}",
        Utc::now().to_rfc3339(),
        level,
        context,
        message
    )?;
    Ok(())
}
