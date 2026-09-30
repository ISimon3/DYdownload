use super::{Store, Work, err, local, media_url};
use futures_util::StreamExt;
use serde::Serialize;
use std::{
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::Duration,
};
use tauri::{Manager, WebviewWindow};
use tokio::io::AsyncWriteExt;

fn work(app: &tauri::AppHandle, id: &str) -> Result<Work, String> {
    app.state::<Store>()
        .works
        .lock()
        .map_err(err)?
        .get(id)
        .cloned()
        .ok_or("作品数据已失效，请重新查询".into())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    image_count: usize,
    media_kinds: Vec<String>,
    has_audio: bool,
    has_music: bool,
    music_title: String,
}

#[tauri::command]
pub fn work_details(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<Details, String> {
    local(&window)?;
    let work = work(&app, &id)?;
    Ok(Details {
        image_count: if work.kind == "图集" {
            work.urls.len()
        } else {
            0
        },
        media_kinds: work.media_kinds.clone(),
        has_audio: !work.music_url.is_empty()
            || work.media_kinds.iter().any(|kind| kind == "video"),
        has_music: !work.music_url.is_empty(),
        music_title: work.music_title,
    })
}

fn source(work: &Work, kind: &str, index: usize) -> Result<String, String> {
    let url = match kind {
        "image" | "video"
            if work
                .media_kinds
                .get(index)
                .is_some_and(|value| value == kind) =>
        {
            work.urls.get(index)
        }
        "audio" if !work.music_url.is_empty() => Some(&work.music_url),
        "audio" => work
            .media_kinds
            .iter()
            .position(|kind| kind == "video")
            .and_then(|index| work.urls.get(index)),
        _ => None,
    }
    .ok_or("此作品没有可用的媒体地址")?;
    if !media_url(url) {
        return Err("无效媒体地址".into());
    }
    Ok(url.clone())
}

async fn cached(app: &tauri::AppHandle, url: &str) -> Result<PathBuf, String> {
    static GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(3);
    let _permit = GATE.acquire().await.map_err(err)?;
    let directory = app.path().app_cache_dir().map_err(err)?.join("playback");
    tokio::fs::create_dir_all(&directory).await.map_err(err)?;
    let mut key = std::collections::hash_map::DefaultHasher::new();
    url.hash(&mut key);
    let stem = format!("{:x}", key.finish());
    for ext in [
        "jpg", "png", "webp", "mp4", "mp3", "m4a", "aac", "ogg", "bin",
    ] {
        let path = directory.join(format!("{stem}.{ext}"));
        if tokio::fs::try_exists(&path).await.map_err(err)? {
            return Ok(path);
        }
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(180))
        .connect_timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() > 8 || !media_url(attempt.url().as_str()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(err)?;
    let response = client
        .get(url)
        .header("Referer", "https://www.douyin.com/")
        .header("User-Agent", "Mozilla/5.0")
        .send()
        .await
        .map_err(|_| "媒体加载失败，请检查网络后重试")?;
    if !response.status().is_success() {
        return Err(format!(
            "媒体加载失败（{}），请重新查询作者后重试",
            response.status().as_u16()
        ));
    }
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let ext = match mime.split(';').next().unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "audio/mpeg" => "mp3",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "application/octet-stream" => "bin",
        value if value.starts_with("video/") => "mp4",
        _ => return Err("服务器未返回支持的媒体格式".into()),
    };
    let limit = 512 * 1024 * 1024;
    let expected = response.content_length();
    if expected.is_some_and(|size| size > limit) {
        return Err("媒体超过 512 MB，请先下载后在本地播放".into());
    }
    let path = directory.join(format!("{stem}.{ext}"));
    let (temporary, mut file) = super::create_download_temp(&path).await?;
    let result = async {
        let mut stream = response.bytes_stream();
        let mut size = 0;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "媒体传输中断，请重试")?;
            size += chunk.len() as u64;
            if size > limit {
                return Err("媒体超过预览大小限制".to_owned());
            }
            file.write_all(&chunk).await.map_err(err)?;
        }
        if size == 0 || expected.is_some_and(|total| total != size) {
            return Err("媒体文件不完整，请重试".into());
        }
        file.flush().await.map_err(err)
    }
    .await;
    drop(file);
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(e);
    }
    // These are disposable cache entries for the same URL, so replacement is safe.
    if let Err(e) = tokio::fs::rename(&temporary, &path).await {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(err(e));
    }
    Ok(path)
}

#[tauri::command]
pub async fn prepare_work_media(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
    kind: String,
    index: usize,
) -> Result<String, String> {
    local(&window)?;
    let url = source(&work(&app, &id)?, &kind, index)?;
    let path = cached(&app, &url).await?;
    app.asset_protocol_scope().allow_file(&path).map_err(err)?;
    Ok(path.to_string_lossy().into())
}

#[tauri::command]
pub async fn download_work_audio(
    app: tauri::AppHandle,
    window: WebviewWindow,
    id: String,
    directory: String,
) -> Result<String, String> {
    local(&window)?;
    let work = work(&app, &id)?;
    let root = Path::new(&directory);
    if !root.is_absolute() {
        return Err("请先在偏好设置中选择保存目录".into());
    }
    let dir = root.join(super::library::author_folder(
        &work.author,
        &work.author_id,
        &work.id,
    ));
    tokio::fs::create_dir_all(&dir).await.map_err(err)?;
    let target = dir.join(format!("{}_audio.mp3", work.id));
    if tokio::fs::try_exists(&target).await.map_err(err)? {
        return Ok("音频已存在，未重复下载".into());
    }
    let input = cached(&app, &source(&work, "audio", 0)?).await?;
    let (temporary, file) = super::create_download_temp(&target).await?;
    drop(file);
    let output = temporary.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        super::media::extract_audio(&input, &output, &AtomicBool::new(false))
    })
    .await
    .map_err(err);
    let result = async {
        result??;
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&temporary)
            .await
            .map_err(err)?;
        file.sync_all().await.map_err(err)?;
        drop(file);
        super::publish_download(&temporary, &target)
            .await
            .map_err(err)
    }
    .await;
    if let Err(e) = result {
        let _ = tokio::fs::remove_file(&temporary).await;
        return Err(e);
    }
    Ok("音频已保存到作者文件夹（MP3）".into())
}

#[cfg(test)]
mod tests {
    #[test]
    fn live_media_preserves_order_and_static_images() {
        let work = super::super::parse_work(&serde_json::json!({
            "aweme_id":"124", "images":[
                {"url_list":["https://p.douyinpic.com/still.jpg"], "video":{"play_addr":{"url_list":["https://evil.example/video", "https://a.douyinvod.com/live.mp4"]}}},
                {"url_list":["https://p.douyinpic.com/second.jpg"]},
                {"video":{"download_addr":{"url_list":["https://a.douyinvod.com/next.mp4"]}}}
            ]
        })).unwrap();
        assert_eq!(work.kind, "Live");
        assert_eq!(work.media_kinds, ["video", "image", "video"]);
        assert!(
            super::source(&work, "video", 2)
                .unwrap()
                .ends_with("next.mp4")
        );
        assert!(
            super::source(&work, "image", 1)
                .unwrap()
                .ends_with("second.jpg")
        );
        assert!(super::source(&work, "image", 0).is_err());
        assert!(super::source(&work, "video", 3).is_err());
        assert!(
            super::source(&work, "audio", 0)
                .unwrap()
                .ends_with("live.mp4")
        );
        assert!(work.cover.ends_with("still.jpg"));
    }

    #[test]
    fn duration_and_album_soundtrack_do_not_imply_live() {
        let video = super::super::parse_work(&serde_json::json!({"aweme_id":"125", "video":{"duration":2000,"play_addr":{"url_list":["https://a.douyinvod.com/short.mp4"]}}})).unwrap();
        assert_eq!(video.kind, "视频");
        let album = super::super::parse_work(&serde_json::json!({"aweme_id":"126", "video":{"duration":2000,"play_addr":{"url_list":["https://a.douyinvod.com/album.mp4"]}},"images":[{"url_list":["https://p.douyinpic.com/still.jpg"]}]})).unwrap();
        assert_eq!(album.kind, "图集");
        assert_eq!(album.media_kinds, ["image"]);
    }

    #[test]
    fn album_keeps_all_images_and_music() {
        let work = super::super::parse_work(&serde_json::json!({"aweme_id":"123", "images":[{"url_list":["https://p.douyinpic.com/1.webp"]},{"url_list":["https://p.douyinpic.com/2.webp"]}], "music":{"title":"BGM", "play_url":{"url_list":["https://evil.example/a", "https://a.douyinvod.com/song.mp3"]}}})).unwrap();
        assert_eq!(work.urls.len(), 2);
        assert_eq!(
            super::source(&work, "image", 1).unwrap(),
            "https://p.douyinpic.com/2.webp"
        );
        assert!(super::source(&work, "image", 2).is_err());
        assert!(
            super::source(&work, "audio", 0)
                .unwrap()
                .ends_with("song.mp3")
        );
        assert!(super::source(&work, "video", 0).is_err());
    }
}
