use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};

static CHILDREN: Mutex<Vec<Weak<Mutex<Child>>>> = Mutex::new(Vec::new());
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

struct Process(Arc<Mutex<Child>>);
impl Drop for Process {
    fn drop(&mut self) {
        if let Ok(mut child) = self.0.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

pub fn shutdown() {
    if let Ok(mut children) = CHILDREN.lock() {
        SHUTTING_DOWN.store(true, Ordering::Relaxed);
        for child in children.drain(..).filter_map(|c| c.upgrade()) {
            if let Ok(mut child) = child.lock() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

fn executable() -> PathBuf {
    if let Some(path) = std::env::var_os("SHIGUANG_FFMPEG") {
        return path.into();
    }
    if let Ok(exe) = std::env::current_exe() {
        let adjacent = exe.with_file_name("ffmpeg.exe");
        if adjacent.is_file() {
            return adjacent;
        }
    }
    "ffmpeg".into()
}

fn run(args: Vec<OsString>, cancel: &AtomicBool, timeout: Duration) -> Result<(), String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("已取消下载".into());
    }
    let mut command = Command::new(executable());
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-nostdin",
            "-y",
            "-threads",
            "2",
            "-protocol_whitelist",
            "file,pipe",
        ])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let process = {
        let mut children = CHILDREN.lock().map_err(super::err)?;
        if SHUTTING_DOWN.load(Ordering::Relaxed) {
            return Err("程序正在退出".into());
        }
        let child = command
            .spawn()
            .map_err(|e| format!("无法启动格式转换工具，请保留程序旁的 ffmpeg.exe：{e}"))?;
        let process = Process(Arc::new(Mutex::new(child)));
        children.retain(|child| child.strong_count() > 0);
        children.push(Arc::downgrade(&process.0));
        process
    };
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("已取消下载".into());
        }
        if start.elapsed() > timeout {
            return Err("媒体处理超时，请检查文件后重试".into());
        }
        if let Some(status) = process
            .0
            .lock()
            .map_err(super::err)?
            .try_wait()
            .map_err(super::err)?
        {
            return if status.success() {
                Ok(())
            } else {
                Err("媒体处理失败：文件可能损坏、编码不受支持或磁盘空间不足".into())
            };
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn arguments(input: &Path, output: &Path, options: &[&str]) -> Vec<OsString> {
    let mut args: Vec<OsString> = vec!["-i".into(), input.as_os_str().into()];
    args.extend(options.iter().map(OsString::from));
    args.push(output.as_os_str().into());
    args
}

pub fn convert(
    input: &Path,
    output: &Path,
    image: bool,
    cancel: &AtomicBool,
) -> Result<(), String> {
    let timeout = Duration::from_secs(1800);
    if image {
        run(
            arguments(
                input,
                output,
                &[
                    "-map",
                    "0:v:0",
                    "-frames:v",
                    "1",
                    "-c:v",
                    "mjpeg",
                    "-q:v",
                    "2",
                    "-f",
                    "image2",
                    "-update",
                    "1",
                ],
            ),
            cancel,
            timeout,
        )
    } else {
        // Remux first: produce a real MP4 container without unnecessary quality loss.
        let copied = run(
            arguments(
                input,
                output,
                &[
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a?",
                    "-c",
                    "copy",
                    "-movflags",
                    "+faststart",
                    "-f",
                    "mp4",
                ],
            ),
            cancel,
            timeout,
        );
        if copied.is_ok() || cancel.load(Ordering::Relaxed) {
            return copied;
        }
        run(
            arguments(
                input,
                output,
                &[
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a?",
                    "-c:v",
                    "libx264",
                    "-threads",
                    "2",
                    "-preset",
                    "veryfast",
                    "-crf",
                    "20",
                    "-vf",
                    "scale=trunc(iw/2)*2:trunc(ih/2)*2",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-movflags",
                    "+faststart",
                    "-f",
                    "mp4",
                ],
            ),
            cancel,
            timeout,
        )
    }
}

pub fn extract_audio(input: &Path, output: &Path, cancel: &AtomicBool) -> Result<(), String> {
    run(
        arguments(
            input,
            output,
            &[
                "-map",
                "0:a:0",
                "-vn",
                "-c:a",
                "libmp3lame",
                "-q:a",
                "2",
                "-f",
                "mp3",
            ],
        ),
        cancel,
        Duration::from_secs(1800),
    )
}

pub fn thumbnail(input: &Path, output: &Path) -> Result<(), String> {
    run(
        arguments(
            input,
            output,
            &[
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-vf",
                "scale=480:480:force_original_aspect_ratio=decrease",
                "-c:v",
                "mjpeg",
                "-q:v",
                "4",
                "-threads",
                "1",
                "-f",
                "image2",
                "-update",
                "1",
            ],
        ),
        &AtomicBool::new(false),
        Duration::from_secs(20),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_media_conversion_and_thumbnail() {
        let root = std::env::temp_dir().join(format!("shiguang-media-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let ppm = root.join("source.ppm");
        let mut pixels = b"P6\n16 16\n255\n".to_vec();
        pixels.extend([255u8, 0, 0].repeat(256));
        std::fs::write(&ppm, pixels).unwrap();
        let flag = AtomicBool::new(false);
        let webp = root.join("source.webp");
        run(
            arguments(&ppm, &webp, &["-frames:v", "1"]),
            &flag,
            Duration::from_secs(20),
        )
        .unwrap();
        let jpg = root.join("converted.jpg");
        convert(&webp, &jpg, true, &flag).unwrap();
        assert!(
            std::fs::read(&jpg)
                .unwrap()
                .starts_with(&[0xff, 0xd8, 0xff])
        );
        let mov = root.join("source.mov");
        run(
            arguments(
                &ppm,
                &mov,
                &["-frames:v", "1", "-c:v", "mpeg4", "-f", "mov"],
            ),
            &flag,
            Duration::from_secs(20),
        )
        .unwrap();
        let mp4 = root.join("converted.mp4");
        convert(&mov, &mp4, false, &flag).unwrap();
        assert_eq!(&std::fs::read(&mp4).unwrap()[4..8], b"ftyp");
        let thumb = root.join("thumb.jpg");
        thumbnail(&mp4, &thumb).unwrap();
        assert!(
            std::fs::read(&thumb)
                .unwrap()
                .starts_with(&[0xff, 0xd8, 0xff])
        );
        let tone = root.join("tone.wav");
        let mut tone_args: Vec<OsString> = ["-f", "lavfi", "-i", "sine=frequency=440:duration=0.2"]
            .iter()
            .map(OsString::from)
            .collect();
        tone_args.push(tone.as_os_str().into());
        run(tone_args, &flag, Duration::from_secs(20)).unwrap();
        let audio = root.join("audio.mp3");
        extract_audio(&tone, &audio, &flag).unwrap();
        assert!(std::fs::metadata(&audio).unwrap().len() > 100);
        run(
            arguments(&audio, Path::new("-"), &["-f", "null"]),
            &flag,
            Duration::from_secs(20),
        )
        .unwrap();
        let bad = root.join("bad.webp");
        std::fs::write(&bad, b"not media").unwrap();
        assert!(convert(&bad, &root.join("bad.jpg"), true, &flag).is_err());
        assert!(
            convert(
                &webp,
                &root.join("cancelled.jpg"),
                true,
                &AtomicBool::new(true)
            )
            .is_err()
        );
        let cancellation = Arc::new(AtomicBool::new(false));
        let worker_cancel = cancellation.clone();
        let worker = std::thread::spawn(move || {
            run(
                [
                    "-re",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=red:s=16x16:r=1",
                    "-f",
                    "null",
                    "-",
                ]
                .iter()
                .map(OsString::from)
                .collect(),
                &worker_cancel,
                Duration::from_secs(20),
            )
        });
        let deadline = Instant::now();
        while !CHILDREN
            .lock()
            .unwrap()
            .iter()
            .any(|child| child.strong_count() > 0)
        {
            assert!(
                deadline.elapsed() < Duration::from_secs(5),
                "转换进程未启动"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        cancellation.store(true, Ordering::Relaxed);
        assert!(worker.join().unwrap().is_err());
        assert!(
            CHILDREN
                .lock()
                .unwrap()
                .iter()
                .all(|child| child.strong_count() == 0)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
