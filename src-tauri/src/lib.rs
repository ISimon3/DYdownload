mod library;
mod media;
mod playback;
mod session;
use futures_util::StreamExt;
use serde::Serialize;
use serde_json::Value;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use tauri::{Emitter, Manager, WebviewWindow};
use tokio::io::AsyncWriteExt;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Work {
    id: String,
    title: String,
    author: String,
    author_id: String,
    cover: String,
    created: i64,
    likes: u64,
    duration: u64,
    kind: String,
    #[serde(skip)]
    urls: Vec<String>,
    #[serde(skip)]
    media_kinds: Vec<String>,
    #[serde(skip)]
    music_url: String,
    music_title: String,
}
#[derive(Default)]
struct Store {
    works: Mutex<HashMap<String, Work>>,
    active: Mutex<HashMap<String, Arc<AtomicBool>>>,
    worker_init: tokio::sync::Mutex<()>,
    session_gate: tokio::sync::Mutex<()>,
    login_visible: AtomicBool,
    session_hash: Mutex<u64>,
    query: Mutex<String>,
}
fn local(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("无权执行此操作".into())
    }
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn text_at(v: &Value, path: &str) -> String {
    v.pointer(path)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}
fn media_url(s: &str) -> bool {
    url::Url::parse(s).is_ok_and(|u| {
        u.scheme() == "https"
            && u.port_or_known_default() == Some(443)
            && u.username().is_empty()
            && u.password().is_none()
            && u.host_str().is_some_and(|h| {
                [
                    "douyinvod.com",
                    "douyin.com",
                    "douyinstatic.com",
                    "douyincdn.com",
                    "douyinpic.com",
                    "byteimg.com",
                    "ibytedtos.com",
                    "bytedance.com",
                    "bytecdn.cn",
                    "pstatp.com",
                    "snssdk.com",
                ]
                .iter()
                .any(|d| h == *d || h.ends_with(&format!(".{d}")))
            })
    })
}
fn parse_work(v: &Value) -> Option<Work> {
    let id = text_at(v, "/aweme_id");
    if id.is_empty() || id.len() > 32 || !id.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let images = v["images"].as_array();
    let mut media_kinds = Vec::new();
    let urls: Vec<String> = if let Some(images) = images.filter(|a| !a.is_empty()) {
        images
            .iter()
            .filter_map(|i| {
                let video = first_media_url(i, "/video/play_addr/url_list")
                    .or_else(|| first_media_url(i, "/video/download_addr/url_list"));
                let (url, kind) = if let Some(url) = video {
                    (url, "video")
                } else {
                    (first_media_url(i, "/url_list")?, "image")
                };
                media_kinds.push(kind.to_owned());
                Some(url)
            })
            .collect()
    } else {
        first_media_url(v, "/video/play_addr/url_list")
            .inspect(|_| media_kinds.push("video".into()))
            .into_iter()
            .collect()
    };
    let cover = text_at(v, "/video/cover/url_list/0");
    let cover = if cover.is_empty() && images.is_some_and(|items| !items.is_empty()) {
        images
            .and_then(|items| items.first())
            .and_then(|i| first_media_url(i, "/url_list"))
            .unwrap_or_default()
    } else {
        cover
    };
    Some(Work {
        id,
        title: text_at(v, "/desc").chars().take(1000).collect(),
        author: text_at(v, "/author/nickname"),
        author_id: text_at(v, "/author/sec_uid"),
        cover,
        created: v["create_time"].as_i64().unwrap_or(0),
        likes: v
            .pointer("/statistics/digg_count")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        duration: v
            .pointer("/video/duration")
            .and_then(Value::as_u64)
            .unwrap_or(0),
        kind: if images.is_some_and(|a| !a.is_empty())
            && media_kinds.iter().any(|kind| kind == "video")
        {
            "Live"
        } else if images.is_some_and(|a| !a.is_empty()) {
            "图集"
        } else {
            "视频"
        }
        .into(),
        urls,
        media_kinds,
        music_url: v
            .pointer("/music/play_url/url_list")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .find(|url| media_url(url))
            .unwrap_or_default()
            .into(),
        music_title: text_at(v, "/music/title"),
    })
}
fn first_media_url(value: &Value, path: &str) -> Option<String> {
    value
        .pointer(path)?
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .find(|url| media_url(url))
        .map(str::to_owned)
}
async fn ensure_worker(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    let state = app.state::<Store>();
    let _guard = state.worker_init.lock().await;
    if let Some(window) = app.get_webview_window("douyin") {
        return Ok(window);
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "douyin",
        tauri::WebviewUrl::External(url::Url::parse("about:blank").map_err(err)?),
    )
    .title("连接抖音号 · 登录后自动关闭")
    .inner_size(1100., 800.)
    .visible(false)
    .initialization_script(include_str!("capture.js"))
    .on_navigation(|u| {
        u.as_str() == "about:blank"
            || (u.scheme() == "https"
                && u.host_str()
                    .is_some_and(|h| h == "douyin.com" || h.ends_with(".douyin.com")))
    })
    .build()
    .map_err(err)?;
    let window_for_close = window.clone();
    let app_for_close = app.clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            let _ = window_for_close.hide();
            app_for_close
                .state::<Store>()
                .login_visible
                .store(false, Ordering::Relaxed);
        }
    });
    let path = app
        .path()
        .app_data_dir()
        .map_err(err)?
        .join("session.dpapi");
    let restored = tauri::async_runtime::spawn_blocking(move || session::restore(&path))
        .await
        .map_err(err)?;
    match restored {
        Ok(cookies) => {
            for cookie in cookies {
                window.set_cookie(cookie).map_err(err)?;
            }
        }
        Err(message) => {
            let _ = app.emit_to("main", "session-warning", message);
        }
    }
    window
        .navigate(url::Url::parse("https://www.douyin.com/").map_err(err)?)
        .map_err(err)?;
    Ok(window)
}
#[tauri::command]
async fn open_douyin(
    app: tauri::AppHandle,
    window: WebviewWindow,
    reauthenticate: Option<bool>,
) -> Result<(), String> {
    local(&window)?;
    let worker = ensure_worker(&app).await?;
    let state = app.state::<Store>();
    let _guard = state.session_gate.lock().await;
    if reauthenticate.unwrap_or(false) {
        let url = url::Url::parse("https://www.douyin.com/").map_err(err)?;
        for cookie in worker.cookies_for_url(url).map_err(err)? {
            worker.delete_cookie(cookie).map_err(err)?;
        }
        let path = app
            .path()
            .app_data_dir()
            .map_err(err)?
            .join("session.dpapi");
        match tokio::fs::remove_file(path).await {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(_) => return Err("无法清除旧会话".into()),
        }
        *state.session_hash.lock().map_err(err)? = 0;
    }
    app.state::<Store>()
        .login_visible
        .store(true, Ordering::Relaxed);
    worker
        .navigate(url::Url::parse("https://www.douyin.com/").map_err(err)?)
        .map_err(err)?;
    worker.show().map_err(err)?;
    worker.set_focus().map_err(err)
}
#[tauri::command]
async fn session_status(app: tauri::AppHandle, window: WebviewWindow) -> Result<bool, String> {
    local(&window)?;
    let worker = ensure_worker(&app).await?;
    let session_state = app.state::<Store>();
    let _guard = session_state.session_gate.lock().await;
    let cookies = worker
        .cookies_for_url(url::Url::parse("https://www.douyin.com/").map_err(err)?)
        .map_err(err)?;
    let logged = cookies
        .iter()
        .any(|c| matches!(c.name(), "sessionid" | "sessionid_ss") && !c.value().is_empty());
    if logged {
        use std::hash::{Hash, Hasher};
        let bytes =
            serde_json::to_vec(&cookies.iter().map(ToString::to_string).collect::<Vec<_>>())
                .map_err(err)?;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        let hash = hasher.finish();
        let state = app.state::<Store>();
        let changed = *state.session_hash.lock().map_err(err)? != hash;
        if changed {
            let path = app
                .path()
                .app_data_dir()
                .map_err(err)?
                .join("session.dpapi");
            tauri::async_runtime::spawn_blocking(move || session::save(&path, &bytes))
                .await
                .map_err(err)??;
            *state.session_hash.lock().map_err(err)? = hash;
        }
        if state.login_visible.swap(false, Ordering::Relaxed) {
            worker.hide().map_err(err)?;
            app.emit_to("main", "session-connected", true)
                .map_err(err)?;
        }
    }
    Ok(logged)
}
fn author_url(input: &str) -> Result<url::Url, String> {
    let s = input.trim();
    if s.is_empty() || s.len() > 512 {
        return Err("请输入有效的抖音号、sec_uid 或作者主页链接".into());
    }
    if s.starts_with("https://") {
        let u = url::Url::parse(s).map_err(err)?;
        if u.host_str() == Some("www.douyin.com")
            && u.path().starts_with("/user/")
            && u.path().len() > 6
        {
            return Ok(u);
        }
        return Err("请粘贴 https://www.douyin.com/user/ 开头的作者主页链接".into());
    }
    let mut u = url::Url::parse("https://www.douyin.com/").map_err(err)?;
    if s.starts_with("MS4wLjAB")
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        u.set_path(&format!("/user/{s}"));
    } else {
        u.path_segments_mut()
            .map_err(|_| "无效网址")?
            .extend(["search", s]);
        u.query_pairs_mut().append_pair("type", "user");
    }
    Ok(u)
}
#[tauri::command]
async fn query_author(
    app: tauri::AppHandle,
    window: WebviewWindow,
    input: String,
    query_id: String,
) -> Result<(), String> {
    local(&window)?;
    if query_id.len() > 64 || query_id.is_empty() {
        return Err("无效查询标识".into());
    }
    let mut target = author_url(&input)?;
    let worker = ensure_worker(&app).await?;
    *app.state::<Store>().query.lock().map_err(err)? = query_id.clone();
    target.set_fragment(Some(&format!("sg={query_id}")));
    // A fragment-only navigation does not run the initialization script again.
    // Use a distinct document URL for every retry, including the same author ID.
    target.query_pairs_mut().append_pair("_sg", &query_id);
    worker.hide().map_err(err)?;
    worker.navigate(target).map_err(err)
}
#[tauri::command]
async fn load_more(app: tauri::AppHandle, window: WebviewWindow) -> Result<(), String> {
    local(&window)?;
    app.get_webview_window("douyin")
        .ok_or("请先查询作者")?
        .eval("window.__shiguangLoadMore?.()")
        .map_err(err)
}
fn author_data(user: &Value) -> Value {
    serde_json::json!({
        "uid":text_at(user,"/uid"),"secUid":text_at(user,"/sec_uid"),
        "uniqueId":if text_at(user,"/unique_id").is_empty() { text_at(user,"/short_id") } else { text_at(user,"/unique_id") },
        "name":text_at(user,"/nickname"),"avatar":text_at(user,"/avatar_thumb/url_list/0"),
        "signature":text_at(user,"/signature"),"followers":user["follower_count"],
        "following":user["following_count"],"likes":user["total_favorited"],"workCount":user["aweme_count"]
    })
}
#[tauri::command]
async fn capture_works(
    app: tauri::AppHandle,
    window: WebviewWindow,
    payload: String,
    state: tauri::State<'_, Store>,
) -> Result<(), String> {
    if window.label() != "douyin"
        || window.url().map_err(err)?.host_str() != Some("www.douyin.com")
        || payload.len() > 8_000_000
    {
        return Err("无效数据来源".into());
    }
    let v: Value = serde_json::from_str(&payload).map_err(|_| "作品数据解析失败")?;
    let query_id = text_at(&v, "/queryId");
    if query_id.is_empty() || *state.query.lock().map_err(err)? != query_id {
        return Ok(());
    }
    match text_at(&v,"/kind").as_str() {
        "error" => app.emit_to("main", "query-error", serde_json::json!({"queryId":query_id,"message":text_at(&v,"/message").chars().take(300).collect::<String>()})).map_err(err),
        "author" => {
            let author = author_data(&v["data"]);
            if author["secUid"].as_str().unwrap_or("").is_empty() { return Ok(()); }
            app.emit_to("main", "author", serde_json::json!({"queryId":query_id,"author":author})).map_err(err)
        },
        "works" => {
            let data = &v["data"];
            let works: Vec<Work> = data["aweme_list"].as_array().ok_or("没有作品列表")?.iter().take(100).filter_map(parse_work).collect();
            { let mut cache = state.works.lock().map_err(err)?; if cache.len()+works.len() > 20000 { return Err("作品缓存已满，请重启应用".into()); } for work in &works { cache.insert(work.id.clone(),work.clone()); } }
            if let Some(first) = data["aweme_list"].as_array().and_then(|a|a.first()) {
                app.emit_to("main", "author", serde_json::json!({"queryId":query_id,"author":author_data(&first["author"])})).map_err(err)?;
            }
            let has_more = match &data["has_more"] { Value::Bool(value) => *value, value => value.as_u64().unwrap_or(1) != 0 };
            app.emit_to("main", "works", serde_json::json!({"queryId":query_id,"works":works,"hasMore":has_more})).map_err(err)
        },
        _ => Err("未知数据类型".into())
    }
}
#[tauri::command]
async fn default_directory(app: tauri::AppHandle, window: WebviewWindow) -> Result<String, String> {
    local(&window)?;
    Ok(app
        .path()
        .download_dir()
        .map_err(err)?
        .join("拾光")
        .to_string_lossy()
        .into())
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    id: String,
    status: String,
    done: u64,
    total: u64,
    message: String,
}
fn progress(app: &tauri::AppHandle, id: &str, status: &str, done: u64, total: u64, message: &str) {
    let _ = app.emit_to(
        "main",
        "download-progress",
        Progress {
            id: id.into(),
            status: status.into(),
            done,
            total,
            message: message.into(),
        },
    );
}
#[tauri::command]
async fn cancel_download(
    window: WebviewWindow,
    state: tauri::State<'_, Store>,
    id: String,
) -> Result<(), String> {
    local(&window)?;
    if let Some(c) = state.active.lock().map_err(err)?.get(&id) {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}
#[tauri::command]
async fn start_download(
    app: tauri::AppHandle,
    window: WebviewWindow,
    state: tauri::State<'_, Store>,
    id: String,
    directory: String,
) -> Result<(), String> {
    local(&window)?;
    let work = state
        .works
        .lock()
        .map_err(err)?
        .get(&id)
        .cloned()
        .ok_or("作品数据已失效，请重新查询")?;
    if work.urls.is_empty() {
        return Err("此作品没有可下载的媒体地址".into());
    }
    let root = PathBuf::from(directory);
    let dir = root.join(library::author_folder(
        &work.author,
        &work.author_id,
        &work.id,
    ));
    if !dir.is_absolute() {
        return Err("请选择绝对路径作为保存目录".into());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(err)?;
        if active.contains_key(&id) {
            return Err("作品正在下载".into());
        }
        if active.len() >= 3 {
            return Err("最多同时下载 3 个作品".into());
        }
        active.insert(id.clone(), cancel.clone());
    }
    let result = download(&app, &work, &dir, &cancel).await;
    if let Ok(mut active) = state.active.lock() {
        active.remove(&id);
    }
    match &result {
        Ok(()) => progress(&app, &id, "done", 1, 1, "下载完成"),
        Err(e) => progress(
            &app,
            &id,
            if cancel.load(Ordering::Relaxed) {
                "cancelled"
            } else {
                "error"
            },
            0,
            0,
            e,
        ),
    }
    result
}
async fn download(
    app: &tauri::AppHandle,
    work: &Work,
    dir: &std::path::Path,
    cancel: &Arc<AtomicBool>,
) -> Result<(), String> {
    tokio::fs::create_dir_all(dir)
        .await
        .map_err(|e| format!("无法创建下载目录：{e}"))?;
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
    for (index, url) in work.urls.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消下载".into());
        }
        let response = cancellable(
            client
                .get(url)
                .header("Referer", "https://www.douyin.com/")
                .header("User-Agent", "Mozilla/5.0")
                .send(),
            cancel,
        )
        .await?
        .map_err(|_| "连接媒体服务器失败，请重新查询后重试")?;
        if !response.status().is_success() {
            return Err(format!(
                "媒体服务器返回 {}，请重新查询后重试",
                response.status().as_u16()
            ));
        }
        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("");
        let is_image = work.media_kinds[index] == "image";
        let ext = if is_image {
            if content_type.starts_with("image/") {
                "jpg"
            } else {
                return Err("服务器没有返回有效图片".into());
            }
        } else {
            if !(content_type.starts_with("video/") || content_type.contains("octet-stream")) {
                return Err("服务器没有返回有效视频".into());
            }
            "mp4"
        };
        let path = dir.join(format!("{}_{}.{}", work.id, index + 1, ext));
        if tokio::fs::try_exists(&path).await.map_err(err)? {
            continue;
        }
        let (temp, mut file) = create_download_temp(&path).await?;
        let total = response.content_length().unwrap_or(0);
        let mut done = 0;
        let mut last = Instant::now();
        let mut stream = response.bytes_stream();
        let result: Result<(), String> = async {
            while let Some(chunk) = cancellable(stream.next(), cancel).await? {
                if cancel.load(Ordering::Relaxed) {
                    return Err("已取消下载".into());
                }
                let bytes = chunk.map_err(|_| "下载中断，请重试")?;
                file.write_all(&bytes)
                    .await
                    .map_err(|e| format!("写入失败：{e}"))?;
                done += bytes.len() as u64;
                if last.elapsed() > Duration::from_millis(150) {
                    progress(
                        app,
                        &work.id,
                        "downloading",
                        done,
                        total,
                        &format!("第 {} / {} 个文件", index + 1, work.urls.len()),
                    );
                    last = Instant::now();
                }
            }
            if done == 0 || (total > 0 && done != total) {
                return Err("文件不完整，请重试".into());
            }
            file.flush().await.map_err(err)?;
            file.sync_all().await.map_err(err)?;
            if cancel.load(Ordering::Relaxed) {
                return Err("已取消下载".into());
            }
            Ok(())
        }
        .await;
        drop(file);
        if let Err(e) = result {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(e);
        }
        progress(
            app,
            &work.id,
            "downloading",
            0,
            0,
            &format!(
                "正在转换为 {} · 第 {} / {} 个文件",
                ext.to_uppercase(),
                index + 1,
                work.urls.len()
            ),
        );
        let converted = convert_and_publish(&temp, &path, is_image, cancel.clone()).await;
        let _ = tokio::fs::remove_file(&temp).await;
        converted?;
    }
    Ok(())
}

async fn convert_and_publish(
    input: &std::path::Path,
    target: &std::path::Path,
    image: bool,
    cancel: Arc<AtomicBool>,
) -> Result<(), String> {
    let (temporary, file) = create_download_temp(target).await?;
    drop(file);
    let source = input.to_path_buf();
    let output = temporary.clone();
    let conversion_cancel = cancel.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        media::convert(&source, &output, image, &conversion_cancel)
    })
    .await
    .map_err(err);
    let result = async {
        result??;
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消下载".into());
        }
        let file = tokio::fs::OpenOptions::new()
            .write(true)
            .open(&temporary)
            .await
            .map_err(err)?;
        file.sync_all().await.map_err(err)?;
        drop(file);
        publish_download(&temporary, target)
            .await
            .map_err(|e| format!("保存转换文件失败：{e}"))
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(&temporary).await;
    }
    result
}

async fn create_download_temp(
    path: &std::path::Path,
) -> Result<(PathBuf, tokio::fs::File), String> {
    for index in 0..10_000 {
        let mut name = path.as_os_str().to_os_string();
        name.push(if index == 0 {
            ".part".into()
        } else {
            format!(".{index}.part")
        });
        let temp = PathBuf::from(name);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .await
        {
            Ok(file) => return Ok((temp, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("无法创建临时文件（{}）：{e}", temp.display())),
        }
    }
    Err("此文件的临时文件过多，请清理旧的 .part 文件后重试".into())
}

async fn publish_download(temp: &std::path::Path, path: &std::path::Path) -> std::io::Result<()> {
    // MoveFileExW without REPLACE_EXISTING works on removable filesystems and
    // atomically refuses an existing destination (std::fs::rename replaces it).
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows::{
            Win32::Storage::FileSystem::{MOVE_FILE_FLAGS, MoveFileExW},
            core::PCWSTR,
        };
        let source: Vec<u16> = temp.as_os_str().encode_wide().chain(Some(0)).collect();
        let destination: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        tauri::async_runtime::spawn_blocking(move || {
            // SAFETY: Both paths are NUL-terminated UTF-16 buffers that remain
            // alive throughout the call. Zero flags disallow target replacement.
            unsafe {
                MoveFileExW(
                    PCWSTR(source.as_ptr()),
                    PCWSTR(destination.as_ptr()),
                    MOVE_FILE_FLAGS(0),
                )
            }
            .map_err(|e| std::io::Error::from_raw_os_error(e.code().0 & 0xffff))
        })
        .await
        .map_err(std::io::Error::other)?
    }
    #[cfg(not(windows))]
    {
        tokio::fs::hard_link(temp, path).await?;
        tokio::fs::remove_file(temp).await
    }
}

async fn cancellable<F: std::future::Future>(
    future: F,
    cancel: &AtomicBool,
) -> Result<F::Output, String> {
    let cancellation = async {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    };
    futures_util::pin_mut!(future, cancellation);
    match futures_util::future::select(cancellation, future).await {
        futures_util::future::Either::Left(_) => Err("已取消下载".into()),
        futures_util::future::Either::Right((result, _)) => Ok(result),
    }
}
#[tauri::command]
async fn media_thumbnail(
    app: tauri::AppHandle,
    window: WebviewWindow,
    directory: String,
    path: String,
) -> Result<String, String> {
    local(&window)?;
    static GATE: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
    let _permit = GATE.acquire().await.map_err(err)?;
    let cache = app.path().app_cache_dir().map_err(err)?.join("thumbnails");
    tauri::async_runtime::spawn_blocking(move || {
        use base64::Engine;
        use std::hash::{Hash, Hasher};
        let path = library::checked_path(
            std::path::Path::new(&directory),
            std::path::Path::new(&path),
        )?;
        if !path.is_file() {
            return Err("请选择媒体文件".into());
        }
        let metadata = path.metadata().map_err(err)?;
        let mut key = std::collections::hash_map::DefaultHasher::new();
        path.hash(&mut key);
        metadata.len().hash(&mut key);
        metadata.modified().map_err(err)?.hash(&mut key);
        std::fs::create_dir_all(&cache).map_err(err)?;
        let target = cache.join(format!("{:x}.jpg", key.finish()));
        if !target.exists() {
            let unique = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(err)?
                .as_nanos();
            let temporary = cache.join(format!("{}-{unique}.part", std::process::id()));
            if let Err(e) = media::thumbnail(&path, &temporary) {
                let _ = std::fs::remove_file(&temporary);
                return Err(e);
            }
            if let Err(e) = std::fs::rename(&temporary, &target) {
                let _ = std::fs::remove_file(&temporary);
                return Err(err(e));
            }
        }
        let bytes = std::fs::read(target).map_err(err)?;
        Ok(format!(
            "data:image/jpeg;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
    .map_err(err)?
}

#[tauri::command]
async fn saved_library(
    window: WebviewWindow,
    directory: String,
) -> Result<Vec<library::SavedAuthor>, String> {
    local(&window)?;
    tauri::async_runtime::spawn_blocking(move || library::scan(std::path::Path::new(&directory)))
        .await
        .map_err(err)?
}

#[tauri::command]
async fn open_saved_path(
    window: WebviewWindow,
    directory: String,
    path: String,
) -> Result<(), String> {
    local(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let path = library::checked_path(
            std::path::Path::new(&directory),
            std::path::Path::new(&path),
        )?;
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            std::process::Command::new("explorer.exe")
                .arg(path)
                .creation_flags(0x08000000)
                .spawn()
                .map_err(err)?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            let _ = path;
            Err("目前仅支持 Windows 打开本地文件".into())
        }
    })
    .await
    .map_err(err)?
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Store::default())
        .on_window_event(|window, event| {
            if window.label() == "main"
                && matches!(event, tauri::WindowEvent::CloseRequested { .. })
            {
                if let Ok(active) = window.state::<Store>().active.lock() {
                    for cancel in active.values() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                }
                // The hidden worker window must not keep the application alive.
                media::shutdown();
                window.app_handle().exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            open_douyin,
            session_status,
            query_author,
            load_more,
            capture_works,
            default_directory,
            saved_library,
            playback::work_details,
            playback::prepare_work_media,
            playback::download_work_audio,
            media_thumbnail,
            open_saved_path,
            start_download,
            cancel_download
        ])
        .run(tauri::generate_context!())
        .expect("应用启动失败");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_files_retry_and_publish_without_overwriting() {
        tauri::async_runtime::block_on(async {
            let root = std::env::var_os("SHIGUANG_TEST_DIRECTORY")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir);
            let dir = root.join(format!(
                "shiguang-download-test-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            tokio::fs::create_dir(&dir).await.unwrap();
            let path = dir.join("123_1.mp4");
            let stale = dir.join("123_1.mp4.part");
            tokio::fs::write(&stale, b"interrupted").await.unwrap();
            let (temp, mut file) = create_download_temp(&path).await.unwrap();
            assert_ne!(temp, stale);
            file.write_all(b"complete").await.unwrap();
            file.flush().await.unwrap();
            file.sync_all().await.unwrap();
            drop(file);
            publish_download(&temp, &path).await.unwrap();
            assert_eq!(tokio::fs::read(&path).await.unwrap(), b"complete");
            assert!(!temp.exists());
            assert_eq!(tokio::fs::read(&stale).await.unwrap(), b"interrupted");
            let (other, file) = create_download_temp(&path).await.unwrap();
            drop(file);
            assert!(publish_download(&other, &path).await.is_err());
            assert_eq!(tokio::fs::read(&path).await.unwrap(), b"complete");
            tokio::fs::remove_dir_all(dir).await.unwrap();
        });
    }

    #[test]
    fn rejects_foreign_hosts() {
        assert!(!media_url("https://douyinvod.com.attacker.com/a"));
        assert!(!media_url("http://v.douyinvod.com/a"));
        assert!(!media_url("https://127.0.0.1/a"));
        assert!(media_url("https://v3.douyinvod.com/a"));
    }
    #[test]
    fn author_inputs() {
        assert!(author_url("https://evil.com/user/a").is_err());
        assert!(author_url("").is_err());
        assert_eq!(
            author_url("测试/作者").unwrap().path(),
            "/search/%E6%B5%8B%E8%AF%95%2F%E4%BD%9C%E8%80%85"
        );
        assert_eq!(
            author_url("MS4wLjAB123").unwrap().path(),
            "/user/MS4wLjAB123"
        );
    }
    #[test]
    fn rejects_bad_ids() {
        assert!(parse_work(&serde_json::json!({"aweme_id":"../bad"})).is_none());
    }
    #[test]
    fn parses_video() {
        let w = parse_work(&serde_json::json!({"aweme_id":"123", "desc":"测试", "video":{"play_addr":{"url_list":["https://evil.com/v","https://v.douyinvod.com/v"]}}})).unwrap();
        assert_eq!(w.urls, vec!["https://v.douyinvod.com/v"]);
        assert_eq!(w.kind, "视频");
    }
}
