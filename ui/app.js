// YouTube Metadata Analyzer v2.0 - Core Frontend Logic

let currentData = null;
let currentFormats = [];
let activeFormatFilter = 'all';
let currentChannelVideos = [];
let channelSortField = null; // null | 'views'
let channelSortDir = 'desc'; // 'desc' | 'asc'

// Tauri invoke helper
async function callTauri(command, args = {}) {
  if (window.__TAURI__ && window.__TAURI__.core) {
    return await window.__TAURI__.core.invoke(command, args);
  }
  throw new Error("Tauri backend chưa sẵn sàng.");
}

// DOM Elements
const navBackBtn = document.getElementById('nav-back-btn');
const navForwardBtn = document.getElementById('nav-forward-btn');
const urlInput = document.getElementById('url-input');
const pasteBtn = document.getElementById('paste-btn');
const clearBtn = document.getElementById('clear-btn');
const sampleVideoBtn = document.getElementById('sample-video-btn');
const sampleChannelBtn = document.getElementById('sample-channel-btn');
const cookieSelect = document.getElementById('cookie-select');
const analyzeBtn = document.getElementById('analyze-btn');

// Navigation History Stack (Back / Forward)
let navStack = [];
let navPointer = -1;
let isNavigatingState = false;
const analyzeBtnText = document.getElementById('analyze-btn-text');
const spinIcon = analyzeBtn.querySelector('.spin-icon');
const actionIcon = analyzeBtn.querySelector('.action-icon');

const errorBanner = document.getElementById('error-banner');
const errorMessage = document.getElementById('error-message');
const closeErrorBtn = document.getElementById('close-error-btn');

const downloadBanner = document.getElementById('download-banner');
const downloadBannerTitle = document.getElementById('download-banner-title');
const downloadProgressPct = document.getElementById('download-progress-pct');
const downloadProgressBar = document.getElementById('download-progress-bar');
const dlStatStatus = document.getElementById('dl-stat-status');
const dlStatSpeed = document.getElementById('dl-stat-speed');
const dlStatEta = document.getElementById('dl-stat-eta');
const dlStatTotal = document.getElementById('dl-stat-total');
const openDownloadedFileBtn = document.getElementById('open-downloaded-file-btn');
const openDownloadedFolderBtn = document.getElementById('open-downloaded-folder-btn');
const closeDownloadBannerBtn = document.getElementById('close-download-banner-btn');
const dlBannerSpinIcon = document.getElementById('dl-banner-spin-icon');
let lastDownloadedFilePath = '';

const emptyState = document.getElementById('empty-state');
const loadingState = document.getElementById('loading-state');
const loadingTitle = document.getElementById('loading-title');
const loadingSubtitle = document.getElementById('loading-subtitle');
const resultContainer = document.getElementById('result-container');
const channelContainer = document.getElementById('channel-container');

const systemStatus = document.getElementById('system-status');
const ytdlpVersionText = document.getElementById('ytdlp-version-text');
const updateYtdlpBtn = document.getElementById('update-ytdlp-btn');
const themeToggleBtn = document.getElementById('theme-toggle-btn');
const themeIcon = document.getElementById('theme-icon');

const historyToggleBtn = document.getElementById('history-toggle-btn');
const historyCountBadge = document.getElementById('history-count');
const historyModal = document.getElementById('history-modal');
const closeHistoryBtn = document.getElementById('close-history-btn');
const historyList = document.getElementById('history-list');
const clearHistoryBtn = document.getElementById('clear-history-btn');

const compareModalBtn = document.getElementById('compare-modal-btn');
const compareModal = document.getElementById('compare-modal');
const closeCompareBtn = document.getElementById('close-compare-btn');
const compareUrl1 = document.getElementById('compare-url-1');
const compareUrl2 = document.getElementById('compare-url-2');
const executeCompareBtn = document.getElementById('execute-compare-btn');
const compareLoading = document.getElementById('compare-loading');
const compareResults = document.getElementById('compare-results');
const compareTbody = document.getElementById('compare-tbody');

const toast = document.getElementById('toast');
const toastMessage = document.getElementById('toast-message');
const dragOverlay = document.getElementById('drag-overlay');

// Initialize
document.addEventListener('DOMContentLoaded', () => {
  initTheme();
  initSystem();
  initEventListeners();
  initDragAndDrop();
  updateHistoryBadge();
  updateNavButtons();
  setTimeout(checkForAppUpdates, 3000);
});

// Auto-updater check for ViewIndex
async function checkForAppUpdates() {
  try {
    const newVersion = await callTauri('check_app_update');
    if (newVersion) {
      const doUpdate = confirm(`🎉 Đã có phiên bản ViewIndex v${newVersion} mới!\n\nBạn có muốn tự động cập nhật ngay bây giờ không?`);
      if (doUpdate) {
        showToast("Đang tải bản cập nhật ngầm, app sẽ tự khởi động lại sau vài giây...");
        await callTauri('apply_app_update');
      }
    }
  } catch (e) {
    console.log("Kiểm tra cập nhật ViewIndex:", e);
  }
}

// System check
async function initSystem() {
  try {
    const status = await callTauri('check_system');
    if (status.ytdlp_installed) {
      systemStatus.className = 'status-chip ready';
      ytdlpVersionText.textContent = `yt-dlp v${status.ytdlp_version}`;
    } else {
      systemStatus.className = 'status-chip error';
      ytdlpVersionText.textContent = 'Chưa cài đặt yt-dlp!';
    }
  } catch (err) {
    console.warn("check_system warning:", err);
    systemStatus.className = 'status-chip ready';
    ytdlpVersionText.textContent = 'yt-dlp sẵn sàng';
  }
}

// Theme handling
function initTheme() {
  const savedTheme = localStorage.getItem('yt_analyzer_theme') || 'dark';
  document.documentElement.setAttribute('data-theme', savedTheme);
  themeIcon.textContent = savedTheme === 'dark' ? '🌙' : '☀️';

  themeToggleBtn.addEventListener('click', () => {
    const current = document.documentElement.getAttribute('data-theme') || 'dark';
    const next = current === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', next);
    localStorage.setItem('yt_analyzer_theme', next);
    themeIcon.textContent = next === 'dark' ? '🌙' : '☀️';
    showToast(`Đã chuyển sang giao diện ${next === 'dark' ? 'Tối' : 'Sáng'}!`);
  });
}

// Navigation History Management (Back / Forward)
function updateNavButtons() {
  if (navBackBtn) {
    navBackBtn.disabled = navPointer <= 0;
  }
  if (navForwardBtn) {
    navForwardBtn.disabled = navPointer >= navStack.length - 1;
  }
}

function restoreNavState(item) {
  if (!item) return;
  isNavigatingState = true;
  try {
    urlInput.value = item.url || '';
    if (urlInput.value) {
      clearBtn.classList.remove('hidden');
    } else {
      clearBtn.classList.add('hidden');
    }
    currentData = item.data;
    if (item.type === 'channel' || item.data._type === 'playlist' || item.data.entries) {
      renderChannelData(item.data);
    } else {
      renderSingleVideoData(item.data);
    }
    window.scrollTo({ top: 0, behavior: 'smooth' });
  } finally {
    isNavigatingState = false;
    updateNavButtons();
  }
}

function navigateBack() {
  if (navPointer > 0) {
    navPointer--;
    restoreNavState(navStack[navPointer]);
  }
}

function navigateForward() {
  if (navPointer < navStack.length - 1) {
    navPointer++;
    restoreNavState(navStack[navPointer]);
  }
}

function pushNavState(url, data, type) {
  if (isNavigatingState) return;
  // If current item matches this url, update its cached data
  if (navPointer >= 0 && navStack[navPointer] && navStack[navPointer].url === url) {
    navStack[navPointer].data = data;
    navStack[navPointer].type = type;
    updateNavButtons();
    return;
  }
  // Truncate forward history if branched
  if (navPointer < navStack.length - 1) {
    navStack = navStack.slice(0, navPointer + 1);
  }
  navStack.push({ url, data, type });
  if (navStack.length > 50) {
    navStack.shift();
  }
  navPointer = navStack.length - 1;
  updateNavButtons();
}

// Event Listeners
function initEventListeners() {
  if (navBackBtn) {
    navBackBtn.addEventListener('click', () => navigateBack());
  }
  if (navForwardBtn) {
    navForwardBtn.addEventListener('click', () => navigateForward());
  }

  // Keyboard navigation shortcuts
  window.addEventListener('keydown', (e) => {
    if (e.altKey && e.key === 'ArrowLeft') {
      e.preventDefault();
      navigateBack();
    } else if (e.altKey && e.key === 'ArrowRight') {
      e.preventDefault();
      navigateForward();
    }
  });

  // Mouse auxiliary back/forward buttons
  window.addEventListener('mouseup', (e) => {
    if (e.button === 3) {
      e.preventDefault();
      navigateBack();
    } else if (e.button === 4) {
      e.preventDefault();
      navigateForward();
    }
  });

  urlInput.addEventListener('input', () => {
    if (urlInput.value.trim().length > 0) {
      clearBtn.classList.remove('hidden');
    } else {
      clearBtn.classList.add('hidden');
    }
  });

  urlInput.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') triggerAnalysis();
  });

  pasteBtn.addEventListener('click', async () => {
    try {
      const text = await navigator.clipboard.readText();
      if (text) {
        urlInput.value = text.trim();
        clearBtn.classList.remove('hidden');
        showToast("Đã dán từ bộ nhớ tạm!");
      }
    } catch (e) {
      showToast("Không thể truy cập clipboard", true);
    }
  });

  clearBtn.addEventListener('click', () => {
    urlInput.value = '';
    clearBtn.classList.add('hidden');
    urlInput.focus();
  });

  sampleVideoBtn.addEventListener('click', () => {
    urlInput.value = 'https://www.youtube.com/watch?v=H5jKIgn4a7Q';
    clearBtn.classList.remove('hidden');
    triggerAnalysis();
  });

  sampleChannelBtn.addEventListener('click', () => {
    urlInput.value = 'https://www.youtube.com/@TED';
    clearBtn.classList.remove('hidden');
    triggerAnalysis();
  });

  analyzeBtn.addEventListener('click', () => triggerAnalysis());

  closeErrorBtn.addEventListener('click', () => errorBanner.classList.add('hidden'));

  // Update yt-dlp button
  updateYtdlpBtn.addEventListener('click', async () => {
    updateYtdlpBtn.disabled = true;
    updateYtdlpBtn.innerHTML = `<span>Đang cập nhật...</span>`;
    showToast("Đang kiểm tra và cập nhật yt-dlp qua internet...");

    try {
      const res = await callTauri('update_ytdlp');
      showToast("Cập nhật thành công: " + res);
      await initSystem();
    } catch (err) {
      showToast("Lỗi cập nhật: " + err, true);
    } finally {
      updateYtdlpBtn.disabled = false;
      updateYtdlpBtn.innerHTML = `
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="23 4 23 10 17 10"></polyline>
          <polyline points="1 20 1 14 7 14"></polyline>
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15"></path>
        </svg>
        <span>Update yt-dlp</span>
      `;
    }
  });

  // 1-Click Media Download buttons
  document.querySelectorAll('.btn-dl').forEach(btn => {
    btn.addEventListener('click', () => {
      const dlType = btn.dataset.dlType;
      executeDownload(dlType);
    });
  });

  closeDownloadBannerBtn.addEventListener('click', () => downloadBanner.classList.add('hidden'));

  openDownloadedFileBtn.addEventListener('click', () => {
    if (lastDownloadedFilePath) {
      callTauri('open_file', { path: lastDownloadedFilePath }).catch(err => {
        showToast("Lỗi mở tệp: " + err, true);
      });
    }
  });

  openDownloadedFolderBtn.addEventListener('click', () => {
    if (lastDownloadedFilePath) {
      callTauri('open_file_folder', { path: lastDownloadedFilePath }).catch(err => {
        showToast("Lỗi mở thư mục: " + err, true);
      });
    }
  });

  // Listen to Tauri real-time download progress events
  if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
    window.__TAURI__.event.listen('download-progress', (event) => {
      updateDownloadProgress(event.payload);
    });
  }

  // Tab switching
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.tab-panel').forEach(p => p.classList.remove('active'));

      btn.classList.add('active');
      const targetPanel = document.getElementById(btn.dataset.tab);
      if (targetPanel) {
        targetPanel.classList.add('active');
        // If switching to player tab, load iframe if not already loaded
        if (btn.dataset.tab === 'tab-player' && currentData && currentData.id) {
          const iframe = document.getElementById('in-app-video-frame');
          if (!iframe.src || !iframe.src.includes(currentData.id)) {
            iframe.src = `https://www.youtube-nocookie.com/embed/${currentData.id}?autoplay=1`;
          }
        }
      }
    });
  });

  // Format filter buttons
  document.querySelectorAll('.filter-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      document.querySelectorAll('.filter-btn').forEach(b => b.classList.remove('active'));
      btn.classList.add('active');
      activeFormatFilter = btn.dataset.filter;
      renderFormatsTable(currentFormats, activeFormatFilter);
    });
  });

  // Export buttons
  document.getElementById('export-json-btn').addEventListener('click', exportFullJson);
  document.getElementById('copy-summary-btn').addEventListener('click', copySummary);
  document.getElementById('copy-all-tags-btn').addEventListener('click', copyAllTags);
  document.getElementById('copy-description-btn').addEventListener('click', () => {
    if (currentData && currentData.description) {
      copyToClipboard(currentData.description, "Đã sao chép nội dung mô tả!");
    }
  });
  document.getElementById('copy-raw-json-btn').addEventListener('click', () => {
    if (currentData) {
      copyToClipboard(JSON.stringify(currentData, null, 2), "Đã sao chép toàn bộ JSON!");
    }
  });
  document.getElementById('copy-chapters-btn').addEventListener('click', copyChapters);
  document.getElementById('copy-thumb-url-btn').addEventListener('click', copyBestThumbnail);
  document.getElementById('export-formats-csv-btn').addEventListener('click', exportFormatsCsv);

  document.getElementById('open-youtube-btn').addEventListener('click', () => {
    if (currentData && currentData.webpage_url) {
      openUrl(currentData.webpage_url);
    }
  });

  // Channel actions
  document.getElementById('channel-open-url-btn').addEventListener('click', () => {
    if (currentData && (currentData.channel_url || currentData.webpage_url)) {
      openUrl(currentData.channel_url || currentData.webpage_url);
    }
  });
  document.getElementById('channel-export-csv-btn').addEventListener('click', exportChannelVideosCsv);

  // Sort by views
  const thViews = document.getElementById('th-views');
  if (thViews) {
    thViews.addEventListener('click', toggleViewsSort);
  }

  // Download all thumbnails
  document.getElementById('channel-dl-thumbnails-btn').addEventListener('click', downloadAllThumbnails);

  // Listen for thumbnail download progress from Rust backend
  if (window.__TAURI__ && window.__TAURI__.event) {
    window.__TAURI__.event.listen('thumbnail-progress', (event) => {
      const d = event.payload;
      const statusEl = document.getElementById('thumb-dl-status');
      const textEl = document.getElementById('thumb-dl-text');
      const barEl = document.getElementById('thumb-dl-bar');
      statusEl.classList.remove('hidden');
      textEl.textContent = `${d.current}/${d.total}`;
      barEl.style.width = `${Math.round((d.current / d.total) * 100)}%`;
    });
  }

  // Filter channel videos
  const filterChannelInput = document.getElementById('filter-channel-videos-input');
  if (filterChannelInput) {
    filterChannelInput.addEventListener('input', (e) => {
      const q = e.target.value.toLowerCase().trim();
      const filtered = currentChannelVideos.filter(v => (v.title || '').toLowerCase().includes(q));
      renderChannelVideosTable(filtered);
    });
  }

  // History modal
  historyToggleBtn.addEventListener('click', openHistoryModal);
  closeHistoryBtn.addEventListener('click', closeHistoryModal);
  document.querySelectorAll('.modal-backdrop').forEach(b => {
    b.addEventListener('click', () => {
      closeHistoryModal();
      closeCompareModal();
    });
  });
  clearHistoryBtn.addEventListener('click', clearHistory);

  // Compare modal
  compareModalBtn.addEventListener('click', openCompareModal);
  closeCompareBtn.addEventListener('click', closeCompareModal);
  executeCompareBtn.addEventListener('click', executeComparison);

  // Dynamic copy field buttons
  document.addEventListener('click', (e) => {
    const copyBtn = e.target.closest('.copy-field-btn');
    if (copyBtn) {
      const targetId = copyBtn.dataset.copyTarget;
      const targetEl = document.getElementById(targetId);
      if (targetEl) {
        copyToClipboard(targetEl.textContent.trim(), "Đã sao chép giá trị!");
      }
    }
  });
}

// Drag & Drop
function initDragAndDrop() {
  window.addEventListener('dragover', (e) => {
    e.preventDefault();
    dragOverlay.classList.remove('hidden');
  });

  window.addEventListener('dragleave', (e) => {
    if (e.clientX === 0 || e.clientY === 0) {
      dragOverlay.classList.add('hidden');
    }
  });

  window.addEventListener('drop', (e) => {
    e.preventDefault();
    dragOverlay.classList.add('hidden');
    const text = e.dataTransfer.getData('text/plain');
    if (text && text.includes('youtube.com')) {
      urlInput.value = text.trim();
      clearBtn.classList.remove('hidden');
      triggerAnalysis();
    }
  });
}

// Trigger Analysis
async function triggerAnalysis(overrideUrl = null) {
  const url = (overrideUrl || urlInput.value).trim();
  if (!url) {
    showError("Vui lòng nhập đường dẫn YouTube cần phân tích.");
    return;
  }

  if (overrideUrl) {
    urlInput.value = overrideUrl;
    clearBtn.classList.remove('hidden');
  }

  hideError();
  const isChannelOrPlaylist = url.includes('/@') || url.includes('/channel/') || url.includes('/c/') || url.includes('/playlist?list=') || url.includes('/user/');
  setLoading(true, isChannelOrPlaylist ? "Đang trích xuất dữ liệu Kênh / Playlist YouTube..." : "Đang kết nối và bóc tách dữ liệu yt-dlp...");

  const cookiesBrowser = cookieSelect.value !== 'none' ? cookieSelect.value : null;

  try {
    const data = await callTauri('analyze_url', { 
      url: url,
      cookiesBrowser: cookiesBrowser
    });

    currentData = data;
    const isChannel = (data._type === 'playlist' || data.entries);
    if (isChannel) {
      renderChannelData(data);
      saveToHistory(data, 'channel', url);
      pushNavState(url, data, 'channel');
    } else {
      renderSingleVideoData(data);
      saveToHistory(data, 'video', url);
      pushNavState(url, data, 'video');
    }
    setLoading(false);
  } catch (err) {
    setLoading(false);
    showError(err.toString());
  }
}

// Render Single Video Data
function renderSingleVideoData(data) {
  channelContainer.classList.add('hidden');
  emptyState.classList.add('hidden');
  resultContainer.classList.remove('hidden');

  // Video Hero Bar
  const bestThumb = getBestThumbnail(data.thumbnails) || data.thumbnail || '';
  document.getElementById('video-thumbnail').src = bestThumb;
  document.getElementById('video-title').textContent = data.title || 'Không có tiêu đề';
  
  const channelNameEl = document.getElementById('channel-name');
  channelNameEl.textContent = data.channel || data.uploader || 'Không rõ kênh';

  const channelUrl = data.channel_url || data.uploader_url || (data.channel_id ? `https://www.youtube.com/channel/${data.channel_id}` : (data.uploader_id ? `https://www.youtube.com/@${data.uploader_id.replace(/^@/, '')}` : null));
  if (channelUrl) {
    channelNameEl.classList.add('clickable-channel');
    channelNameEl.title = 'Bấm để phân tích toàn bộ Kênh này';
    channelNameEl.onclick = (e) => {
      e.preventDefault();
      triggerAnalysis(channelUrl);
    };
  } else {
    channelNameEl.classList.remove('clickable-channel');
    channelNameEl.title = '';
    channelNameEl.onclick = null;
  }
  
  if (data.channel_follower_count) {
    document.getElementById('channel-subscribers').textContent = `${formatNumber(data.channel_follower_count)} người đăng ký`;
    document.getElementById('channel-subscribers').classList.remove('hidden');
  } else {
    document.getElementById('channel-subscribers').classList.add('hidden');
  }

  // Badges
  const durStr = data.duration_string || (data.duration ? formatDuration(data.duration) : '--:--');
  document.getElementById('badge-duration').textContent = durStr;
  
  const liveBadge = document.getElementById('badge-live-status');
  if (data.is_live || data.live_status === 'is_live') {
    liveBadge.textContent = 'TRỰC TIẾP';
    liveBadge.classList.remove('hidden');
  } else if (data.was_live || data.live_status === 'was_live') {
    liveBadge.textContent = 'ĐÃ TỪNG LIVE';
    liveBadge.classList.remove('hidden');
  } else {
    liveBadge.classList.add('hidden');
  }

  document.getElementById('hero-badge-type').textContent = data._type ? data._type.toUpperCase() : 'VIDEO';
  const maxRes = (data.width && data.height) ? `${data.width}x${data.height}` : (data.resolution || 'HD');
  document.getElementById('hero-badge-resolution').textContent = maxRes;
  document.getElementById('hero-badge-upload-date').textContent = formatDateStr(data.upload_date);

  // 1. OVERVIEW TAB
  document.getElementById('ov-video-id').textContent = data.id || '--';
  document.getElementById('ov-duration').textContent = `${durStr} (${data.duration ? data.duration + ' giây' : ''})`;
  document.getElementById('ov-upload-date').textContent = formatDateStr(data.upload_date);
  document.getElementById('ov-timestamp').textContent = data.timestamp ? `${data.timestamp} (${formatTimestamp(data.timestamp)})` : '--';
  document.getElementById('ov-release-date').textContent = data.release_date ? formatDateStr(data.release_date) : (data.release_timestamp ? formatTimestamp(data.release_timestamp) : 'Trùng ngày tải lên');
  document.getElementById('ov-live-status').textContent = data.live_status || (data.is_live ? 'is_live' : 'not_live');
  document.getElementById('ov-was-live').textContent = data.was_live ? 'Có (Was Live)' : 'Không';
  
  const webpageUrlEl = document.getElementById('ov-webpage-url');
  webpageUrlEl.textContent = data.webpage_url || data.original_url || '--';
  webpageUrlEl.href = data.webpage_url || '#';
  webpageUrlEl.onclick = (e) => {
    e.preventDefault();
    if (data.webpage_url) openUrl(data.webpage_url);
  };

  const ovChannelEl = document.getElementById('ov-channel-title');
  ovChannelEl.textContent = data.channel || data.uploader || '--';
  if (channelUrl) {
    ovChannelEl.classList.add('clickable-channel');
    ovChannelEl.title = 'Bấm để phân tích toàn bộ Kênh này';
    ovChannelEl.onclick = (e) => {
      e.preventDefault();
      triggerAnalysis(channelUrl);
    };
  } else {
    ovChannelEl.classList.remove('clickable-channel');
    ovChannelEl.title = '';
    ovChannelEl.onclick = null;
  }
  document.getElementById('ov-channel-id').textContent = data.channel_id || '--';
  document.getElementById('ov-uploader').textContent = data.uploader || '--';
  document.getElementById('ov-uploader-id').textContent = data.uploader_id || '--';
  document.getElementById('ov-age-limit').textContent = data.age_limit ? `${data.age_limit}+ tuổi` : 'Không giới hạn (0+)';
  document.getElementById('ov-license').textContent = data.license || 'Standard YouTube License';
  document.getElementById('ov-language').textContent = data.language ? data.language.toUpperCase() : 'Không rõ';

  renderThumbnailGallery(data.thumbnails || []);

  // 2. STATS TAB
  const views = data.view_count || 0;
  const likes = data.like_count || 0;
  const comments = data.comment_count || 0;

  document.getElementById('stat-views').textContent = formatCompact(views);
  document.getElementById('stat-views-raw').textContent = `${formatNumber(views)} lượt xem`;
  document.getElementById('stat-likes').textContent = formatCompact(likes);
  document.getElementById('stat-likes-raw').textContent = `${formatNumber(likes)} lượt thích`;
  document.getElementById('stat-comments').textContent = formatCompact(comments);
  document.getElementById('stat-comments-raw').textContent = `${formatNumber(comments)} bình luận`;

  const ratio = views > 0 ? ((likes / views) * 100).toFixed(2) + '%' : '0%';
  document.getElementById('stat-like-ratio').textContent = ratio;

  document.getElementById('stat-duration-sec').textContent = data.duration ? `${data.duration}s` : '--';
  document.getElementById('stat-duration-min').textContent = data.duration ? `${(data.duration / 60).toFixed(1)} phút` : '--';
  document.getElementById('stat-max-res').textContent = maxRes;
  document.getElementById('stat-fps').textContent = data.fps ? `${data.fps} fps` : '--';

  // 3. TAGS & SEO TAB (with character counter)
  renderCategories(data.categories || []);
  renderTags(data.tags || []);
  updateTagCounter(data.tags || []);
  document.getElementById('tags-tab-count').textContent = (data.tags || []).length;

  // 4. FORMATS TAB
  currentFormats = data.formats || [];
  document.getElementById('formats-tab-count').textContent = currentFormats.length;
  renderFormatsTable(currentFormats, activeFormatFilter);

  // 5. CHAPTERS TAB
  renderChapters(data.chapters || []);
  document.getElementById('chapters-tab-count').textContent = (data.chapters || []).length;

  // 6. HEATMAP TAB
  renderHeatmap(data.heatmap || []);

  // 7. DESCRIPTION TAB
  document.getElementById('video-description-text').textContent = data.description || 'Video này không có mô tả.';

  // 8. RAW JSON TAB
  document.getElementById('raw-json-code').textContent = JSON.stringify(data, null, 2);

  // Reset in-app player iframe
  const iframe = document.getElementById('in-app-video-frame');
  if (iframe) iframe.src = '';
}

// Render Channel & Playlist Data
function renderChannelData(data) {
  resultContainer.classList.add('hidden');
  emptyState.classList.add('hidden');
  channelContainer.classList.remove('hidden');

  // Reset sort state and thumbnail progress
  channelSortField = null;
  channelSortDir = 'desc';
  document.getElementById('thumb-dl-status').classList.add('hidden');

  document.getElementById('channel-title-text').textContent = data.title || data.channel || 'Kênh YouTube';
  document.getElementById('channel-uploader-id').textContent = data.uploader_id || data.id || '';
  
  if (data.channel_follower_count) {
    document.getElementById('channel-subs-count').textContent = `${formatNumber(data.channel_follower_count)} người đăng ký`;
  } else {
    document.getElementById('channel-subs-count').textContent = 'Kênh / Playlist';
  }

  currentChannelVideos = extractAllVideos(data.entries);
  document.getElementById('channel-total-videos').textContent = `${currentChannelVideos.length} video hiển thị`;
  document.getElementById('channel-videos-count-badge').textContent = currentChannelVideos.length;
  document.getElementById('channel-desc-text').textContent = data.description || 'Không có phần giới thiệu cho kênh này.';

  // Avatar & Banner
  const avatarImg = document.getElementById('channel-avatar-img');
  const bannerWrap = document.getElementById('channel-banner-wrap');
  const bannerImg = document.getElementById('channel-banner-img');

  const thumbs = data.thumbnails || [];
  let avatarUrl = '';
  let bannerUrl = '';

  thumbs.forEach(t => {
    if (t.id === 'avatar_uncropped' || (t.width && t.width === t.height && t.width > 200)) {
      avatarUrl = t.url;
    } else if (t.id === 'banner_uncropped' || (t.width && t.width > 1000)) {
      bannerUrl = t.url;
    }
  });

  if (!avatarUrl && thumbs.length > 0) avatarUrl = thumbs[thumbs.length - 1].url;
  avatarImg.src = avatarUrl || 'https://www.youtube.com/s/desktop/9963e639/img/favicon_144x144.png';

  if (bannerUrl) {
    bannerImg.src = bannerUrl;
    bannerWrap.classList.remove('hidden');
  } else {
    bannerWrap.classList.add('hidden');
  }

  renderChannelVideosTable(currentChannelVideos);
}

// Recursively unnest videos from channel tabs/shelves (Videos, Live, Shorts, etc.)
function extractAllVideos(entries) {
  if (!entries || !Array.isArray(entries)) return [];
  const result = [];
  const seenIds = new Set();

  function traverse(list) {
    if (!list || !Array.isArray(list)) return;
    for (const item of list) {
      if (!item) continue;
      // If this item is a container/tab (e.g. "Videos" tab, "Live" tab, or shelf)
      if (item.entries && Array.isArray(item.entries) && item.entries.length > 0) {
        traverse(item.entries);
      } else if (item.id) {
        // Exclude channel root objects or playlist containers
        const isChannelId = item.id.startsWith('UC') && item.id.length >= 24;
        const isPlaylistContainer = item._type === 'playlist' && !item.duration;
        if (!isChannelId && !isPlaylistContainer) {
          if (!seenIds.has(item.id)) {
            seenIds.add(item.id);
            result.push(item);
          }
        }
      }
    }
  }

  traverse(entries);
  return result;
}

function escapeHtml(str) {
  if (!str) return '';
  return String(str)
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function renderChannelVideosTable(videos) {
  const tbody = document.getElementById('channel-videos-tbody');
  tbody.innerHTML = '';

  if (!videos || videos.length === 0) {
    tbody.innerHTML = '<tr><td colspan="7" style="text-align: center; color: var(--text-muted); padding: 20px;">Không tìm thấy video nào.</td></tr>';
    return;
  }

  // Apply sort if active
  let displayVideos = [...videos];
  const thViews = document.getElementById('th-views');
  if (channelSortField === 'views') {
    displayVideos.sort((a, b) => {
      const va = a.view_count || 0;
      const vb = b.view_count || 0;
      return channelSortDir === 'desc' ? vb - va : va - vb;
    });
    if (thViews) {
      thViews.classList.add('active');
      thViews.querySelector('.sort-icon').textContent = channelSortDir === 'desc' ? '▼' : '▲';
    }
  } else {
    if (thViews) {
      thViews.classList.remove('active');
      thViews.querySelector('.sort-icon').textContent = '▲▼';
    }
  }

  displayVideos.forEach((v, idx) => {
    const tr = document.createElement('tr');
    let thumb = '';
    if (v.thumbnails && v.thumbnails.length > 0) {
      thumb = v.thumbnails[v.thumbnails.length - 1].url;
    } else if (v.thumbnail) {
      thumb = v.thumbnail;
    } else if (v.id) {
      thumb = `https://i.ytimg.com/vi/${v.id}/hqdefault.jpg`;
    }
    const dur = v.duration ? formatDuration(v.duration) : '--:--';
    const videoUrl = v.url || `https://www.youtube.com/watch?v=${v.id}`;
    const safeTitle = escapeHtml(v.title || 'Không có tiêu đề');
    const viewsText = (v.view_count != null && v.view_count !== undefined)
      ? formatCompact(v.view_count)
      : '—';

    tr.innerHTML = `
      <td>${idx + 1}</td>
      <td>
        <img src="${thumb}" style="width: 72px; aspect-ratio: 16/9; object-fit: cover; border-radius: 4px;" loading="lazy" alt="thumb" onerror="this.src='https://i.ytimg.com/vi/${v.id}/hqdefault.jpg'" />
      </td>
      <td><strong>${safeTitle}</strong></td>
      <td><span class="code-text">${dur}</span></td>
      <td><span class="view-count-cell">${viewsText}</span></td>
      <td><span class="code-text">${v.id || '--'}</span></td>
      <td>
        <div style="display: flex; gap: 6px;">
          <button class="btn-primary btn-sm" onclick="triggerAnalysis('${videoUrl}')">🔍 Phân tích sâu</button>
          <button class="btn-secondary btn-sm" onclick="openUrl('${videoUrl}')">🔗 Xem</button>
        </div>
      </td>
    `;
    tbody.appendChild(tr);
  });
}

// Toggle sort for channel video list
function toggleViewsSort() {
  if (channelSortField !== 'views') {
    channelSortField = 'views';
    channelSortDir = 'desc';
  } else if (channelSortDir === 'desc') {
    channelSortDir = 'asc';
  } else {
    channelSortField = null;
    channelSortDir = 'desc';
  }
  renderChannelVideosTable(currentChannelVideos);
}

// Download all thumbnails for channel videos
async function downloadAllThumbnails() {
  if (!currentChannelVideos || currentChannelVideos.length === 0) {
    showToast("Không có video nào để tải thumbnail!", true);
    return;
  }

  try {
    // Ask user to select folder
    const folder = await callTauri('select_folder');
    if (!folder) return;

    const channelName = sanitizeFilename(
      currentData?.title || currentData?.channel || 'YouTube_Channel'
    );

    const btn = document.getElementById('channel-dl-thumbnails-btn');
    btn.disabled = true;
    btn.textContent = '⏳ Đang tải...';

    const statusEl = document.getElementById('thumb-dl-status');
    const textEl = document.getElementById('thumb-dl-text');
    const barEl = document.getElementById('thumb-dl-bar');
    statusEl.classList.remove('hidden');
    textEl.textContent = `0/${currentChannelVideos.length}`;
    barEl.style.width = '0%';

    // Prepare video list for Rust backend
    const videoList = currentChannelVideos.map(v => ({
      id: v.id || '',
      title: v.title || 'video'
    }));

    const resultPath = await callTauri('download_all_thumbnails', {
      folder: folder,
      channelName: channelName,
      videos: videoList
    });

    btn.disabled = false;
    btn.textContent = '📥 Tải Thumbnail';
    barEl.style.width = '100%';
    textEl.textContent = `✅ ${currentChannelVideos.length}/${currentChannelVideos.length}`;

    showToast(`Đã tải xong tất cả thumbnail vào: ${resultPath}`);

    // After 3 seconds hide progress and offer to open folder
    setTimeout(() => {
      statusEl.classList.add('hidden');
    }, 4000);

    // Open the folder
    if (resultPath) {
      callTauri('open_file_folder', { path: resultPath + '\\dummy.jpg' }).catch(() => {});
    }
  } catch (err) {
    const btn = document.getElementById('channel-dl-thumbnails-btn');
    btn.disabled = false;
    btn.textContent = '📥 Tải Thumbnail';
    document.getElementById('thumb-dl-status').classList.add('hidden');
    showToast("Lỗi tải thumbnail: " + err, true);
  }
}

// Sanitize filename for safe Windows file paths
function sanitizeFilename(name) {
  if (!name) return 'video';
  return name.replace(/[\\/:*?"<>|]+/g, '_').trim().slice(0, 100);
}

// Update Real-Time Download Progress
function updateDownloadProgress(p) {
  if (!p) return;
  downloadBanner.classList.remove('hidden');

  if (p.is_error) {
    downloadBanner.className = 'banner error';
    downloadBannerTitle.textContent = 'Lỗi khi tải tệp';
    dlStatStatus.textContent = p.status || 'Tải về thất bại!';
    dlBannerSpinIcon.classList.add('hidden');
    openDownloadedFileBtn.classList.add('hidden');
    openDownloadedFolderBtn.classList.add('hidden');
    return;
  }

  if (p.is_done) {
    downloadBanner.className = 'banner success';
    downloadBannerTitle.textContent = '🎉 Tải về hoàn tất thành công!';
    downloadProgressPct.textContent = '100%';
    downloadProgressBar.style.width = '100%';
    dlStatStatus.textContent = 'Hoàn thành';
    dlStatSpeed.textContent = 'Đã xong';
    dlStatEta.textContent = '00:00';
    dlStatTotal.textContent = p.total || '100%';
    dlBannerSpinIcon.classList.add('hidden');

    if (p.file_path) {
      lastDownloadedFilePath = p.file_path;
      openDownloadedFileBtn.classList.remove('hidden');
      openDownloadedFolderBtn.classList.remove('hidden');
    }
    showToast("Tải về hoàn tất!");
    return;
  }

  // In-progress download
  downloadBanner.className = 'banner info';
  dlBannerSpinIcon.classList.remove('hidden');
  openDownloadedFileBtn.classList.add('hidden');
  openDownloadedFolderBtn.classList.add('hidden');

  const pct = Math.max(0, Math.min(100, Math.round(p.percent || 0)));
  downloadProgressPct.textContent = `${pct}%`;
  downloadProgressBar.style.width = `${pct}%`;

  dlStatStatus.textContent = p.status || `Đang tải: ${pct}%`;
  dlStatSpeed.textContent = p.speed && p.speed !== '--' ? `⚡ ${p.speed}` : '--';
  dlStatEta.textContent = p.eta && p.eta !== '--' ? `⏱️ Còn lại: ${p.eta}` : '--';
  dlStatTotal.textContent = p.total && p.total !== '--' ? `📦 ${p.total}` : '--';
}

// Media Download Execution with File Explorer destination selection
async function executeDownload(kind) {
  if (!currentData) {
    showToast("Vui lòng phân tích video trước khi tải về!", true);
    return;
  }

  const rawTitle = currentData.title || currentData.id || 'video';
  const cleanTitle = sanitizeFilename(rawTitle);
  let defaultFilename = `${cleanTitle}.mp4`;
  let filterExt = 'mp4';

  let actualKind = kind;
  if (kind === 'video_1080p') {
    defaultFilename = `${cleanTitle}_1080p.mp4`;
    filterExt = 'mp4';
  } else if (kind === 'video_720p') {
    defaultFilename = `${cleanTitle}_720p.mp4`;
    filterExt = 'mp4';
  } else if (kind === 'video_best') {
    defaultFilename = `${cleanTitle}_best.mp4`;
    filterExt = 'mp4';
  } else if (kind === 'audio_mp3') {
    defaultFilename = `${cleanTitle}.mp3`;
    filterExt = 'mp3';
  } else if (kind === 'thumbnail') {
    defaultFilename = `${cleanTitle}_thumbnail.png`;
    filterExt = 'png';
  } else if (kind === 'subtitles' || kind.startsWith('subtitles')) {
    const origLang = (currentData && currentData.language) ? currentData.language : '';
    const manualSubs = (currentData && currentData.subtitles) ? Object.keys(currentData.subtitles) : [];
    const autoSubs = (currentData && currentData.automatic_captions) ? Object.keys(currentData.automatic_captions) : [];

    let chosenLangs = [];
    // 1. Manual subtitles priority
    if (manualSubs.length > 0) {
      if (manualSubs.includes('vi')) chosenLangs.push('vi');
      if (origLang && manualSubs.includes(origLang)) chosenLangs.push(origLang);
      if (manualSubs.includes('en')) chosenLangs.push('en');
      chosenLangs.push(...manualSubs.slice(0, 3));
    }
    // 2. Automatic subtitles priority
    if (autoSubs.length > 0) {
      if (autoSubs.includes('vi')) chosenLangs.push('vi');
      if (origLang && autoSubs.includes(origLang)) chosenLangs.push(origLang);
      if (autoSubs.includes('en')) chosenLangs.push('en');
      if (autoSubs.includes('en-orig')) chosenLangs.push('en-orig');
    }
    // 3. Fallbacks
    chosenLangs.push('en', 'vi');

    const uniqueLangs = [...new Set(chosenLangs)].filter(Boolean).slice(0, 4).join(',');
    actualKind = `subtitles:${uniqueLangs}`;
    defaultFilename = `${cleanTitle}_subtitles.srt`;
    filterExt = 'srt';
  } else if (kind.startsWith('format_')) {
    const fId = kind.replace('format_', '');
    const fmt = (currentFormats || []).find(f => f.format_id === fId);
    const ext = fmt && fmt.ext ? fmt.ext : 'mp4';
    const note = fmt && (fmt.height ? `${fmt.height}p` : (fmt.resolution || fId));
    defaultFilename = `${cleanTitle}_${fId}_${note}.${ext}`;
    filterExt = ext;
  }

  // 1. Prompt native Windows Save File Dialog (File Explorer)
  let selectedSavePath = null;
  try {
    selectedSavePath = await callTauri('select_save_path', {
      defaultFilename: defaultFilename,
      filterExt: filterExt
    });
  } catch (dialogErr) {
    console.warn("Lỗi mở File Explorer:", dialogErr);
  }

  // If user canceled the File Explorer dialog, abort quietly
  if (!selectedSavePath) {
    return;
  }

  lastDownloadedFilePath = selectedSavePath;

  // 2. Initialize progress UI banner
  downloadBanner.className = 'banner info';
  downloadBanner.classList.remove('hidden');
  const baseName = selectedSavePath.split('\\').pop();
  downloadBannerTitle.textContent = `Đang tải: ${baseName}`;
  downloadProgressPct.textContent = '0%';
  downloadProgressBar.style.width = '0%';
  dlStatStatus.textContent = 'Khởi tạo luồng tải qua yt-dlp...';
  dlStatSpeed.textContent = '--';
  dlStatEta.textContent = '--';
  dlStatTotal.textContent = '--';
  dlBannerSpinIcon.classList.remove('hidden');
  openDownloadedFileBtn.classList.add('hidden');
  openDownloadedFolderBtn.classList.add('hidden');

  const url = currentData.webpage_url || `https://www.youtube.com/watch?v=${currentData.id}`;
  const cookiesBrowser = cookieSelect.value !== 'none' ? cookieSelect.value : null;

  try {
    const resultMsg = await callTauri('download_media', {
      url: url,
      kind: actualKind,
      outputPath: selectedSavePath,
      cookiesBrowser: cookiesBrowser
    });

    downloadBanner.className = 'banner success';
    downloadBannerTitle.textContent = '🎉 Tải về hoàn tất thành công!';
    downloadProgressPct.textContent = '100%';
    downloadProgressBar.style.width = '100%';
    dlStatStatus.textContent = 'Đã lưu thành công!';
    dlBannerSpinIcon.classList.add('hidden');
    openDownloadedFileBtn.classList.remove('hidden');
    openDownloadedFolderBtn.classList.remove('hidden');
    showToast("Tải về thành công!");
  } catch (err) {
    downloadBanner.className = 'banner error';
    downloadBannerTitle.textContent = "Lỗi khi tải tệp";
    dlStatStatus.textContent = err.toString();
    dlBannerSpinIcon.classList.add('hidden');
    showToast("Tải thất bại: " + err, true);
  }
}

// SEO Tag Character Counter
function updateTagCounter(tags) {
  const countEl = document.getElementById('tag-char-count');
  const badgeEl = document.getElementById('tag-limit-badge');
  if (!countEl || !badgeEl) return;

  const totalLen = tags.join(', ').length;
  countEl.textContent = totalLen;

  if (totalLen <= 500) {
    badgeEl.className = 'pill pill-accent';
    badgeEl.textContent = 'Hợp lệ (<= 500)';
  } else {
    badgeEl.className = 'pill pill-warning';
    badgeEl.textContent = `Quá giới hạn (+${totalLen - 500} ký tự)`;
  }
}

// Heatmap SVG Visualization
function renderHeatmap(heatmap) {
  const container = document.getElementById('heatmap-canvas-wrap');
  const emptyMsg = document.getElementById('heatmap-empty-msg');
  container.innerHTML = '';

  if (!heatmap || heatmap.length === 0) {
    emptyMsg.classList.remove('hidden');
    return;
  }
  emptyMsg.classList.add('hidden');

  const width = container.clientWidth || 800;
  const height = 180;
  const maxVal = Math.max(...heatmap.map(h => h.value || 0)) || 1;

  const points = heatmap.map((h, i) => {
    const x = (i / (heatmap.length - 1)) * width;
    const y = height - ((h.value || 0) / maxVal) * (height - 30) - 15;
    return `${x},${y}`;
  });

  const pathD = `M 0,${height} L ${points.join(' L ')} L ${width},${height} Z`;
  const lineD = `M ${points.join(' L ')}`;

  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", `0 0 ${width} ${height}`);
  svg.setAttribute("class", "heatmap-svg");
  svg.innerHTML = `
    <defs>
      <linearGradient id="heatGrad" x1="0%" y1="0%" x2="0%" y2="100%">
        <stop offset="0%" stop-color="#6366f1" stop-opacity="0.65"/>
        <stop offset="100%" stop-color="#6366f1" stop-opacity="0.0"/>
      </linearGradient>
    </defs>
    <path d="${pathD}" fill="url(#heatGrad)" />
    <path d="${lineD}" fill="none" stroke="#818cf8" stroke-width="2.5" />
  `;

  container.appendChild(svg);
}

// Compare 2 Videos
function openCompareModal() {
  if (currentData && currentData.webpage_url) {
    compareUrl1.value = currentData.webpage_url;
  }
  compareModal.classList.remove('hidden');
}

function closeCompareModal() {
  compareModal.classList.add('hidden');
}

async function executeComparison() {
  const u1 = compareUrl1.value.trim();
  const u2 = compareUrl2.value.trim();
  if (!u1 || !u2) {
    showToast("Vui lòng nhập đầy đủ 2 URL video để so sánh!", true);
    return;
  }

  compareLoading.classList.remove('hidden');
  compareResults.classList.add('hidden');

  try {
    const [d1, d2] = await Promise.all([
      callTauri('analyze_url', { url: u1, cookiesBrowser: null }),
      callTauri('analyze_url', { url: u2, cookiesBrowser: null })
    ]);

    document.getElementById('comp-th-1').textContent = d1.title ? d1.title.slice(0, 30) + '...' : 'Video 1';
    document.getElementById('comp-th-2').textContent = d2.title ? d2.title.slice(0, 30) + '...' : 'Video 2';

    const v1 = d1.view_count || 0;
    const v2 = d2.view_count || 0;
    const l1 = d1.like_count || 0;
    const l2 = d2.like_count || 0;
    const c1 = d1.comment_count || 0;
    const c2 = d2.comment_count || 0;
    const r1 = v1 > 0 ? ((l1 / v1) * 100).toFixed(2) + '%' : '0%';
    const r2 = v2 > 0 ? ((l2 / v2) * 100).toFixed(2) + '%' : '0%';

    const tags1 = new Set(d1.tags || []);
    const tags2 = new Set(d2.tags || []);
    const sharedTags = [...tags1].filter(t => tags2.has(t));

    compareTbody.innerHTML = `
      <tr>
        <td><strong>Kênh phát</strong></td>
        <td>${d1.channel || d1.uploader || '--'}</td>
        <td>${d2.channel || d2.uploader || '--'}</td>
      </tr>
      <tr>
        <td><strong>Lượt xem (Views)</strong></td>
        <td><strong style="color: ${v1 >= v2 ? '#34d399' : '#f87171'}">${formatNumber(v1)}</strong></td>
        <td><strong style="color: ${v2 >= v1 ? '#34d399' : '#f87171'}">${formatNumber(v2)}</strong></td>
      </tr>
      <tr>
        <td><strong>Lượt thích (Likes)</strong></td>
        <td>${formatNumber(l1)}</td>
        <td>${formatNumber(l2)}</td>
      </tr>
      <tr>
        <td><strong>Bình luận (Comments)</strong></td>
        <td>${formatNumber(c1)}</td>
        <td>${formatNumber(c2)}</td>
      </tr>
      <tr>
        <td><strong>Tỷ lệ Like/View</strong></td>
        <td>${r1}</td>
        <td>${r2}</td>
      </tr>
      <tr>
        <td><strong>Thời lượng</strong></td>
        <td>${d1.duration_string || formatDuration(d1.duration)}</td>
        <td>${d2.duration_string || formatDuration(d2.duration)}</td>
      </tr>
      <tr>
        <td><strong>Số lượng thẻ Tags</strong></td>
        <td>${(d1.tags || []).length} tags</td>
        <td>${(d2.tags || []).length} tags</td>
      </tr>
      <tr>
        <td><strong>Tags dùng chung</strong></td>
        <td colspan="2">${sharedTags.length > 0 ? sharedTags.map(t => `<span class="tag-badge">#${t}</span>`).join(' ') : 'Không có tag nào trùng lặp'}</td>
      </tr>
    `;

    compareLoading.classList.add('hidden');
    compareResults.classList.remove('hidden');
  } catch (err) {
    compareLoading.classList.add('hidden');
    showToast("Lỗi khi so sánh: " + err, true);
  }
}

// Export Formats CSV
function exportFormatsCsv() {
  if (!currentFormats || currentFormats.length === 0) {
    showToast("Không có dữ liệu luồng để xuất CSV", true);
    return;
  }

  let csv = "Format ID,Loại,Định dạng,Độ phân giải,FPS,Video Codec,Audio Codec,Bitrate (kbps),Dung lượng,Stream URL\n";
  currentFormats.forEach(f => {
    const res = (f.width && f.height) ? `${f.width}x${f.height}` : (f.resolution || '');
    const size = f.filesize ? formatBytes(f.filesize) : (f.filesize_approx ? `~${formatBytes(f.filesize_approx)}` : '');
    const bitrate = f.tbr || f.vbr || f.abr || '';
    csv += `"${f.format_id || ''}","${f.ext || ''}","${res}","${f.fps || ''}","${f.vcodec || ''}","${f.acodec || ''}","${bitrate}","${size}","${f.url || ''}"\n`;
  });

  downloadFile(`${currentData ? currentData.id : 'youtube'}_formats.csv`, csv, 'text/csv;charset=utf-8;');
  showToast("Đã xuất tệp CSV danh sách Formats!");
}

// Export Channel Videos CSV
function exportChannelVideosCsv() {
  if (!currentChannelVideos || currentChannelVideos.length === 0) {
    showToast("Không có video để xuất CSV", true);
    return;
  }

  let csv = "STT,Video ID,Tiêu đề,Thời lượng,Lượt xem,Đường dẫn URL\n";
  currentChannelVideos.forEach((v, idx) => {
    const dur = v.duration ? formatDuration(v.duration) : '';
    const views = v.view_count != null ? v.view_count : '';
    const u = v.url || `https://www.youtube.com/watch?v=${v.id}`;
    csv += `${idx + 1},"${v.id || ''}","${(v.title || '').replace(/"/g, '""')}","${dur}","${views}","${u}"\n`;
  });

  downloadFile(`${currentData ? currentData.id : 'channel'}_videos.csv`, csv, 'text/csv;charset=utf-8;');
  showToast("Đã xuất tệp CSV danh sách Video kênh!");
}

// Helpers for thumbnails
function getBestThumbnail(thumbs) {
  if (!thumbs || thumbs.length === 0) return null;
  return thumbs[thumbs.length - 1].url;
}

function renderThumbnailGallery(thumbs) {
  const container = document.getElementById('thumbnails-gallery');
  container.innerHTML = '';

  if (!thumbs || thumbs.length === 0) {
    container.innerHTML = '<p style="color: var(--text-muted); font-size: 0.85rem;">Không có danh sách thumbnail phụ.</p>';
    return;
  }

  const uniqueThumbs = [];
  const seen = new Set();
  for (let i = thumbs.length - 1; i >= 0; i--) {
    const t = thumbs[i];
    const key = `${t.width}x${t.height}`;
    if (!seen.has(key) && t.url) {
      seen.add(key);
      uniqueThumbs.unshift(t);
    }
  }

  uniqueThumbs.forEach(t => {
    const card = document.createElement('div');
    card.className = 'thumb-card';
    card.innerHTML = `
      <img src="${t.url}" loading="lazy" alt="Thumbnail" />
      <div class="thumb-info">
        <span>${t.id || 'image'}</span>
        <strong>${t.width ? `${t.width}x${t.height}` : 'Auto'}</strong>
      </div>
    `;
    card.onclick = () => openUrl(t.url);
    container.appendChild(card);
  });
}

function renderCategories(cats) {
  const container = document.getElementById('categories-container');
  container.innerHTML = '';
  if (cats.length === 0) {
    container.innerHTML = '<span style="color: var(--text-muted); font-size: 0.85rem;">Không có thông tin thể loại.</span>';
    return;
  }
  cats.forEach(c => {
    const badge = document.createElement('span');
    badge.className = 'category-badge';
    badge.textContent = c;
    container.appendChild(badge);
  });
}

function renderTags(tags) {
  const container = document.getElementById('tags-container');
  container.innerHTML = '';
  if (!tags || tags.length === 0) {
    container.innerHTML = '<span style="color: var(--text-muted); font-size: 0.85rem;">Video này không gắn thẻ từ khóa (tags).</span>';
    return;
  }
  tags.forEach(tag => {
    const badge = document.createElement('span');
    badge.className = 'tag-badge';
    badge.innerHTML = `<span>#${tag}</span> <span class="copy-icon">📋</span>`;
    badge.onclick = () => copyToClipboard(tag, `Đã sao chép tag: #${tag}`);
    container.appendChild(badge);
  });
}

function copyAllTags() {
  if (currentData && currentData.tags && currentData.tags.length > 0) {
    const text = currentData.tags.join(', ');
    copyToClipboard(text, "Đã sao chép toàn bộ tags (chuẩn dán vào YouTube Studio)!");
  } else {
    showToast("Không có tags để sao chép", true);
  }
}

// Formats Table
function renderFormatsTable(formats, filter = 'all') {
  const tbody = document.getElementById('formats-tbody');
  tbody.innerHTML = '';

  if (!formats || formats.length === 0) {
    tbody.innerHTML = '<tr><td colspan="9" style="text-align: center; color: var(--text-muted);">Không tìm thấy luồng dữ liệu nào.</td></tr>';
    return;
  }

  let filtered = formats.filter(f => {
    const hasVideo = f.vcodec && f.vcodec !== 'none';
    const hasAudio = f.acodec && f.acodec !== 'none';

    if (filter === 'video-audio') return hasVideo && hasAudio;
    if (filter === 'video-only') return hasVideo && !hasAudio;
    if (filter === 'audio-only') return !hasVideo && hasAudio;
    return true;
  });

  filtered.forEach(f => {
    const tr = document.createElement('tr');
    const hasVideo = f.vcodec && f.vcodec !== 'none';
    const hasAudio = f.acodec && f.acodec !== 'none';

    let typeBadge = '';
    if (hasVideo && hasAudio) {
      typeBadge = '<span class="format-pill fp-video-audio">Video + Audio</span>';
    } else if (hasVideo) {
      typeBadge = '<span class="format-pill fp-video-only">Chỉ Video</span>';
    } else if (hasAudio) {
      typeBadge = '<span class="format-pill fp-audio-only">Chỉ Audio</span>';
    } else {
      typeBadge = '<span class="format-pill">Khác</span>';
    }

    const res = (f.width && f.height) ? `${f.width}x${f.height}` : (f.resolution || '--');
    const fps = f.fps ? `${f.fps} fps` : '--';
    const codecs = [f.vcodec !== 'none' ? f.vcodec : null, f.acodec !== 'none' ? f.acodec : null].filter(Boolean).join(' / ') || '--';
    const bitrate = f.tbr ? `${Math.round(f.tbr)} kbps` : (f.vbr ? `${Math.round(f.vbr)} kbps` : (f.abr ? `${Math.round(f.abr)} kbps` : '--'));
    const size = f.filesize ? formatBytes(f.filesize) : (f.filesize_approx ? `~${formatBytes(f.filesize_approx)}` : '--');

    tr.innerHTML = `
      <td><span class="code-text">${f.format_id || '--'}</span></td>
      <td>${typeBadge}</td>
      <td><strong>${(f.ext || '').toUpperCase()}</strong></td>
      <td>${res}</td>
      <td>${fps}</td>
      <td title="${codecs}">${codecs.length > 18 ? codecs.slice(0, 18) + '...' : codecs}</td>
      <td>${bitrate}</td>
      <td>${size}</td>
      <td>
        <div style="display: flex; gap: 6px; align-items: center;">
          <button class="btn-primary btn-sm" onclick="downloadFormat('${f.format_id}')" title="Chọn thư mục lưu và tải format này bằng File Explorer">
            ⬇️ Tải về
          </button>
          <button class="btn-secondary btn-sm" onclick="copyStreamUrl('${f.url || ''}')" title="Sao chép Stream URL">
            🔗 Copy
          </button>
        </div>
      </td>
    `;
    tbody.appendChild(tr);
  });
}

window.downloadFormat = function(formatId) {
  executeDownload('format_' + formatId);
};

window.copyStreamUrl = function(url) {
  if (url) {
    copyToClipboard(url, "Đã sao chép đường dẫn Stream trực tiếp!");
  } else {
    showToast("Không tìm thấy URL trực tiếp cho định dạng này", true);
  }
};

function renderChapters(chapters) {
  const container = document.getElementById('chapters-list');
  container.innerHTML = '';

  if (!chapters || chapters.length === 0) {
    container.innerHTML = '<div style="color: var(--text-muted); font-size: 0.85rem; padding: 12px;">Video này không có mốc chia chương (chapters).</div>';
    return;
  }

  chapters.forEach((ch, idx) => {
    const item = document.createElement('div');
    item.className = 'chapter-item';
    const startStr = formatDuration(ch.start_time || 0);
    const endStr = ch.end_time ? formatDuration(ch.end_time) : '';
    const durSec = ch.end_time ? Math.round(ch.end_time - ch.start_time) : 0;

    item.innerHTML = `
      <span class="ch-time">${startStr}</span>
      <span class="ch-title">${ch.title || `Chương ${idx + 1}`}</span>
      <span class="ch-dur">${durSec > 0 ? formatDuration(durSec) : ''}</span>
      <button class="btn-secondary btn-sm" onclick="copyToClipboard('${startStr} ${ch.title}', 'Đã sao chép mốc chương!')">📋</button>
    `;
    container.appendChild(item);
  });
}

function copyChapters() {
  if (currentData && currentData.chapters && currentData.chapters.length > 0) {
    const lines = currentData.chapters.map(ch => `${formatDuration(ch.start_time || 0)} ${ch.title || ''}`);
    copyToClipboard(lines.join('\n'), "Đã sao chép toàn bộ danh sách chương!");
  } else {
    showToast("Video không có chương để sao chép", true);
  }
}

function copyBestThumbnail() {
  if (currentData) {
    const url = getBestThumbnail(currentData.thumbnails) || currentData.thumbnail;
    if (url) copyToClipboard(url, "Đã sao chép link ảnh thumbnail tốt nhất!");
  }
}

function copySummary() {
  if (!currentData) return;
  const d = currentData;
  const summary = `TITLE: ${d.title || ''} | CHANNEL: ${d.channel || ''} | VIDEO ID: ${d.id || ''} | UPLOAD DATE UTC: ${d.upload_date || ''} | TIMESTAMP: ${d.timestamp || ''} | RELEASE DATE UTC: ${d.release_date || ''} | RELEASE TIMESTAMP: ${d.release_timestamp || ''} | LIVE STATUS: ${d.live_status || ''} | WAS LIVE: ${d.was_live || ''} | DURATION: ${d.duration_string || ''} | VIEWS: ${d.view_count || ''} | LIKES: ${d.like_count || ''} | COMMENTS: ${d.comment_count || ''} | CATEGORY: ${(d.categories || []).join(', ')} | TAGS: ${(d.tags || []).join(', ')} | URL: ${d.webpage_url || ''}`;
  copyToClipboard(summary, "Đã sao chép chuỗi tóm tắt Metadata!");
}

function exportFullJson() {
  if (!currentData) return;
  const jsonStr = JSON.stringify(currentData, null, 2);
  const filename = `${currentData.id || 'youtube_metadata'}.json`;
  downloadFile(filename, jsonStr, 'application/json');
  showToast(`Đã xuất tệp ${filename}!`);
}

function downloadFile(filename, content, mimeType) {
  const blob = new Blob([content], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

// History Management
function saveToHistory(data, type = 'video', inputUrl = '') {
  try {
    const history = JSON.parse(localStorage.getItem('yt_analyzer_history') || '[]');
    let item;

    if (type === 'channel' || data._type === 'playlist' || data.entries) {
      // Find channel avatar
      let avatarUrl = '';
      const thumbs = data.thumbnails || [];
      thumbs.forEach(t => {
        if (t.id === 'avatar_uncropped' || (t.width && t.width === t.height && t.width > 80)) {
          avatarUrl = t.url;
        }
      });
      if (!avatarUrl && thumbs.length > 0) avatarUrl = thumbs[thumbs.length - 1].url;
      if (!avatarUrl) avatarUrl = 'https://www.youtube.com/s/desktop/9963e639/img/favicon_144x144.png';

      const channelSubs = data.channel_follower_count 
        ? `${formatNumber(data.channel_follower_count)} người đăng ký` 
        : (data.uploader_id || 'Kênh YouTube');

      const channelUrl = inputUrl || data.channel_url || data.webpage_url || data.original_url || '';

      item = {
        id: data.id || data.uploader_id || channelUrl,
        title: data.title || data.channel || 'Kênh YouTube',
        channel: channelSubs,
        thumbnail: avatarUrl,
        url: channelUrl,
        type: 'channel',
        date: new Date().toLocaleString('vi-VN')
      };
    } else {
      const videoUrl = data.webpage_url || inputUrl || `https://www.youtube.com/watch?v=${data.id}`;
      item = {
        id: data.id,
        title: data.title || 'Video YouTube',
        channel: data.channel || data.uploader || 'Không rõ kênh',
        thumbnail: getBestThumbnail(data.thumbnails) || data.thumbnail || `https://i.ytimg.com/vi/${data.id}/hqdefault.jpg`,
        url: videoUrl,
        type: 'video',
        date: new Date().toLocaleString('vi-VN')
      };
    }

    // Filter out duplicates by id or url
    const filtered = history.filter(h => (h.id !== item.id && h.url !== item.url));
    filtered.unshift(item);
    if (filtered.length > 30) filtered.pop();

    localStorage.setItem('yt_analyzer_history', JSON.stringify(filtered));
    updateHistoryBadge();
  } catch (e) {
    console.error("Lỗi lưu lịch sử:", e);
  }
}

function updateHistoryBadge() {
  const history = JSON.parse(localStorage.getItem('yt_analyzer_history') || '[]');
  historyCountBadge.textContent = history.length;
}

function openHistoryModal() {
  const history = JSON.parse(localStorage.getItem('yt_analyzer_history') || '[]');
  historyList.innerHTML = '';

  if (history.length === 0) {
    historyList.innerHTML = '<p style="text-align: center; color: var(--text-muted); padding: 20px;">Chưa có lịch sử phân tích nào.</p>';
  } else {
    history.forEach(item => {
      const isChannel = item.type === 'channel';
      const badgeHtml = isChannel
        ? `<span class="hist-badge badge-channel">Kênh</span>`
        : `<span class="hist-badge badge-video">Video</span>`;
      const thumbClass = isChannel ? 'hist-thumb is-channel' : 'hist-thumb';
      const safeTitle = escapeHtml(item.title || '');
      const safeChannel = escapeHtml(item.channel || '');

      const row = document.createElement('div');
      row.className = 'history-item';
      row.innerHTML = `
        <img class="${thumbClass}" src="${item.thumbnail || ''}" alt="thumb" onerror="this.src='https://www.youtube.com/s/desktop/9963e639/img/favicon_144x144.png'" />
        <div class="hist-info">
          <div class="hist-title">${badgeHtml} ${safeTitle}</div>
          <div class="hist-channel">${safeChannel} • ${item.date}</div>
        </div>
      `;
      row.onclick = () => {
        urlInput.value = item.url;
        clearBtn.classList.remove('hidden');
        closeHistoryModal();
        triggerAnalysis();
      };
      historyList.appendChild(row);
    });
  }

  historyModal.classList.remove('hidden');
}

function closeHistoryModal() {
  historyModal.classList.add('hidden');
}

function clearHistory() {
  localStorage.removeItem('yt_analyzer_history');
  updateHistoryBadge();
  openHistoryModal();
  showToast("Đã xóa toàn bộ lịch sử!");
}

// Helpers
function openUrl(url) {
  try {
    callTauri('open_in_browser', { url: url });
  } catch (e) {
    window.open(url, '_blank');
  }
}

function setLoading(isLoading, customTitle = null) {
  if (isLoading) {
    spinIcon.classList.remove('hidden');
    actionIcon.classList.add('hidden');
    analyzeBtnText.textContent = 'Đang phân tích...';
    analyzeBtn.disabled = true;
    loadingTitle.textContent = customTitle || "Đang kết nối và bóc tách dữ liệu yt-dlp...";
    loadingState.classList.remove('hidden');
    emptyState.classList.add('hidden');
    resultContainer.classList.add('hidden');
    channelContainer.classList.add('hidden');
  } else {
    spinIcon.classList.add('hidden');
    actionIcon.classList.remove('hidden');
    analyzeBtnText.textContent = 'Phân tích';
    analyzeBtn.disabled = false;
    loadingState.classList.add('hidden');
  }
}

function showError(msg) {
  errorMessage.textContent = msg;
  errorBanner.classList.remove('hidden');
  emptyState.classList.remove('hidden');
}

function hideError() {
  errorBanner.classList.add('hidden');
}

function showToast(msg, isError = false) {
  toastMessage.textContent = msg;
  toast.style.background = isError ? 'rgba(239, 68, 68, 0.95)' : 'rgba(16, 185, 129, 0.95)';
  toast.classList.remove('hidden');
  setTimeout(() => toast.classList.add('hidden'), 2600);
}

function copyToClipboard(text, successMsg = "Đã sao chép!") {
  navigator.clipboard.writeText(text).then(() => {
    showToast(successMsg);
  }).catch(() => {
    showToast("Không thể sao chép", true);
  });
}

function formatNumber(num) {
  if (num === null || num === undefined) return '--';
  return Number(num).toLocaleString('vi-VN');
}

function formatCompact(num) {
  if (!num) return '0';
  const n = Number(num);
  if (n >= 1e9) return (n / 1e9).toFixed(2) + 'B';
  if (n >= 1e6) return (n / 1e6).toFixed(2) + 'M';
  if (n >= 1e3) return (n / 1e3).toFixed(1) + 'K';
  return n.toString();
}

function formatDuration(sec) {
  if (!sec) return '00:00';
  const s = Math.floor(sec);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const remainingS = s % 60;

  if (h > 0) {
    return `${h}:${m.toString().padStart(2, '0')}:${remainingS.toString().padStart(2, '0')}`;
  }
  return `${m.toString().padStart(2, '0')}:${remainingS.toString().padStart(2, '0')}`;
}

function formatDateStr(str) {
  if (!str) return '--';
  if (str.length === 8) {
    const y = str.slice(0, 4);
    const m = str.slice(4, 6);
    const d = str.slice(6, 8);
    return `${d}/${m}/${y}`;
  }
  return str;
}

function formatTimestamp(ts) {
  if (!ts) return '--';
  const date = new Date(ts * 1000);
  return date.toLocaleString('vi-VN');
}

function formatBytes(bytes) {
  if (!bytes || bytes === 0) return '0 B';
  const k = 1024;
  const sizes = ['B', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
}
