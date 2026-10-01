use std::process::{Command, Stdio};
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;
use std::io::{BufRead, BufReader};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

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
        ffmpeg_path.parent().map(|p| p.to_string_lossy().to_string())
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
        let text = format!("{} {}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
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

#[tauri::command]
async fn analyze_url(url: String, cookies_browser: Option<String>) -> Result<serde_json::Value, String> {
    let trimmed_url = url.trim();
    if trimmed_url.is_empty() {
        return Err("Vui lòng nhập đường dẫn URL YouTube hợp lệ.".to_string());
    }

    let is_watch_video = trimmed_url.contains("/watch") || trimmed_url.contains("youtu.be/");
    let is_playlist_or_channel = !is_watch_video && (
        trimmed_url.contains("/@")
        || trimmed_url.contains("/channel/")
        || trimmed_url.contains("/c/")
        || trimmed_url.contains("/playlist")
        || trimmed_url.contains("/user/")
    );

    let mut cmd = Command::new(get_tool_path("yt-dlp"));
    cmd.arg("--dump-single-json")
       .arg("--no-warnings");

    if let Some(ffdir) = get_ffmpeg_dir() {
        cmd.arg("--ffmpeg-location").arg(&ffdir);
    }

    if is_playlist_or_channel {
        cmd.arg("--flat-playlist")
           .arg("--playlist-end")
           .arg("60");
    } else {
        cmd.arg("--skip-download")
           .arg("--no-playlist");
    }

    if let Some(browser) = cookies_browser {
        let b = browser.trim();
        if !b.is_empty() && b != "none" {
            cmd.arg("--cookies-from-browser").arg(b);
        }
    }

    cmd.arg("--").arg(trimmed_url);

    #[cfg(target_os = "windows")]
    cmd.creation_flags(CREATE_NO_WINDOW);

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
        let err_msg = if stderr.trim().is_empty() {
            format!("yt-dlp kết thúc với mã lỗi: {:?}", output.status.code())
        } else {
            stderr.trim().to_string()
        };
        return Err(err_msg);
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    match serde_json::from_str::<serde_json::Value>(&stdout_str) {
        Ok(json) => Ok(json),
        Err(e) => Err(format!("Lỗi khi phân giải dữ liệu JSON từ yt-dlp: {}", e)),
    }
}

#[tauri::command]
fn select_save_path(default_filename: String, filter_ext: String) -> Option<String> {
    let mut dialog = rfd::FileDialog::new()
        .set_file_name(&default_filename);

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
    dialog.pick_folder().map(|p| p.to_string_lossy().to_string())
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
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7",
        "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let upper = cleaned.to_uppercase();
    if reserved.iter().any(|&r| upper == r || upper.starts_with(&format!("{}.", r))) {
        format!("{}_safe", cleaned)
    } else if cleaned.is_empty() {
        "unnamed".to_string()
    } else {
        cleaned
    }
}

#[tauri::command]
async fn download_all_thumbnails(
    app: AppHandle,
    folder: String,
    channel_name: String,
    videos: Vec<serde_json::Value>,
) -> Result<String, String> {
    let safe_channel = sanitize_filename(&channel_name);
    let target_dir = std::path::Path::new(&folder).join(&safe_channel);
    std::fs::create_dir_all(&target_dir)
        .map_err(|e| format!("Lỗi tạo thư mục: {}", e))?;

    let total = videos.len();
    let mut downloaded: u32 = 0;

    for (idx, video) in videos.iter().enumerate() {
        let id = video["id"].as_str().unwrap_or("");
        let title = video["title"].as_str().unwrap_or("video");
        if id.is_empty() {
            continue;
        }

        let url = format!("https://i.ytimg.com/vi/{}/hqdefault.jpg", id);
        let safe_title = sanitize_filename(title);
        let filename = format!("{:03} - {}.jpg", idx + 1, safe_title);
        let filepath = target_dir.join(&filename);

        // Download using curl (built-in on Windows 10+)
        let mut cmd = Command::new("curl");
        cmd.args(["-L", "-s", "-o", &filepath.to_string_lossy(), &url]);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        if let Ok(output) = cmd.output() {
            if output.status.success() && filepath.exists() {
                downloaded += 1;
            }
        }

        // Emit progress to frontend
        let _ = app.emit("thumbnail-progress", serde_json::json!({
            "current": idx + 1,
            "total": total,
            "downloaded": downloaded,
            "filename": filename
        }));
    }

    Ok(target_dir.to_string_lossy().to_string())
}
#[tauri::command]
async fn download_media(
    app: AppHandle,
    url: String,
    kind: String,
    output_path: Option<String>,
    cookies_browser: Option<String>
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

    let stdout = child.stdout.take().ok_or("Không thể đọc tiến trình stdout")?;
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
    let _ = app.emit("download-progress", ProgressPayload {
        percent: 0.0,
        speed: "--".to_string(),
        eta: "--".to_string(),
        total: "--".to_string(),
        status: "Bắt đầu kết nối tải tệp...".to_string(),
        is_done: false,
        is_error: false,
        file_path: target_file_path.clone(),
    });

    let reader = BufReader::new(stdout);
    for line in reader.lines().flatten() {
        let trimmed = line.trim();
        if trimmed.contains("PROGRESS:") {
            let parts: Vec<&str> = trimmed.split("PROGRESS:").nth(1).unwrap_or("").split('|').collect();
            if parts.len() >= 4 {
                let pct_str = parts[0].replace('%', "").trim().to_string();
                let pct = pct_str.parse::<f32>().unwrap_or(0.0);
                let speed = parts[1].trim().to_string();
                let eta = parts[2].trim().to_string();
                let total = parts[3].trim().to_string();

                let _ = app.emit("download-progress", ProgressPayload {
                    percent: pct,
                    speed,
                    eta,
                    total,
                    status: format!("Đang tải về: {:.1}%", pct),
                    is_done: false,
                    is_error: false,
                    file_path: target_file_path.clone(),
                });
            }
        } else if trimmed.contains("[Merger]") || trimmed.contains("Merging") {
            let _ = app.emit("download-progress", ProgressPayload {
                percent: 98.0,
                speed: "--".to_string(),
                eta: "--".to_string(),
                total: "--".to_string(),
                status: "Đang ghép video và âm thanh qua ffmpeg...".to_string(),
                is_done: false,
                is_error: false,
                file_path: target_file_path.clone(),
            });
        } else if trimmed.contains("[ExtractAudio]") {
            let _ = app.emit("download-progress", ProgressPayload {
                percent: 98.0,
                speed: "--".to_string(),
                eta: "--".to_string(),
                total: "--".to_string(),
                status: "Đang chuyển đổi sang định dạng MP3 320kbps...".to_string(),
                is_done: false,
                is_error: false,
                file_path: target_file_path.clone(),
            });
        }
    }

    let _status = child.wait().map_err(|e| format!("Lỗi khi chờ tiến trình hoàn tất: {}", e))?;
    let err_output = stderr_handle.join().unwrap_or_default();

    let resolved = resolve_actual_file_path(&target_file_path);
    let file_exists = resolved.is_some();

    if file_exists {
        let mut final_path = resolved.unwrap();

        // If user specifically requested an .srt file and yt-dlp produced an .en.srt / .vi.srt file:
        // Automatically rename it to the exact target_file_path if target_file_path does not exist!
        if target_file_path.to_lowercase().ends_with(".srt") && !std::path::Path::new(&target_file_path).exists() {
            if std::fs::rename(&final_path, &target_file_path).is_ok() {
                final_path = target_file_path.clone();
            }
        }

        let _ = app.emit("download-progress", ProgressPayload {
            percent: 100.0,
            speed: "--".to_string(),
            eta: "00:00".to_string(),
            total: "100%".to_string(),
            status: "Tải về hoàn tất thành công!".to_string(),
            is_done: true,
            is_error: false,
            file_path: final_path.clone(),
        });
        Ok(format!("Đã tải thành công về: {}", final_path))
    } else {
        let _ = app.emit("download-progress", ProgressPayload {
            percent: 0.0,
            speed: "--".to_string(),
            eta: "--".to_string(),
            total: "--".to_string(),
            status: "Tải về thất bại (Không tạo được tệp)!".to_string(),
            is_done: true,
            is_error: true,
            file_path: target_file_path.clone(),
        });
        let msg = if err_output.contains("HTTP Error 429") {
            "YouTube giới hạn tải phụ đề (HTTP 429). Video này không có phụ đề khả dụng vào lúc này.".to_string()
        } else if kind.starts_with("subtitles") {
            "Không thể tìm thấy hoặc tải phụ đề cho video này (YouTube không có phụ đề khả dụng).".to_string()
        } else if err_output.trim().is_empty() {
            "yt-dlp không thể tạo tệp tải về.".to_string()
        } else {
            err_output.trim().to_string()
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
                    if let Some(matched) = candidates.iter().find(|c| c.to_lowercase().ends_with(&dot_ext)) {
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
fn open_file(path: String) -> Result<(), String> {
    if let Some(target) = resolve_actual_file_path(&path) {
        let mut cmd = Command::new("explorer");
        cmd.arg(&target);
        #[cfg(target_os = "windows")]
        cmd.creation_flags(CREATE_NO_WINDOW);

        cmd.spawn().map_err(|e| format!("Không thể mở tệp: {}", e))?;
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

    cmd.spawn().map_err(|e| format!("Không thể mở thư mục: {}", e))?;
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
            update.download_and_install(|_downloaded, _total| {}, || {}).await
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
