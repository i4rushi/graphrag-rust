use anyhow::{Context, Result};
use std::path::Path;
use tokio::fs;

pub struct FileReader;

impl FileReader {
    pub async fn read_file(path: &Path) -> Result<String> {
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "txt" | "md" => {
                let content = fs::read_to_string(path)
                    .await
                    .context(format!("Failed to read file: {:?}", path))?;
                Ok(content)
            }
            _ => anyhow::bail!("Unsupported file format: {}", extension),
        }
    }

    pub async fn read_directory(dir: &Path) -> Result<Vec<(String, String)>> {
        let mut files = Vec::new();
        
        let mut entries = fs::read_dir(dir).await?;
        
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "txt" || ext == "md" {
                        let content = Self::read_file(&path).await?;
                        let path_str = path.to_string_lossy().to_string();
                        files.push((path_str, content));
                    }
                }
            }
        }
        
        Ok(files)
    }
}