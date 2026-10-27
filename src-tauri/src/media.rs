//! What is playing in Windows right now, through the Global System Media
//! Transport Controls (GSMTC): every app that shows up in the volume flyout
//! (Spotify, browsers, Yandex Music, Media Player…). Read-only apart from
//! play/pause/next/previous; nothing is stored.
//!
//! The `widget_media` module owns a background task that polls the current
//! session once a second and emits `MEDIA_EVENT` when anything changes.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, AppResult};
use crate::modules::catalog::WIDGET_MEDIA;
use crate::permissions::{self, PERM_MEDIA};
use crate::state::AppState;

/// Emitted with a `NowPlaying` payload when the track or playback state changes.
pub const MEDIA_EVENT: &str = "focusdeck://media-changed";

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct NowPlaying {
    /// A media session exists (something is or was playing).
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// App user model id of the source, e.g. `Spotify.exe`.
    pub source_id: String,
    /// Human-readable source app name.
    pub source: String,
    /// `playing`, `paused`, `stopped`, `changing`, `closed` or `opened`.
    pub status: String,
    /// Cover art as a `data:` URL, if the app provides one.
    pub cover: Option<String>,
    pub can_play: bool,
    pub can_pause: bool,
    pub can_next: bool,
    pub can_previous: bool,
    pub position_ms: Option<i64>,
    pub duration_ms: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Control {
    PlayPause,
    Next,
    Previous,
}

impl Control {
    pub fn parse(s: &str) -> AppResult<Self> {
        match s {
            "play_pause" => Ok(Self::PlayPause),
            "next" => Ok(Self::Next),
            "previous" => Ok(Self::Previous),
            other => Err(AppError::Invalid(format!("неизвестная команда воспроизведения «{other}»"))),
        }
    }
}

/// Friendly name for a GSMTC source id (an exe name or a packaged app id).
pub fn source_name(id: &str) -> String {
    let lower = id.to_ascii_lowercase();
    const KNOWN: &[(&str, &str)] = &[
        ("spotify", "Spotify"),
        ("yandex.music", "Яндекс Музыка"),
        ("yandexmusic", "Яндекс Музыка"),
        ("yandex", "Яндекс Браузер"),
        ("chrome", "Google Chrome"),
        ("msedge", "Microsoft Edge"),
        ("firefox", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("vivaldi", "Vivaldi"),
        ("zunemusic", "Медиаплеер"),
        ("zunevideo", "Кино и ТВ"),
        ("vlc", "VLC"),
        ("aimp", "AIMP"),
        ("foobar2000", "foobar2000"),
        ("itunes", "iTunes"),
        ("applemusic", "Apple Music"),
        ("telegram", "Telegram"),
        ("discord", "Discord"),
        ("vk", "VK"),
    ];
    if let Some((_, name)) = KNOWN.iter().find(|(k, _)| lower.contains(k)) {
        return name.to_string();
    }
    // `Company.App_hash!App` → `App`; `app.exe` → `app`.
    let base = id.rsplit('!').next().unwrap_or(id);
    let base = base.rsplit(['\\', '/']).next().unwrap_or(base);
    let base = base.strip_suffix(".exe").or_else(|| base.strip_suffix(".EXE")).unwrap_or(base);
    let base = base.rsplit('.').next().unwrap_or(base);
    if base.is_empty() { "Неизвестная программа".into() } else { base.to_string() }
}

#[cfg(windows)]
mod imp {
    use std::sync::Mutex;

    use base64::Engine;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::{Buffer, DataReader, InputStreamOptions};
    use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};

    use super::{Control, NowPlaying, source_name};

    /// Max cover size we embed (bigger thumbnails are skipped).
    const MAX_COVER: u32 = 2 * 1024 * 1024;

    /// The last cover, keyed by source + title + artist, so it is read once per track.
    static COVER: Mutex<Option<(String, Option<String>)>> = Mutex::new(None);

    fn session() -> windows::core::Result<Option<Session>> {
        // SAFETY: initializes COM for this blocking thread; S_FALSE / changed mode are fine.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        let manager = Manager::RequestAsync()?.join()?;
        match manager.GetCurrentSession() {
            Ok(s) => Ok(Some(s)),
            // No session: the call fails with a null result.
            Err(_) => Ok(None),
        }
    }

    fn cover(s: &Session, key: &str) -> Option<String> {
        if let Some((k, c)) = COVER.lock().unwrap().as_ref() {
            if k == key {
                return c.clone();
            }
        }
        let read = || -> windows::core::Result<Option<String>> {
            let props = s.TryGetMediaPropertiesAsync()?.join()?;
            let Ok(thumb) = props.Thumbnail() else { return Ok(None) };
            let stream = thumb.OpenReadAsync()?.join()?;
            let size = stream.Size()? as u32;
            if size == 0 || size > MAX_COVER {
                return Ok(None);
            }
            let mime = stream.ContentType().map(|m| m.to_string()).unwrap_or_default();
            let buffer = Buffer::Create(size)?;
            let buffer = stream.ReadAsync(&buffer, size, InputStreamOptions::None)?.join()?;
            let reader = DataReader::FromBuffer(&buffer)?;
            let mut bytes = vec![0u8; buffer.Length()? as usize];
            reader.ReadBytes(&mut bytes)?;
            let mime = if mime.starts_with("image/") { mime } else { "image/png".into() };
            Ok(Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes))))
        };
        let c = read().unwrap_or(None);
        // A track often gets its cover a moment after the title: retry until there is one.
        if c.is_some() {
            *COVER.lock().unwrap() = Some((key.to_string(), c.clone()));
        }
        c
    }

    pub fn now_playing(with_cover: bool) -> Result<NowPlaying, String> {
        let Some(s) = session().map_err(|e| format!("медиа-элементы Windows недоступны: {e}"))? else {
            return Ok(NowPlaying { status: "closed".into(), ..Default::default() });
        };
        let source_id = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        let props = s.TryGetMediaPropertiesAsync().and_then(|op| op.join()).ok();
        let text = |f: &dyn Fn() -> windows::core::Result<windows::core::HSTRING>| f().map(|h| h.to_string()).unwrap_or_default();
        let (title, artist, album) = match &props {
            Some(p) => (
                text(&|| p.Title()),
                text(&|| p.Artist()).trim().to_string(),
                text(&|| p.AlbumTitle()),
            ),
            None => Default::default(),
        };
        let artist = if artist.is_empty() { props.as_ref().map(|p| text(&|| p.AlbumArtist())).unwrap_or_default() } else { artist };
        let info = s.GetPlaybackInfo().ok();
        let status = info
            .as_ref()
            .and_then(|i| i.PlaybackStatus().ok())
            .map(|st| match st {
                Status::Playing => "playing",
                Status::Paused => "paused",
                Status::Stopped => "stopped",
                Status::Changing => "changing",
                Status::Opened => "opened",
                _ => "closed",
            })
            .unwrap_or("closed")
            .to_string();
        let controls = info.as_ref().and_then(|i| i.Controls().ok());
        let can = |f: &dyn Fn(&windows::Media::Control::GlobalSystemMediaTransportControlsSessionPlaybackControls) -> windows::core::Result<bool>| {
            controls.as_ref().and_then(|c| f(c).ok()).unwrap_or(false)
        };
        let timeline = s.GetTimelineProperties().ok();
        // TimeSpan is in 100 ns ticks.
        let ms = |d: Option<windows::Foundation::TimeSpan>| d.map(|t| t.Duration / 10_000);
        let position_ms = ms(timeline.as_ref().and_then(|t| t.Position().ok()));
        let duration_ms = ms(timeline.as_ref().and_then(|t| t.EndTime().ok())).filter(|d| *d > 0);
        let key = format!("{source_id}\u{1}{title}\u{1}{artist}");
        Ok(NowPlaying {
            active: true,
            source: source_name(&source_id),
            cover: if with_cover { cover(&s, &key) } else { None },
            can_play: can(&|c| c.IsPlayEnabled()),
            can_pause: can(&|c| c.IsPauseEnabled()),
            can_next: can(&|c| c.IsNextEnabled()),
            can_previous: can(&|c| c.IsPreviousEnabled()),
            source_id,
            title,
            artist,
            album,
            status,
            position_ms,
            duration_ms,
        })
    }

    pub fn control(c: Control) -> Result<bool, String> {
        let s = session().map_err(|e| e.to_string())?.ok_or_else(|| "сейчас ничего не играет".to_string())?;
        let op = match c {
            Control::PlayPause => s.TryTogglePlayPauseAsync(),
            Control::Next => s.TrySkipNextAsync(),
            Control::Previous => s.TrySkipPreviousAsync(),
        };
        op.and_then(|op| op.join()).map_err(|e| e.to_string())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{Control, NowPlaying};
    pub fn now_playing(_: bool) -> Result<NowPlaying, String> {
        Err("только в Windows".into())
    }
    pub fn control(_: Control) -> Result<bool, String> {
        Err("только в Windows".into())
    }
}

/// Reads the current session off the async runtime.
pub async fn now_playing(with_cover: bool) -> AppResult<NowPlaying> {
    tauri::async_runtime::spawn_blocking(move || imp::now_playing(with_cover))
        .await
        .map_err(anyhow::Error::from)?
        .map_err(|e| AppError::Other(anyhow::anyhow!(e)))
}

/// Play/pause, next or previous in the current session; returns the new state.
pub async fn control(app: &AppHandle, c: Control) -> AppResult<NowPlaying> {
    let db = &app.state::<AppState>().db;
    permissions::require(db, PERM_MEDIA, "widget:media").await?;
    let ok = tauri::async_runtime::spawn_blocking(move || imp::control(c)).await.map_err(anyhow::Error::from)?.map_err(|e| AppError::Other(anyhow::anyhow!(e)))?;
    if !ok {
        return Err(AppError::Invalid("программа-источник не поддерживает эту команду".into()));
    }
    // Give the source app a moment to switch tracks.
    tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    let now = now_playing(true).await?;
    let _ = app.emit(MEDIA_EVENT, &now);
    Ok(now)
}

/// Background task of the `widget_media` module: emits `MEDIA_EVENT` on changes.
pub async fn watch_loop(app: AppHandle) {
    let mut last: Option<NowPlaying> = None;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        interval.tick().await;
        let db = &app.state::<AppState>().db;
        if !permissions::is_granted(db, PERM_MEDIA).await.unwrap_or(false) {
            last = None;
            continue;
        }
        let Ok(now) = now_playing(true).await else { continue };
        // The position moves every second; only real changes are events.
        let same = last.as_ref().is_some_and(|l| {
            let mut a = l.clone();
            a.position_ms = now.position_ms;
            a == now
        });
        if !same {
            let _ = app.emit(MEDIA_EVENT, &now);
        }
        last = Some(now);
    }
}

/// What is playing now (the music widget gets the same data through `widget_data`).
#[tauri::command]
pub async fn media_now_playing(state: State<'_, AppState>) -> AppResult<NowPlaying> {
    crate::productivity::require(&state, WIDGET_MEDIA)?;
    permissions::require(&state.db, PERM_MEDIA, "media_now_playing").await?;
    now_playing(true).await
}

/// `action`: `play_pause`, `next` or `previous`.
#[tauri::command]
pub async fn media_control(app: AppHandle, state: State<'_, AppState>, action: String) -> AppResult<NowPlaying> {
    crate::productivity::require(&state, WIDGET_MEDIA)?;
    control(&app, Control::parse(&action)?).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_names() {
        assert_eq!(source_name("Spotify.exe"), "Spotify");
        assert_eq!(source_name("chrome"), "Google Chrome");
        assert_eq!(source_name("MSEdge"), "Microsoft Edge");
        assert_eq!(source_name("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "Медиаплеер");
        assert_eq!(source_name("Yandex.Music_abc!App"), "Яндекс Музыка");
        assert_eq!(source_name(r"C:\Apps\MyPlayer.exe"), "MyPlayer");
        assert_eq!(source_name(""), "Неизвестная программа");
    }

    #[test]
    fn controls_parse() {
        assert_eq!(Control::parse("next").unwrap(), Control::Next);
        assert!(Control::parse("stop").is_err());
    }
}
