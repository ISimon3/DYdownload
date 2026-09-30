# 拾光 · 抖音作品下载器

Rust + Tauri 2 + React + TypeScript，iOS 浅色玻璃拟态桌面界面。

主窗口使用一体化玻璃标题栏：拖动顶部空白区域移动窗口，双击顶部最大化/还原，右侧提供最小化、最大化/还原和关闭。程序图标使用原创「拾光」光带符号；Windows 文件属性中的公司为 `simonius`。

## 使用

1. 首次点击「连接抖音号」，在官方窗口自行扫码登录。检测到登录后窗口自动关闭，会话通过 Windows DPAPI 加密保存到应用数据目录。以后启动会自动恢复，无需重复登录或填写 Cookie。
2. 在主程序输入作者抖音号（例如 `40204190842`），点击「查询作者」。后台精确匹配抖音号，显示头像、昵称、唯一 UID、sec_uid 和统计信息。也支持完整作者主页链接或 sec_uid。
   搜索框上方保留最近 12 条搜索历史，点击气泡可重新查询，点击气泡右侧的 × 单独删除。历史仅保存在本机，重启后保留。
3. 点击「下载所有作品」自动分页并下载；点击「查看作品列表」展开网格，可以筛选、排序、多选或下载单个作品。查询和分页不弹出浏览器窗口。
4. 「下载队列」显示进度、取消和重试；「偏好设置」选择保存文件夹。最多同时下载 3 个作品，图集逐张保存，已有同名文件跳过。
5. 「清空队列」停止批量加入、取消正在进行的下载并清空记录，不删除已经保存的作品。
6. 新下载按「作者昵称__作者标识」建立子文件夹，非法文件名字符会自动替换，避免同名作者混放。「拾光作品」读取当前保存位置，支持进入作者文件夹、搜索文件名、刷新、通过系统打开作品或文件夹；旧版根目录作品显示为「未分类作品」。未完成的 `.part` 文件不会显示。
7. 「拾光作品」只显示作者昵称，以缩略图卡片浏览作品，支持「全部 / 图片 / 视频」分类。保存路径仅在偏好设置和下载队列显示。缩略图按需生成并缓存在应用缓存目录，刷新时保留当前内容。
8. 新下载图片实际编码为 JPG；视频优先无损封装为 MP4，无法直接封装时转码为 H.264/AAC。仅处理未来下载，已下载文件不转换。格式处理期间可取消下载，关闭主窗口会终止处理进程。
9. 作者作品卡片提供「查看详情」「媒体播放」「下载」三个操作，点击封面直接播放。视频使用原视频音轨；图集使用作品配乐轮播，也可暂停或手动切换图片。详情中支持封面/图片切换、左右翻页和圆点跳转。「下载音频」将配乐保存为作者文件夹内的 `作品ID_audio.mp3`，视频未提供独立配乐时提取原音轨。没有可用配乐的图集会明确提示。

Cookie 不会显示在前端或写入日志。加密会话仅限当前 Windows 用户解密。抖音主动使会话过期时仍需重新认证。

## 开发与构建

需要 Node.js、Rust MSVC、Visual Studio C++ Build Tools 和 WebView2 Runtime。

```powershell
npm install
npm run tauri dev
```

独立调试程序（无需 Vite 服务）：

```powershell
npm run tauri build -- --debug --no-bundle
```

输出 `src-tauri/target/debug/dy-download.exe`。

包含格式转换工具的完整本地程序：`./build-release.ps1`。需要可用的 FFmpeg，或传入 `-FFmpegPath '完整的 ffmpeg.exe 路径'`。输出 `release/Shiguang/Shiguang.exe` 和 `release/Shiguang-release.zip`；请保留同目录 `ffmpeg.exe` 和许可证文件。普通 Cargo 开发构建会使用程序旁的 FFmpeg 或 PATH 中的 FFmpeg，也支持 `SHIGUANG_FFMPEG` 指定路径。

发行版本：`npm run tauri build -- --no-bundle`，输出 `src-tauri/target/release/dy-download.exe`。
浏览器界面预览：`npm run dev`，浏览器模式不能登录或下载。

## 实现

- `src/main.tsx`：会话、作者详情、作品网格、批量调度、下载队列。
- `src-tauri/src/lib.rs`：受限 IPC、隐藏 WebView 生命周期、查询隔离、作品解析、Rust 下载器。
- `src-tauri/src/capture.js`：后台页面搜索精确匹配、作者信息与作品响应监听、分页。
- 作者搜索等待前端事件订阅完成后启动；后台先确认作者信息回传成功，再跳转主页。作者资料接口未返回时，从已校验来源的作品列表恢复作者信息，避免作品已加载而作者查询仍超时。
- `src-tauri/src/session.rs`：Windows DPAPI 会话加密与恢复。
- `src-tauri/src/library.rs`、`src/SavedLibrary.tsx`：作者目录命名、本地媒体扫描和作品浏览页面。
- `src-tauri/src/media.rs`：真实 JPG/MP4 转换、视频/图片缩略图、可取消的 FFmpeg 子进程管理。
- `src-tauri/capabilities/`：远程抖音页面仅有元数据回传权限，本地命令额外校验调用窗口。

为了使用抖音自身会话和签名中间件，后台保留不可见的官方 WebView；登录成功后不再展示。没有第三方解析服务，也不内置绕过验证或访问私密内容的逻辑。

## 验证

```powershell
npm run build
node --test scripts/capture.test.mjs
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
```

测试包括抖音号精确匹配、过期查询隔离标识、fetch/XHR 监听、来源限制、作品解析以及 DPAPI 加解密/篡改检测。

Rust 回归测试还覆盖作者目录非法字符、同名作者隔离、本地文件扫描、旧版根目录兼容、未完成文件过滤和打开路径限制。运行 Vite 后可打开 `/scripts/ui-fixture.html`，使用模拟作者验证「3 个下载中 + 2 个排队」清空后活动任务为 0，以及本地作品列表、搜索和打开命令；此测试页不执行真实下载或文件操作，也不包含在发行页面中。

媒体测试需要 FFmpeg，覆盖真实 WebP → JPEG、MOV → MP4、视频缩略图、损坏文件和处理中取消。模拟页面支持 `?loading=1` 查看加载状态，并可验证缩略图卡片、分类和刷新。

## 当前边界

- 抖音网站并非稳定公开 API，接口改版、验证码、账号权限与临时媒体链接过期可能影响查询。界面明确报告超时，不会把部分作品当作全部下载成功。
- 播放媒体会先缓存到应用缓存目录，首次加载大型视频需要等待；单项预览上限为 512 MB。关闭播放窗口后停止声音。平台地址过期时需重新查询作者。
- 作品缓存、下载队列仅在当前运行期间有效；下载目录与登录会话持久化。尚不支持断点续传，重试会自动避开异常退出留下的 `.part` 文件，旧文件可自行清理。
- Windows 下载采用同目录临时文件和不覆盖目标的重命名保存，兼容 NTFS、exFAT 和 FAT32；FAT32 仍有单文件 4 GB 限制。主窗口启动居中，关闭主窗口会退出整个应用。
- 当前目标平台为 Windows，其他系统尚未验证，加密会话模块不支持其他系统。

如 Windows Schannel 无法下载 crates，可选运行 `node scripts/fetch-crates.mjs`；它从官方 registry 下载 Cargo.lock 锁定的包并核对 SHA-256，不修改全局源配置。
