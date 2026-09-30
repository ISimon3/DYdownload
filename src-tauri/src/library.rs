use serde::Serialize;
use std::path::{Path, PathBuf};

pub fn author_folder(name: &str, id: &str, work_id: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(40)
        .collect();
    let name = name.trim().trim_end_matches(['.', ' ']);
    let id: String = id
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(100)
        .collect();
    format!(
        "{}__{}",
        if name.is_empty() {
            "未知作者"
        } else {
            name
        },
        if id.is_empty() { work_id } else { &id }
    )
}

#[derive(Serialize)]
pub struct SavedFile {
    name: String,
    path: String,
    kind: &'static str,
    size: u64,
}
#[derive(Serialize)]
pub struct SavedAuthor {
    name: String,
    path: String,
    files: Vec<SavedFile>,
}

pub fn media_kind(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v" => Some("视频"),
        "jpg" | "jpeg" | "png" | "webp" | "bmp" | "avif" => Some("图片"),
        _ => None,
    }
}

fn files_in(dir: &Path) -> Result<Vec<SavedFile>, String> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(dir).map_err(|e| format!("无法读取 {}：{e}", dir.display()))?
    {
        let entry = entry.map_err(super::err)?;
        if !entry.file_type().map_err(super::err)?.is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(kind) = media_kind(&path) {
            files.push(SavedFile {
                name: entry.file_name().to_string_lossy().into(),
                path: path.to_string_lossy().into(),
                kind,
                size: entry.metadata().map_err(super::err)?.len(),
            });
        }
    }
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

pub fn scan(root: &Path) -> Result<Vec<SavedAuthor>, String> {
    if !root.is_absolute() {
        return Err("请选择绝对路径作为保存目录".into());
    }
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("无法读取保存目录：{e}")),
    };
    let mut authors = Vec::new();
    for entry in entries {
        let entry = entry.map_err(super::err)?;
        if !entry.file_type().map_err(super::err)?.is_dir() {
            continue;
        }
        let path = entry.path();
        authors.push(SavedAuthor {
            name: entry.file_name().to_string_lossy().into(),
            files: files_in(&path)?,
            path: path.to_string_lossy().into(),
        });
    }
    authors.sort_by(|a, b| a.name.cmp(&b.name));
    let legacy = files_in(root)?;
    if !legacy.is_empty() {
        authors.insert(
            0,
            SavedAuthor {
                name: "未分类作品（旧版保存）".into(),
                path: root.to_string_lossy().into(),
                files: legacy,
            },
        );
    }
    Ok(authors)
}

pub fn checked_path(root: &Path, path: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(super::err)?;
    let path = path.canonicalize().map_err(super::err)?;
    if !path.starts_with(&root)
        || !(path.is_dir() || (path.is_file() && media_kind(&path).is_some()))
    {
        return Err("只能打开保存目录内的文件夹、视频或图片".into());
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn safe_author_names() {
        assert_eq!(author_folder(" ../A:* ", "abc", "1"), ".._A____abc");
        assert_ne!(
            author_folder("同名", "a", "1"),
            author_folder("同名", "b", "2")
        );
        assert_eq!(author_folder("", "", "123"), "未知作者__123");
    }
    #[test]
    fn scans_media_and_legacy_but_excludes_partial_files() {
        let root = std::env::temp_dir().join(format!("shiguang-library-{}", std::process::id()));
        std::fs::create_dir_all(root.join("作者__123")).unwrap();
        std::fs::write(root.join("old.mp4"), b"old").unwrap();
        std::fs::write(root.join("作者__123/1.jpg"), b"image").unwrap();
        std::fs::write(root.join("作者__123/2.mp4.part"), b"partial").unwrap();
        let result = scan(&root).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[1].files.len(), 1);
        assert!(checked_path(&root, &root.join("作者__123/1.jpg")).is_ok());
        assert!(checked_path(&root, &root.join("作者__123/2.mp4.part")).is_err());
        assert!(checked_path(&root, root.parent().unwrap()).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
