use std::path::Path;
use tauri::webview::Cookie;

#[cfg(windows)]
fn protect(data: &[u8], decrypt: bool) -> Result<Vec<u8>, String> {
    use windows::{
        Win32::{
            Foundation::{HLOCAL, LocalFree},
            Security::Cryptography::{
                CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
            },
        },
        core::PCWSTR,
    };
    let input = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).map_err(|_| "会话文件过大")?,
        pbData: data.as_ptr().cast_mut(),
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    // SAFETY: Both blobs live across the synchronous call. Windows treats input
    // bytes as read-only and allocates output with LocalAlloc. We copy it before
    // freeing exactly once with LocalFree. No pointers escape this function.
    unsafe {
        let result = if decrypt {
            CryptUnprotectData(
                &input,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        } else {
            CryptProtectData(
                &input,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output,
            )
        };
        result.map_err(|_| "无法访问本机加密会话，请重新登录")?;
        let bytes = if output.cbData == 0 {
            Vec::new()
        } else {
            std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec()
        };
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn protect(_: &[u8], _: bool) -> Result<Vec<u8>, String> {
    Err("当前加密会话存储仅支持 Windows".into())
}

pub fn restore(path: &Path) -> Result<Vec<Cookie<'static>>, String> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read(path).map_err(|_| "无法读取已保存的会话")?;
    if data.len() > 1_000_000 {
        return Err("会话文件损坏，请重新登录".into());
    }
    let entries: Vec<String> =
        serde_json::from_slice(&protect(&data, true)?).map_err(|_| "会话文件损坏")?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| Cookie::parse(entry).ok())
        .filter(|cookie| {
            let domain = cookie.domain().unwrap_or("").trim_start_matches('.');
            (domain == "douyin.com" || domain.ends_with(".douyin.com"))
                && cookie.expires_datetime().is_none_or(|expires| {
                    expires.unix_timestamp()
                        > std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64
                })
        })
        .collect())
}

pub fn save(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().ok_or("无效的会话保存位置")?;
    std::fs::create_dir_all(parent).map_err(|_| "无法创建会话目录")?;
    let encrypted = protect(bytes, false)?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, encrypted).map_err(|_| "无法保存加密会话")?;
    std::fs::rename(&temporary, path).map_err(|_| "无法更新加密会话".to_owned())
}

#[cfg(all(test, windows))]
mod tests {
    #[test]
    fn encryption_round_trip_and_tamper_detection() {
        let plain = b"test cookie, not a real credential";
        let mut encrypted = super::protect(plain, false).unwrap();
        assert_ne!(encrypted, plain);
        assert_eq!(super::protect(&encrypted, true).unwrap(), plain);
        encrypted[20] ^= 0xff;
        assert!(super::protect(&encrypted, true).is_err());
    }
}
