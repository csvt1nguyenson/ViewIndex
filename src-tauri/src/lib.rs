use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

static AUDIT_PID: Mutex<Option<u32>> = Mutex::new(None);
static AUDIT_CANCELLED: AtomicBool = AtomicBool::new(false);

const CREATE_NO_WINDOW: u32 = 0x08000000;

/// Tìm đường dẫn file thực thi đi kèm app (yt-dlp hoặc ffmpeg).
/// Ưu tiên: thư mục bin/ cùng cấp exe > cùng cấp exe > fallback tìm trong PATH hệ thống.
fn get_tool_path(name: &str) -> std::path::PathBuf {
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            // 1. Tìm trong thư mục bin/ cùng cấp
            let in_bin = exe_dir.join("bin").join(format!("{}.exe", name));
            if in_bin.exists() {
                return in_bin;
            }
            // 2. Tìm ngay tại thư mục app
            let direct = exe_dir.join(format!("{}.exe", name));
            if direct.exists() {
                return direct;
            }
        }
    }
    // 3. Fallback: tìm trong biến môi trường PATH hệ thống
    std::path::PathBuf::from(name)
}

/// Lấy thư mục chứa ffmpeg (để truyền vào --ffmpeg-location cho yt-dlp)
fn get_ffmpeg_dir() -> Option<String> {
    let ffmpeg_path = get_tool_path("ffmpeg");
    if ffmpeg_path.exists() {
        ffmpeg_path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
    } else {
        None
    }
}

#[derive(Serialize, Deserialize)]
pub struct SystemStatus {
    pub ytdlp_installed: bool,
    pub ytdlp_version: String,
    pub ffmpeg_installed: bool,
    pub downloads_dir: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProgressPayload {
    pub percent: f32,
    pub speed: String,
    pub eta: String,
    pub total: String,
    pub status: String,
    pub is_done: bool,
    pub is_error: bool,
    pub file_path: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SubtitleBatchResult {
    pub folder: String,
    pub processed: usize,
    pub downloaded: u32,
    pub skipped: u32,
    pub failed: u32,
}

#[tauri::command]
fn check_system() -> SystemStatus {
    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let (ytdlp_installed, ytdlp_version) = match cmd.output() {
        Ok(output) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (true, ver)
        }
        _ => (false, String::new()),
    };

    let mut ffmpeg_cmd = Command::new("ffmpeg");
    ffmpeg_cmd.arg("-version");
    #[cfg(target_os = "windows")]
    ffmpeg_cmd.creation_flags(CREATE_NO_WINDOW);

    let ffmpeg_installed = match ffmpeg_cmd.output() {
        Ok(output) => output.status.success(),
        _ => false,
    };

    let downloads_dir = dirs::download_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "Downloads".to_string());

    SystemStatus {
        ytdlp_installed,
        ytdlp_version,
        ffmpeg_installed,
        downloads_dir,
    }
}

fn get_current_ytdlp_version() -> String {
    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("--version");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    match cmd.output() {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).trim().to_string(),
        _ => "Mới nhất".to_string(),
    }
}

#[tauri::command]
async fn update_ytdlp() -> Result<String, String> {
    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("-U");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output_u = cmd.output();
    let mut needs_pip = false;

    if let Ok(ref out) = output_u {
        let text = format!(
            "{} {}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        if out.status.success() {
            let ver = get_current_ytdlp_version();
            return Ok(format!("Cập nhật thành công lên bản: {}", ver));
        } else if text.contains("pip") || text.contains("PyPi") || text.contains("wheel") {
            needs_pip = true;
        }
    } else {
        needs_pip = true;
    }

    if needs_pip {
        let mut pip_cmd = Command::new("python");
        pip_cmd.args(["-m", "pip", "install", "-U", "yt-dlp"]);
        #[cfg(target_os = "windows")]
        pip_cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = pip_cmd.output() {
            if out.status.success() {
                let ver = get_current_ytdlp_version();
                return Ok(format!("Đã cập nhật thành công lên phiên bản: {}", ver));
            }
        }

        let mut direct_pip = Command::new("pip");
        direct_pip.args(["install", "-U", "yt-dlp"]);
        #[cfg(target_os = "windows")]
        direct_pip.creation_flags(CREATE_NO_WINDOW);

        if let Ok(out) = direct_pip.output() {
            if out.status.success() {
                let ver = get_current_ytdlp_version();
                return Ok(format!("Đã cập nhật thành công lên phiên bản: {}", ver));
            }
        }
    }

    let ver = get_current_ytdlp_version();
    Ok(format!("yt-dlp hiện tại là phiên bản: {}", ver))
}

fn is_cookie_extraction_error(raw_err: &str) -> bool {
    let lower = raw_err.to_lowercase();
    lower.contains("could not copy")
        || lower.contains("cookie database")
        || lower.contains("cookies database")
        || (lower.contains("could not find") && lower.contains("cookies"))
        || lower.contains("failed to decrypt with dpapi")
        || lower.contains("dpapi")
}

fn format_ytdlp_error(raw_err: &str) -> String {
    let lower = raw_err.to_lowercase();

    // 1. Video deleted / removed / private / unavailable (yt-dlp often misleadingly prints bot/cookies error for these)
    if lower.contains("video unavailable")
        || lower.contains("this video is unavailable")
        || lower.contains("this video has been removed")
        || lower.contains("private video")
        || lower.contains("sign in to confirm you're not a bot")
        || lower.contains("sign in to confirm you’re not a bot")
    {
        return "Video không khả dụng (Video này không tồn tại, đã bị chủ kênh xóa, bị ẩn hoặc đang đặt ở chế độ Riêng tư trên YouTube).".to_string();
    }

    // 2. Cookie extraction / decryption failure on Windows
    if is_cookie_extraction_error(raw_err) {
        return "Không thể đọc Cookie từ trình duyệt đã chọn (do trình duyệt đang mở hoặc chưa cài đặt trên máy). Hãy chuyển về 'Không dùng cookies'.".to_string();
    }

    // 3. Rate limiting / HTTP 429
    if lower.contains("429") || lower.contains("too many requests") {
        return "YouTube đang tạm thời giới hạn tần suất yêu cầu (HTTP 429: Too Many Requests). Vui lòng thử lại sau ít phút.".to_string();
    }

    // 4. Channel or Playlist not found
    if lower.contains("not found")
        || lower.contains("unable to extract channel")
        || lower.contains("does not exist")
    {
        return "Không tìm thấy dữ liệu. Kênh hoặc Danh sách phát này không tồn tại hoặc đã bị đổi tên/xóa.".to_string();
    }

    // 5. Invalid URL / ID
    if lower.contains("incomplete youtube id") || lower.contains("does not look like a valid") {
        return "Đường dẫn không hợp lệ. Vui lòng kiểm tra lại link YouTube đã nhập.".to_string();
    }

    // 6. Network connection error
    if lower.contains("temporary failure in name resolution")
        || lower.contains("network is unreachable")
        || lower.contains("connection refused")
        || lower.contains("timed out")
        || lower.contains("failed to establish a new connection")
    {
        return "Lỗi kết nối mạng: Không thể kết nối tới máy chủ YouTube. Vui lòng kiểm tra lại Internet.".to_string();
    }

    // Fallback: clean up yt-dlp technical prefixes if any
    let cleaned = raw_err
        .lines()
        .filter(|l| l.contains("ERROR:") || !l.trim().is_empty())
        .map(|l| l.trim_start_matches("ERROR:").trim())
        .collect::<Vec<_>>()
        .join(" ");

    if cleaned.trim().is_empty() {
        "Không thể phân tích dữ liệu từ YouTube. Vui lòng thử lại sau.".to_string()
    } else {
        cleaned
    }
}

fn build_analyze_command(
    url: &str,
    is_playlist_or_channel: bool,
    cookies_browser: Option<&str>,
) -> Command {
    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("--dump-single-json").arg("--no-warnings");

    if let Some(ffdir) = get_ffmpeg_dir() {
        cmd.arg("--ffmpeg-location").arg(&ffdir);
    }

    if is_playlist_or_channel {
        cmd.arg("--flat-playlist");
    } else {
        cmd.arg("--skip-download").arg("--no-playlist");
    }

    if let Some(browser) = cookies_browser {
        let b = browser.trim();
        if !b.is_empty() && b != "none" {
            cmd.arg("--cookies-from-browser").arg(b);
        }
    }

    cmd.arg("--").arg(url);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd
}

#[tauri::command]
async fn analyze_url(
    url: String,
    cookies_browser: Option<String>,
) -> Result<serde_json::Value, String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err("Vui lòng nhập đường dẫn URL YouTube hợp lệ.".to_string());
    }

    let is_watch_video = trimmed_url.contains("/watch") || trimmed_url.contains("youtu.be/");
    let is_playlist_or_channel = !is_watch_video
        && (trimmed_url.contains("/@")
            || trimmed_url.contains("/channel/")
            || trimmed_url.contains("/c/")
            || trimmed_url.contains("/playlist")
            || trimmed_url.contains("/user/"));

    let browser_choice = cookies_browser
        .as_deref()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty() && *s != "none");

    let mut cmd = build_analyze_command(trimmed_url, is_playlist_or_channel, browser_choice);

    let output = match cmd.output() {
        Ok(out) => out,
        Err(e) => {
            return Err(format!(
                "Không thể khởi chạy yt-dlp: {}. Hãy chắc chắn rằng yt-dlp đã được cài đặt và thêm vào biến môi trường PATH.",
                e
            ));
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // NẾU CÓ CHỌN COOKIE MÀ BỊ LỖI DO TRÌNH DUYỆT ĐANG MỞ HOẶC CHƯA CÀI:
        // TỰ ĐỘNG THỬ LẠI LẦN NỮA MÀ KHÔNG DÙNG COOKIE ĐỂ VIDEO BÌNH THƯỜNG VẪN CHẠY ĐƯỢC!
        if browser_choice.is_some() && is_cookie_extraction_error(&stderr) {
            let mut retry_cmd = build_analyze_command(trimmed_url, is_playlist_or_channel, None);
            if let Ok(retry_output) = retry_cmd.output() {
                if retry_output.status.success() {
                    let stdout_str = String::from_utf8_lossy(&retry_output.stdout);
                    match serde_json::from_str::<serde_json::Value>(&stdout_str) {
                        Ok(json) => return Ok(json),
                        Err(e) => return Err(format!("Lỗi khi phân giải dữ liệu JSON từ yt-dlp: {}", e)),
                    }
                } else {
                    let retry_stderr = String::from_utf8_lossy(&retry_output.stderr);
                    return Err(format_ytdlp_error(&retry_stderr));
                }
            }
        }
        return Err(format_ytdlp_error(&stderr));
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<serde_json::Value>(&stdout_str) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Lỗi khi phân giải dữ liệu JSON từ yt-dlp: {}", e)),
    }
}

#[tauri::command]
fn select_save_path(default_filename: String, filter_ext: String) -> Option<String> {
    let mut dialog = rfd::FileDialog::new().set_file_name(&default_filename);

    if let Some(dl) = dirs::download_dir() {
        dialog = dialog.set_directory(&dl);
    }

    if !filter_ext.is_empty() {
        let ext = filter_ext.trim_start_matches('.').to_lowercase();
        dialog = dialog.add_filter(&format!("Tệp {}", ext.to_uppercase()), &[&ext]);
    }

    let result = dialog.save_file();
    result.map(|mut p| {
        if !filter_ext.is_empty() && p.extension().is_none() {
            let ext = filter_ext.trim_start_matches('.').to_lowercase();
            p.set_extension(ext);
        }
        p.to_string_lossy().to_string()
    })
}

#[tauri::command]
fn select_folder() -> Option<String> {
    let mut dialog = rfd::FileDialog::new();
    if let Some(dl) = dirs::download_dir() {
        dialog = dialog.set_directory(&dl);
    }
    dialog
        .pick_folder()
        .map(|p| p.to_string_lossy().to_string())
}

fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => c,
        })
        .collect::<String>()
        .trim()
        .chars()
        .take(100)
        .collect();

    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let upper = cleaned.to_uppercase();
    if reserved
        .iter()
        .any(|&r| upper == r || upper.starts_with(&format!("{}.", r)))
    {
        format!("{}_safe", cleaned)
    } else if cleaned.is_empty() {
        "unnamed".to_string()
    } else {
        cleaned
    }
}

#[derive(serde::Serialize)]
struct ThumbnailBatchResult {
    folder: String,
    downloaded: u32,
    failed: usize,
    total: usize,
}

#[tauri::command]
async fn download_all_thumbnails(
    app: AppHandle,
    folder: String,
    channel_name: String,
    videos: Vec<serde_json::Value>,
) -> Result<ThumbnailBatchResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let safe_channel = sanitize_filename(&channel_name);
        let target_dir = std::path::Path::new(&folder).join(&safe_channel);
        std::fs::create_dir_all(&target_dir).map_err(|e| format!("Lỗi tạo thư mục: {}", e))?;

        let total = videos.len();
        let mut downloaded: u32 = 0;

        for (idx, video) in videos.iter().enumerate() {
            let id = video["id"].as_str().unwrap_or("");
            let title = video["title"].as_str().unwrap_or("video");
            if id.is_empty() {
                continue;
            }

            if id.len() != 11
                || !id
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
            {
                continue;
            }
            let url = format!("https://i.ytimg.com/vi/{}/maxresdefault.jpg", id);
            let safe_title = sanitize_filename(title);
            let filename = format!("{:03} - {} - {}.jpg", idx + 1, safe_title, id);
            let filepath = target_dir.join(&filename);
            let temporary_path = filepath.with_extension("jpg.part");

            // Download using curl (built-in on Windows 10+)
            let mut cmd = Command::new("curl");
            cmd.args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--connect-timeout",
                "15",
                "--max-time",
                "60",
                "--retry",
                "2",
                "-o",
                &temporary_path.to_string_lossy(),
                &url,
            ]);
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);

            if let Ok(output) = cmd.output() {
                if output.status.success()
                    && std::fs::metadata(&temporary_path)
                        .map(|m| m.len() > 0)
                        .unwrap_or(false)
                    && std::fs::rename(&temporary_path, &filepath).is_ok()
                {
                    downloaded += 1;
                }
            }
            let _ = std::fs::remove_file(&temporary_path);

            // Emit progress to frontend
            let _ = app.emit(
                "thumbnail-progress",
                serde_json::json!({
                    "current": idx + 1,
                    "total": total,
                    "downloaded": downloaded,
                    "filename": filename
                }),
            );
        }

        Ok(ThumbnailBatchResult {
            folder: target_dir.to_string_lossy().to_string(),
            downloaded,
            failed: total - downloaded as usize,
            total,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

fn is_regular_video_for_subtitles(video: &serde_json::Value) -> bool {
    let id = video["id"].as_str().unwrap_or("");
    if id.is_empty() {
        return false;
    }

    let duration = video["duration"].as_f64().unwrap_or(0.0);
    if duration <= 0.0 || duration <= 180.0 {
        return false;
    }

    let title = video["title"].as_str().unwrap_or("").to_lowercase();
    let url = video["url"].as_str().unwrap_or("").to_lowercase();
    let live_status = video["live_status"].as_str().unwrap_or("").to_lowercase();
    let was_live = video["was_live"].as_bool().unwrap_or(false);

    !was_live
        && !live_status.contains("live")
        && !url.contains("/live/")
        && !url.contains("/shorts/")
        && !title.contains("#shorts")
        && !title.contains("[live]")
}

fn count_srt_files(dir: &std::path::Path) -> usize {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("srt"))
                        .unwrap_or(false)
                })
                .count()
        })
        .unwrap_or(0)
}

#[tauri::command]
async fn download_all_subtitles(
    app: AppHandle,
    folder: String,
    channel_name: String,
    videos: Vec<serde_json::Value>,
    cookies_browser: Option<String>,
) -> Result<SubtitleBatchResult, String> {
    let safe_channel = sanitize_filename(&channel_name);
    let target_dir = std::path::Path::new(&folder).join(format!("{} - SRT", safe_channel));
    std::fs::create_dir_all(&target_dir).map_err(|e| format!("Lỗi tạo thư mục SRT: {}", e))?;

    let regular_videos: Vec<&serde_json::Value> = videos
        .iter()
        .filter(|video| is_regular_video_for_subtitles(video))
        .collect();
    let total = regular_videos.len();
    if total == 0 {
        return Ok(SubtitleBatchResult {
            folder: target_dir.to_string_lossy().to_string(),
            processed: 0,
            downloaded: 0,
            skipped: videos.len() as u32,
            failed: 0,
        });
    }

    let mut downloaded: u32 = 0;
    let mut failed: u32 = 0;

    for (idx, video) in regular_videos.iter().enumerate() {
        let id = video["id"].as_str().unwrap_or("");
        let title = video["title"].as_str().unwrap_or("video");
        let url = video["url"]
            .as_str()
            .map(|u| u.to_string())
            .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));

        let safe_title = sanitize_filename(title);
        let output_template = target_dir
            .join(format!("{:03} - {} - {}.%(ext)s", idx + 1, safe_title, id))
            .to_string_lossy()
            .to_string();
        let before_count = count_srt_files(&target_dir);

        let mut cmd = Command::new(get_tool_path("yt-dlp"));
        cmd.arg("--skip-download")
            .arg("--write-sub")
            .arg("--write-auto-sub")
            .arg("--sub-format")
            .arg("srt")
            .arg("--sub-lang")
            .arg("vi,en")
            .arg("--convert-subs")
            .arg("srt")
            .arg("--no-playlist")
            .arg("--ignore-errors")
            .arg("-o")
            .arg(&output_template);

        if let Some(ffdir) = get_ffmpeg_dir() {
            cmd.arg("--ffmpeg-location").arg(&ffdir);
        }

        if let Some(browser) = cookies_browser.as_ref() {
            let b = browser.trim();
            if !b.is_empty() && b != "none" {
                cmd.arg("--cookies-from-browser").arg(b);
            }
        }

        cmd.arg("--").arg(&url);

        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        match cmd.output() {
            Ok(output) => {
                let after_count = count_srt_files(&target_dir);
                if output.status.success() && after_count > before_count {
                    downloaded += 1;
                } else {
                    failed += 1;
                }
            }
            Err(_) => failed += 1,
        }

        let current = idx + 1;
        let skipped = videos.len().saturating_sub(total) as u32;
        let _ = app.emit(
            "subtitle-progress",
            serde_json::json!({
                "current": current,
                "total": total,
                "downloaded": downloaded,
                "skipped": skipped,
                "failed": failed,
                "title": title
            }),
        );
    }

    Ok(SubtitleBatchResult {
        folder: target_dir.to_string_lossy().to_string(),
        processed: total,
        downloaded,
        skipped: videos.len().saturating_sub(total) as u32,
        failed,
    })
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChannelAuditOptions {
    pub channel_url: String,
    pub target_folder: Option<String>,
    pub tabs: Vec<String>,
    pub limit: usize,
    pub download_subs: bool,
    pub download_comments: bool,
    pub download_thumbnails: bool,
    pub anti_bot_delay: bool,
    pub cookies_browser: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChannelAuditResult {
    pub folder: String,
    pub csv_file: String,
    pub channel_json: String,
    pub ai_transcripts_file: String,
    pub total_crawled: usize,
    pub videos_count: usize,
    pub shorts_count: usize,
    pub streams_count: usize,
    pub total_views: u64,
    pub total_likes: u64,
    pub total_comments: u64,
    pub channel_title: String,
    pub channel_subscribers: String,
    pub sample_videos: Vec<serde_json::Value>,
}

fn clean_channel_base_url(raw: &str) -> String {
    let mut s = raw.trim().to_string();
    if let Some(idx) = s.find('?') {
        s.truncate(idx);
    }
    if let Some(idx) = s.find('#') {
        s.truncate(idx);
    }
    s = s.trim_end_matches('/').to_string();

    if !s.starts_with("http://") && !s.starts_with("https://") {
        if s.starts_with('@') {
            s = format!("https://www.youtube.com/{}", s);
        } else if s.starts_with("channel/") || s.starts_with("c/") || s.starts_with("user/") {
            s = format!("https://www.youtube.com/{}", s);
        } else if s.starts_with("UC") && s.len() >= 24 {
            s = format!("https://www.youtube.com/channel/{}", s);
        } else {
            s = format!("https://www.youtube.com/@{}", s);
        }
    }

    let suffixes = [
        "/videos",
        "/shorts",
        "/streams",
        "/featured",
        "/community",
        "/playlists",
        "/live",
    ];
    for suffix in suffixes {
        if s.ends_with(suffix) {
            s.truncate(s.len() - suffix.len());
            break;
        }
    }
    s
}

fn clean_vtt_to_plain_text(content: &str) -> String {
    let mut lines = Vec::new();
    let mut prev_line = String::new();

    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty()
            || line.starts_with("WEBVTT")
            || line.starts_with("Kind:")
            || line.starts_with("Language:")
            || line.contains("-->")
            || line.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        let mut clean = String::new();
        let mut in_tag = false;
        for ch in line.chars() {
            if ch == '<' {
                in_tag = true;
            } else if ch == '>' {
                in_tag = false;
            } else if !in_tag {
                clean.push(ch);
            }
        }
        let clean = clean.trim();
        if !clean.is_empty() && clean != prev_line {
            lines.push(clean.to_string());
            prev_line = clean.to_string();
        }
    }

    lines.join(" ")
}

fn escape_csv_cell(val: &str) -> String {
    let escaped = val
        .replace('"', "\"\"")
        .replace('\r', " ")
        .replace('\n', " ");
    format!("\"{}\"", escaped)
}

fn format_duration_hms(secs: u64) -> String {
    let hours = secs / 3600;
    let mins = (secs % 3600) / 60;
    let s = secs % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, mins, s)
    } else {
        format!("{:02}:{:02}", mins, s)
    }
}

fn get_simple_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    secs.to_string()
}

#[tauri::command]
fn cancel_channel_audit() -> Result<(), String> {
    AUDIT_CANCELLED.store(true, Ordering::SeqCst);
    if let Ok(guard) = AUDIT_PID.lock() {
        if let Some(pid) = *guard {
            #[cfg(target_os = "windows")]
            {
                let _ = Command::new("taskkill")
                    .args(["/F", "/T", "/PID", &pid.to_string()])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();
            }
            #[cfg(not(target_os = "windows"))]
            {
                let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
            }
        }
    }
    Ok(())
}

#[tauri::command]
async fn start_channel_audit(
    app: AppHandle,
    options: ChannelAuditOptions,
) -> Result<ChannelAuditResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
        AUDIT_CANCELLED.store(false, Ordering::SeqCst);

        let raw_url = options.channel_url.trim();
        if raw_url.is_empty() {
            return Err("Vui lòng nhập đường dẫn kênh YouTube hợp lệ.".to_string());
        }

        let base_url = clean_channel_base_url(raw_url);
        let selected_tabs = if options.tabs.is_empty() {
            vec!["videos".to_string()]
        } else {
            options.tabs.clone()
        };

        let _ = app.emit(
            "channel-audit-progress",
            serde_json::json!({
                "phase": "init",
                "percent": 3,
                "message": format!("Đang kết nối tới kênh: {}", base_url)
            }),
        );

        let mut info_cmd = Command::new(get_tool_path("yt-dlp"));
        info_cmd.args(["-J", "--flat-playlist", "--playlist-end", "1", "--no-warnings"]);
        if let Some(ffdir) = get_ffmpeg_dir() {
            info_cmd.arg("--ffmpeg-location").arg(&ffdir);
        }
        if let Some(ref browser) = options.cookies_browser {
            let b = browser.trim();
            if !b.is_empty() && b != "none" {
                info_cmd.arg("--cookies-from-browser").arg(b);
            }
        }
        info_cmd.arg(format!("{}/videos", base_url.trim_end_matches('/')));
        #[cfg(target_os = "windows")]
        info_cmd.creation_flags(CREATE_NO_WINDOW);

        let info_output = info_cmd.output().map_err(|e| format!("Không thể chạy yt-dlp lấy thông tin kênh: {}", e))?;
        let info_stdout = String::from_utf8_lossy(&info_output.stdout);
        let channel_info: serde_json::Value = serde_json::from_str(&info_stdout).unwrap_or(serde_json::json!({}));

        let channel_title = channel_info["channel"]
            .as_str()
            .or_else(|| channel_info["uploader"].as_str())
            .or_else(|| channel_info["title"].as_str())
            .unwrap_or("Kenh_YouTube")
            .to_string();

        let channel_subs = if let Some(subs) = channel_info["channel_follower_count"].as_u64() {
            format!("{} subs", subs)
        } else {
            "N/A".to_string()
        };

        let safe_name = sanitize_filename(&channel_title);
        let timestamp = get_simple_timestamp();
        let target_dir = match &options.target_folder {
            Some(f) if !f.trim().is_empty() => std::path::PathBuf::from(f),
            _ => {
                let dl = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
                dl.join("ViewIndex_Audit").join(format!("{}_{}", safe_name, timestamp))
            }
        };

        std::fs::create_dir_all(&target_dir).map_err(|e| format!("Lỗi tạo thư mục đích: {}", e))?;

        let channel_json_path = target_dir.join("channel.json");
        let _ = std::fs::write(&channel_json_path, serde_json::to_string_pretty(&channel_info).unwrap_or_default());

        let _ = app.emit(
            "channel-audit-progress",
            serde_json::json!({
                "phase": "channel_ready",
                "percent": 8,
                "message": format!("Đã nhận diện kênh '{}' ({}). Đang chuẩn bị quét...", channel_title, channel_subs)
            }),
        );

        let tab_count = selected_tabs.len();
        for (tab_idx, tab) in selected_tabs.iter().enumerate() {
            if AUDIT_CANCELLED.load(Ordering::SeqCst) {
                return Err("Quá trình quét kênh đã bị người dùng hủy bỏ.".to_string());
            }

            let tab_dir = target_dir.join(tab);
            std::fs::create_dir_all(&tab_dir).map_err(|e| format!("Lỗi tạo thư mục tab {}: {}", tab, e))?;

            let tab_url = format!("{}/{}", base_url.trim_end_matches('/'), tab);

            let tab_percent_base = 10 + (tab_idx * 75 / tab_count);
            let _ = app.emit(
                "channel-audit-progress",
                serde_json::json!({
                    "phase": "tab_crawling",
                    "tab": tab,
                    "percent": tab_percent_base,
                    "message": format!("Đang cào dữ liệu tab '{}'...", tab)
                }),
            );

            let mut cmd = Command::new(get_tool_path("yt-dlp"));
            cmd.arg("--skip-download")
                .arg("--write-info-json")
                .arg("--ignore-errors")
                .arg("--no-overwrites")
                .arg("--no-warnings");

            if let Some(ffdir) = get_ffmpeg_dir() {
                cmd.arg("--ffmpeg-location").arg(&ffdir);
            }

            if options.anti_bot_delay {
                cmd.arg("--sleep-interval").arg("1").arg("--max-sleep-interval").arg("3");
            }

            let out_pattern = format!("{}/%(upload_date)s_%(id)s.%(ext)s", tab_dir.to_string_lossy().replace('\\', "/"));
            cmd.arg("-o").arg(&out_pattern);

            if options.limit > 0 {
                cmd.arg("--playlist-end").arg(options.limit.to_string());
            }

            if options.download_thumbnails {
                cmd.arg("--write-thumbnail").arg("--convert-thumbnails").arg("jpg");
            }

            if options.download_subs {
                cmd.arg("--write-auto-subs")
                    .arg("--sub-langs")
                    .arg("vi.*,en.*")
                    .arg("--sub-format")
                    .arg("vtt");
            }

            if options.download_comments {
                cmd.arg("--write-comments")
                    .arg("--extractor-args")
                    .arg("youtube:max_comments=100,100,0,0;comment_sort=top");
            }

            if let Some(ref browser) = options.cookies_browser {
                let b = browser.trim();
                if !b.is_empty() && b != "none" {
                    cmd.arg("--cookies-from-browser").arg(b);
                }
            }

            cmd.arg("--").arg(&tab_url);

            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
            #[cfg(target_os = "windows")]
            cmd.creation_flags(CREATE_NO_WINDOW);

            let mut child = cmd.spawn().map_err(|e| format!("Lỗi khởi chạy yt-dlp cho tab {}: {}", tab, e))?;

            if let Ok(mut guard) = AUDIT_PID.lock() {
                *guard = Some(child.id());
            }

            if let Some(stdout) = child.stdout.take() {
                let reader = BufReader::new(stdout);
                for line_result in reader.lines() {
                    if AUDIT_CANCELLED.load(Ordering::SeqCst) {
                        break;
                    }
                    if let Ok(line) = line_result {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let _ = app.emit(
                                "channel-audit-progress",
                                serde_json::json!({
                                    "phase": "tab_crawling",
                                    "tab": tab,
                                    "log": trimmed,
                                    "message": trimmed
                                }),
                            );
                        }
                    }
                }
            }

            let _ = child.wait();

            if let Ok(mut guard) = AUDIT_PID.lock() {
                *guard = None;
            }
        }

        if AUDIT_CANCELLED.load(Ordering::SeqCst) {
            return Err("Quá trình quét kênh đã bị người dùng hủy bỏ.".to_string());
        }

        let _ = app.emit(
            "channel-audit-progress",
            serde_json::json!({
                "phase": "processing",
                "percent": 88,
                "message": "Đang tổng hợp dữ liệu, tạo bảng CSV và kịch bản cho AI..."
            }),
        );

        let mut all_rows: Vec<serde_json::Value> = Vec::new();
        let mut videos_count = 0;
        let mut shorts_count = 0;
        let mut streams_count = 0;
        let mut total_views: u64 = 0;
        let mut total_likes: u64 = 0;
        let mut total_comments: u64 = 0;

        let mut ai_transcripts = String::new();
        ai_transcripts.push_str(&format!("# TỔNG HỢP KỊCH BẢN / PHỤ ĐỀ KÊNH: {}\n", channel_title));
        ai_transcripts.push_str(&format!("# NGÀY QUÉT: {}\n\n", timestamp));

        for tab in &selected_tabs {
            let tab_dir = target_dir.join(tab);
            if let Ok(entries) = std::fs::read_dir(&tab_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let path_str = path.to_string_lossy().to_string();
                    if path_str.ends_with(".info.json") {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            if let Ok(d) = serde_json::from_str::<serde_json::Value>(&content) {
                                let id = d["id"].as_str().unwrap_or("").to_string();
                                let title = d["title"].as_str().unwrap_or("").to_string();
                                let url = d["webpage_url"]
                                    .as_str()
                                    .map(|u| u.to_string())
                                    .unwrap_or_else(|| format!("https://www.youtube.com/watch?v={}", id));
                                let raw_date = d["upload_date"].as_str().unwrap_or("");
                                let formatted_date = if raw_date.len() == 8 {
                                    format!("{}-{}-{}", &raw_date[0..4], &raw_date[4..6], &raw_date[6..8])
                                } else {
                                    raw_date.to_string()
                                };
                                let duration = d["duration"].as_u64().unwrap_or(0);
                                let views = d["view_count"].as_u64().unwrap_or(0);
                                let likes = d["like_count"].as_u64().unwrap_or(0);
                                let comments_cnt = d["comment_count"].as_u64().unwrap_or(0);

                                total_views += views;
                                total_likes += likes;
                                total_comments += comments_cnt;

                                match tab.as_str() {
                                    "shorts" => shorts_count += 1,
                                    "streams" => streams_count += 1,
                                    _ => videos_count += 1,
                                }

                                let categories = d["categories"].as_array().map(|arr| {
                                    arr.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>().join("; ")
                                }).unwrap_or_default();

                                let tags = d["tags"].as_array().map(|arr| {
                                    arr.iter().filter_map(|t| t.as_str()).collect::<Vec<_>>().join("; ")
                                }).unwrap_or_default();

                                let chapter_count = d["chapters"].as_array().map(|c| c.len()).unwrap_or(0);
                                let live_status = d["live_status"].as_str().unwrap_or("not_live").to_string();
                                let age_limit = d["age_limit"].as_i64().unwrap_or(0);
                                let availability = d["availability"].as_str().unwrap_or("public").to_string();

                                let base_prefix = path_str.trim_end_matches(".info.json");
                                let thumb_file = format!("{}.jpg", base_prefix);
                                let has_thumb = std::path::Path::new(&thumb_file).exists();

                                let mut has_subs = false;
                                let mut transcript_text = String::new();
                                if let Ok(dir_entries) = std::fs::read_dir(&tab_dir) {
                                    for de in dir_entries.flatten() {
                                        let p = de.path();
                                        let p_name = p.file_name().unwrap_or_default().to_string_lossy();
                                        let stem = p.file_stem().unwrap_or_default().to_string_lossy();
                                        if (p_name.ends_with(".vtt") || p_name.ends_with(".srt")) && stem.contains(&id) {
                                            has_subs = true;
                                            if let Ok(vtt_raw) = std::fs::read_to_string(&p) {
                                                transcript_text = clean_vtt_to_plain_text(&vtt_raw);
                                                break;
                                            }
                                        }
                                    }
                                }

                                if !transcript_text.is_empty() {
                                    ai_transcripts.push_str("==================================================\n");
                                    ai_transcripts.push_str(&format!("VIDEO: {}\nURL: {}\nNGÀY ĐĂNG: {} | LƯỢT XEM: {} | LIKES: {}\n", title, url, formatted_date, views, likes));
                                    ai_transcripts.push_str("==================================================\n");
                                    ai_transcripts.push_str(&transcript_text);
                                    ai_transcripts.push_str("\n\n");
                                }

                                let top_comments_summary = if let Some(comments) = d["comments"].as_array() {
                                    comments.iter().take(5).filter_map(|c| {
                                        c["text"].as_str().map(|t| t.replace('\n', " ").replace('\r', "").trim().to_string())
                                    }).collect::<Vec<_>>().join(" || ")
                                } else {
                                    String::new()
                                };

                                let description = d["description"].as_str().unwrap_or("").replace('\r', " ").replace('\n', " ");

                                all_rows.push(serde_json::json!({
                                    "tab": tab,
                                    "id": id,
                                    "url": url,
                                    "upload_date": formatted_date,
                                    "raw_date": raw_date,
                                    "title": title,
                                    "duration_sec": duration,
                                    "duration_formatted": format_duration_hms(duration),
                                    "view_count": views,
                                    "like_count": likes,
                                    "comment_count": comments_cnt,
                                    "categories": categories,
                                    "tags": tags,
                                    "chapter_count": chapter_count,
                                    "live_status": live_status,
                                    "age_limit": age_limit,
                                    "availability": availability,
                                    "has_auto_subs": has_subs,
                                    "thumbnail_file": if has_thumb { thumb_file } else { String::new() },
                                    "top_comments": top_comments_summary,
                                    "description": description
                                }));
                            }
                        }
                    }
                }
            }
        }

        all_rows.sort_by(|a, b| {
            let va = a["view_count"].as_u64().unwrap_or(0);
            let vb = b["view_count"].as_u64().unwrap_or(0);
            vb.cmp(&va)
        });

        let total_crawled = all_rows.len();

        let csv_path = target_dir.join("videos.csv");
        let mut csv_content = String::from("\u{FEFF}");
        csv_content.push_str("Tab,ID,URL,Ngày đăng,Tiêu đề,Thời lượng (giây),Thời lượng,Lượt xem,Lượt thích,Số bình luận,Danh mục,Tags,Số Chapters,Trạng thái Live,Giới hạn tuổi,Khả dụng,Có Transcript,Tệp Thumbnail,Top Bình luận,Mô tả\n");

        for r in &all_rows {
            let line = format!(
                "{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                escape_csv_cell(r["tab"].as_str().unwrap_or("")),
                escape_csv_cell(r["id"].as_str().unwrap_or("")),
                escape_csv_cell(r["url"].as_str().unwrap_or("")),
                escape_csv_cell(r["upload_date"].as_str().unwrap_or("")),
                escape_csv_cell(r["title"].as_str().unwrap_or("")),
                r["duration_sec"].as_u64().unwrap_or(0),
                escape_csv_cell(r["duration_formatted"].as_str().unwrap_or("")),
                r["view_count"].as_u64().unwrap_or(0),
                r["like_count"].as_u64().unwrap_or(0),
                r["comment_count"].as_u64().unwrap_or(0),
                escape_csv_cell(r["categories"].as_str().unwrap_or("")),
                escape_csv_cell(r["tags"].as_str().unwrap_or("")),
                r["chapter_count"].as_u64().unwrap_or(0),
                escape_csv_cell(r["live_status"].as_str().unwrap_or("")),
                r["age_limit"].as_i64().unwrap_or(0),
                escape_csv_cell(r["availability"].as_str().unwrap_or("")),
                if r["has_auto_subs"].as_bool().unwrap_or(false) { "Có" } else { "Không" },
                escape_csv_cell(r["thumbnail_file"].as_str().unwrap_or("")),
                escape_csv_cell(r["top_comments"].as_str().unwrap_or("")),
                escape_csv_cell(r["description"].as_str().unwrap_or("")),
            );
            csv_content.push_str(&line);
        }
        let _ = std::fs::write(&csv_path, csv_content);

        let ai_transcripts_path = target_dir.join("transcripts_for_ai.txt");
        let _ = std::fs::write(&ai_transcripts_path, ai_transcripts);

        let summary_json_path = target_dir.join("summary.json");
        let summary_data = serde_json::json!({
            "channel_title": channel_title,
            "channel_subscribers": channel_subs,
            "total_crawled": total_crawled,
            "videos_count": videos_count,
            "shorts_count": shorts_count,
            "streams_count": streams_count,
            "total_views": total_views,
            "total_likes": total_likes,
            "total_comments": total_comments,
            "folder": target_dir.to_string_lossy().to_string(),
            "csv_file": csv_path.to_string_lossy().to_string(),
            "ai_transcripts_file": ai_transcripts_path.to_string_lossy().to_string()
        });
        let _ = std::fs::write(&summary_json_path, serde_json::to_string_pretty(&summary_data).unwrap_or_default());

        let _ = app.emit(
            "channel-audit-progress",
            serde_json::json!({
                "phase": "done",
                "percent": 100,
                "message": format!("Đã hoàn tất quét {} video! Đã tạo videos.csv và transcripts_for_ai.txt", total_crawled),
                "folder": target_dir.to_string_lossy().to_string()
            }),
        );

        let sample_videos: Vec<serde_json::Value> = all_rows.into_iter().take(50).collect();

        Ok(ChannelAuditResult {
            folder: target_dir.to_string_lossy().to_string(),
            csv_file: csv_path.to_string_lossy().to_string(),
            channel_json: channel_json_path.to_string_lossy().to_string(),
            ai_transcripts_file: ai_transcripts_path.to_string_lossy().to_string(),
            total_crawled,
            videos_count,
            shorts_count,
            streams_count,
            total_views,
            total_likes,
            total_comments,
            channel_title,
            channel_subscribers: channel_subs,
            sample_videos,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
async fn download_media(
    app: AppHandle,
    url: String,
    kind: String,
    output_path: Option<String>,
    cookies_browser: Option<String>,
) -> Result<String, String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err("URL không hợp lệ.".to_string());
    }

    let target_file_path = output_path.unwrap_or_else(|| {
        let dl = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        dl.join("%(title)s.%(ext)s").to_string_lossy().to_string()
    });

    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("--newline");
    cmd.arg("--progress-template");
    cmd.arg("download:PROGRESS:%(progress._percent_str)s|%(progress._speed_str)s|%(progress._eta_str)s|%(progress._total_bytes_estimate_str)s");
    cmd.arg("--no-warnings");
    cmd.arg("--no-playlist");

    if let Some(ffdir) = get_ffmpeg_dir() {
        cmd.arg("--ffmpeg-location").arg(&ffdir);
    }

    let mut out_flag_path = target_file_path.clone();

    if kind.starts_with("format_") {
        let format_id = kind.trim_start_matches("format_");
        cmd.arg("-f").arg(format_id);
    } else {
        match kind.as_str() {
            "video_1080p" => {
                cmd.arg("-f")
                   .arg("bestvideo[height<=1080][ext=mp4]+bestaudio[ext=m4a]/best[height<=1080][ext=mp4]/best")
                   .arg("--merge-output-format")
                   .arg("mp4");
            }
            "video_720p" => {
                cmd.arg("-f")
                   .arg("bestvideo[height<=720][ext=mp4]+bestaudio[ext=m4a]/best[height<=720][ext=mp4]/best")
                   .arg("--merge-output-format")
                   .arg("mp4");
            }
            "video_best" => {
                cmd.arg("-f")
                    .arg("bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best")
                    .arg("--merge-output-format")
                    .arg("mp4");
            }
            "audio_mp3" => {
                cmd.arg("-f")
                    .arg("ba/b")
                    .arg("-x")
                    .arg("--audio-format")
                    .arg("mp3")
                    .arg("--audio-quality")
                    .arg("320K");
            }
            "thumbnail" => {
                cmd.arg("--skip-download")
                    .arg("--write-thumbnail")
                    .arg("--convert-thumbnails")
                    .arg("png");
                if out_flag_path.to_lowercase().ends_with(".png") {
                    out_flag_path = out_flag_path[..out_flag_path.len() - 4].to_string();
                }
            }
            kind if kind.starts_with("subtitles") => {
                cmd.arg("--skip-download")
                    .arg("--write-sub")
                    .arg("--write-auto-sub")
                    .arg("--sub-format")
                    .arg("srt")
                    .arg("--ignore-errors");

                let langs = if kind.starts_with("subtitles:") {
                    kind.trim_start_matches("subtitles:").to_string()
                } else {
                    "en,vi".to_string()
                };
                cmd.arg("--sub-lang").arg(&langs);

                if out_flag_path.to_lowercase().ends_with(".srt") {
                    out_flag_path = out_flag_path[..out_flag_path.len() - 4].to_string();
                }
            }
            _ => {
                cmd.arg("-f").arg("best");
            }
        }
    }

    cmd.arg("-o").arg(&out_flag_path);

    if let Some(browser) = cookies_browser {
        let b = browser.trim();
        if !b.is_empty() && b != "none" {
            cmd.arg("--cookies-from-browser").arg(b);
        }
    }

    cmd.arg("--").arg(trimmed_url);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => return Err(format!("Lỗi khi kích hoạt tiến trình tải về: {}", e)),
    };

    let stdout = child
        .stdout
        .take()
        .ok_or("Không thể đọc tiến trình stdout")?;
    let stderr = child.stderr.take();

    let stderr_handle = std::thread::spawn(move || {
        let mut err_str = String::new();
        if let Some(err_stream) = stderr {
            let reader = BufReader::new(err_stream);
            for line in reader.lines().flatten() {
                err_str.push_str(&line);
                err_str.push('\n');
            }
        }
        err_str
    });

    // Initial progress event
    let _ = app.emit(
        "download-progress",
        ProgressPayload {
            percent: 0.0,
            speed: "--".to_string(),
            eta: "--".to_string(),
            total: "--".to_string(),
            status: "Bắt đầu kết nối tải tệp...".to_string(),
            is_done: false,
            is_error: false,
            file_path: target_file_path.clone(),
        },
    );

    let reader = BufReader::new(stdout);
    for line in reader.lines().flatten() {
        let trimmed = line.trim();
        if trimmed.contains("PROGRESS:") {
            let parts: Vec<&str> = trimmed
                .split("PROGRESS:")
                .nth(1)
                .unwrap_or("")
                .split('|')
                .collect();
            if parts.len() >= 4 {
                let pct_str = parts[0].replace('%', "").trim().to_string();
                let pct = pct_str.parse::<f32>().unwrap_or(0.0);
                let speed = parts[1].trim().to_string();
                let eta = parts[2].trim().to_string();
                let total = parts[3].trim().to_string();

                let _ = app.emit(
                    "download-progress",
                    ProgressPayload {
                        percent: pct,
                        speed,
                        eta,
                        total,
                        status: format!("Đang tải về: {:.1}%", pct),
                        is_done: false,
                        is_error: false,
                        file_path: target_file_path.clone(),
                    },
                );
            }
        } else if trimmed.contains("[Merger]") || trimmed.contains("Merging") {
            let _ = app.emit(
                "download-progress",
                ProgressPayload {
                    percent: 98.0,
                    speed: "--".to_string(),
                    eta: "--".to_string(),
                    total: "--".to_string(),
                    status: "Đang ghép video và âm thanh qua ffmpeg...".to_string(),
                    is_done: false,
                    is_error: false,
                    file_path: target_file_path.clone(),
                },
            );
        } else if trimmed.contains("[ExtractAudio]") {
            let _ = app.emit(
                "download-progress",
                ProgressPayload {
                    percent: 98.0,
                    speed: "--".to_string(),
                    eta: "--".to_string(),
                    total: "--".to_string(),
                    status: "Đang chuyển đổi sang định dạng MP3 320kbps...".to_string(),
                    is_done: false,
                    is_error: false,
                    file_path: target_file_path.clone(),
                },
            );
        }
    }

    let _status = child
        .wait()
        .map_err(|e| format!("Lỗi khi chờ tiến trình hoàn tất: {}", e))?;
    let err_output = stderr_handle.join().unwrap_or_default();

    let resolved = resolve_actual_file_path(&target_file_path);
    let file_exists = resolved.is_some();

    if file_exists {
        let mut final_path = resolved.unwrap();

        // If user specifically requested an .srt file and yt-dlp produced an .en.srt / .vi.srt file:
        // Automatically rename it to the exact target_file_path if target_file_path does not exist!
        if target_file_path.to_lowercase().ends_with(".srt")
            && !std::path::Path::new(&target_file_path).exists()
        {
            if std::fs::rename(&final_path, &target_file_path).is_ok() {
                final_path = target_file_path.clone();
            }
        }

        let _ = app.emit(
            "download-progress",
            ProgressPayload {
                percent: 100.0,
                speed: "--".to_string(),
                eta: "00:00".to_string(),
                total: "100%".to_string(),
                status: "Tải về hoàn tất thành công!".to_string(),
                is_done: true,
                is_error: false,
                file_path: final_path.clone(),
            },
        );
        Ok(format!("Đã tải thành công về: {}", final_path))
    } else {
        let _ = app.emit(
            "download-progress",
            ProgressPayload {
                percent: 0.0,
                speed: "--".to_string(),
                eta: "--".to_string(),
                total: "--".to_string(),
                status: "Tải về thất bại (Không tạo được tệp)!".to_string(),
                is_done: true,
                is_error: true,
                file_path: target_file_path.clone(),
            },
        );
        let msg = if err_output.contains("HTTP Error 429") {
            "YouTube giới hạn tải phụ đề (HTTP 429). Video này không có phụ đề khả dụng vào lúc này.".to_string()
        } else if kind.starts_with("subtitles") {
            "Không thể tìm thấy hoặc tải phụ đề cho video này (YouTube không có phụ đề khả dụng)."
                .to_string()
        } else if err_output.trim().is_empty() {
            "yt-dlp không thể tạo tệp tải về.".to_string()
        } else {
            format_ytdlp_error(&err_output)
        };
        Err(msg)
    }
}

fn resolve_actual_file_path(path: &str) -> Option<String> {
    let p = std::path::Path::new(path);
    if p.exists() {
        return Some(path.to_string());
    }
    if let Some(parent) = p.parent() {
        if let Some(file_stem) = p.file_stem().and_then(|s| s.to_str()) {
            if let Ok(entries) = std::fs::read_dir(parent) {
                let mut candidates = Vec::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.starts_with(file_stem)
                        && !name.ends_with(".part")
                        && !name.ends_with(".ytdl")
                        && !name.ends_with(".temp")
                    {
                        candidates.push(entry.path().to_string_lossy().to_string());
                    }
                }
                // Prefer candidate matching target extension
                if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                    let dot_ext = format!(".{}", ext.to_lowercase());
                    if let Some(matched) = candidates
                        .iter()
                        .find(|c| c.to_lowercase().ends_with(&dot_ext))
                    {
                        return Some(matched.clone());
                    }
                }
                if let Some(first) = candidates.into_iter().next() {
                    return Some(first);
                }
            }
        }
    }
    None
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("Không thể đọc tệp: {}", e))
}

#[tauri::command]
fn open_file(path: String) -> Result<(), String> {
    if let Some(target) = resolve_actual_file_path(&path) {
        let mut cmd = Command::new("explorer");
        cmd.arg(&target);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.spawn()
            .map_err(|e| format!("Không thể mở tệp: {}", e))?;
        Ok(())
    } else {
        Err("Tệp không tồn tại hoặc đã bị di chuyển.".to_string())
    }
}

#[tauri::command]
fn open_file_folder(path: String) -> Result<(), String> {
    let resolved = resolve_actual_file_path(&path).unwrap_or(path);
    let p = std::path::Path::new(&resolved);
    let win_path = resolved.replace('/', "\\");

    let mut cmd = Command::new("explorer");
    #[cfg(target_os = "windows")]
    {
        if p.exists() && p.is_file() {
            cmd.raw_arg(format!("/select,\"{}\"", win_path));
        } else {
            let parent_dir = if p.is_dir() {
                p.to_path_buf()
            } else {
                p.parent().unwrap_or(p).to_path_buf()
            };
            let win_dir = parent_dir.to_string_lossy().to_string().replace('/', "\\");
            cmd.raw_arg(format!("\"{}\"", win_dir));
        }
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(target_os = "windows"))]
    {
        let parent = p.parent().unwrap_or(p);
        cmd.arg(parent);
    }

    cmd.spawn()
        .map_err(|e| format!("Không thể mở thư mục: {}", e))?;
    Ok(())
}

#[tauri::command]
fn open_file_path(path: String) -> Result<(), String> {
    open_file_folder(path)
}

#[tauri::command]
fn open_in_browser(url: String) -> Result<(), String> {
    let trimmed = url.trim();
    if !trimmed.starts_with("https://") && !trimmed.starts_with("http://") {
        return Err("Chỉ cho phép mở các liên kết web http:// hoặc https:// hợp lệ.".to_string());
    }
    let mut cmd = Command::new("explorer");
    cmd.arg(trimmed);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

    cmd.spawn()
        .map_err(|e| format!("Không thể mở trình duyệt: {}", e))?;
    Ok(())
}

#[tauri::command]
async fn check_app_update(app: AppHandle) -> Result<Option<String>, String> {
    if let Ok(updater) = app.updater() {
        match updater.check().await {
            Ok(Some(update)) => Ok(Some(update.version)),
            Ok(None) => Ok(None),
            Err(e) => Err(format!("Lỗi kiểm tra cập nhật: {}", e)),
        }
    } else {
        Ok(None)
    }
}

#[tauri::command]
async fn apply_app_update(app: AppHandle) -> Result<(), String> {
    if let Ok(updater) = app.updater() {
        if let Ok(Some(update)) = updater.check().await {
            update
                .download_and_install(|_downloaded, _total| {}, || {})
                .await
                .map_err(|e| format!("Lỗi tải cài đặt bản cập nhật: {}", e))?;
            app.restart();
        }
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            for (_label, w) in app.webview_windows() {
                let _ = w.show();
                let _ = w.set_focus();
                let _ = w.unminimize();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            check_system,
            update_ytdlp,
            analyze_url,
            select_save_path,
            select_folder,
            download_media,
            download_all_thumbnails,
            download_all_subtitles,
            start_channel_audit,
            cancel_channel_audit,
            read_text_file,
            open_file,
            open_file_folder,
            open_file_path,
            open_in_browser,
            check_app_update,
            apply_app_update
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
