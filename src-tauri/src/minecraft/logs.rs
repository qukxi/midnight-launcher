//! Bounded reads of this launcher's redacted game output only.
use std::path::Path;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
const LIMIT: u64 = 256 * 1024;
pub async fn read(data: &Path, instance: &str) -> Result<String, String> {
    let relative = super::install::safe_path(instance)?;
    if relative.components().count() != 1 {
        return Err("LOG_INVALID_INSTANCE".into());
    }
    let path = data.join("logs").join(format!("minecraft-{instance}.log"));
    let mut file = match tokio::fs::File::open(path).await {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
        Err(_) => return Err("LOG_READ_FAILED".into()),
    };
    let len = file.metadata().await.map_err(|_| "LOG_READ_FAILED")?.len();
    let start = len.saturating_sub(LIMIT);
    file.seek(std::io::SeekFrom::Start(start))
        .await
        .map_err(|_| "LOG_READ_FAILED")?;
    let mut bytes = Vec::new();
    file.take(LIMIT)
        .read_to_end(&mut bytes)
        .await
        .map_err(|_| "LOG_READ_FAILED")?;
    if start > 0 {
        if let Some(end) = bytes.iter().position(|b| *b == b'\n') {
            bytes.drain(..=end);
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn bounds_output_and_rejects_traversal() {
        let root = std::env::temp_dir().join(format!("midnight-logs-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(root.join("logs")).await.unwrap();
        assert_eq!(read(&root, "missing").await.unwrap(), "");
        assert!(read(&root, "../secret").await.is_err());
        let text = format!("{}\nlast line\n", "x".repeat(LIMIT as usize + 100));
        tokio::fs::write(root.join("logs/minecraft-test.log"), text)
            .await
            .unwrap();
        assert_eq!(read(&root, "test").await.unwrap(), "last line\n");
        tokio::fs::remove_dir_all(root).await.unwrap();
    }
}
