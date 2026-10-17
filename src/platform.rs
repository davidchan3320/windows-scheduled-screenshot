//! Windows boundaries. Every owned GDI/desktop/COM/handle resource has a guard.
use crate::{config::Task, engine::App, output};
use anyhow::{Context, Result, ensure};
use chrono::Local;
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            Com::*, LibraryLoader::GetModuleHandleW, RemoteDesktop::*, StationsAndDesktops::*,
            Threading::*,
        },
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
    core::{Interface, PCWSTR, PWSTR, w},
};

pub fn wide(s: impl AsRef<std::ffi::OsStr>) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.as_ref().encode_wide().chain(Some(0)).collect()
}
pub fn message(text: &str) {
    unsafe {
        MessageBoxW(
            None,
            PCWSTR(wide(text).as_ptr()),
            w!("Scheduled Screenshot"),
            MB_OK | MB_ICONERROR,
        );
    }
}
pub fn open(path: &str) -> Result<()> {
    let value = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(wide(path).as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    ensure!(value.0 as isize > 32, "Windows could not open {path}.");
    Ok(())
}
pub fn open_folder(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .with_context(|| format!("Could not open output folder {}", path.display()))?;
    open(&path.to_string_lossy())
}
struct Com;
impl Com {
    fn init() -> Result<Self> {
        unsafe {
            CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        }
        Ok(Self)
    }
}
impl Drop for Com {
    fn drop(&mut self) {
        unsafe {
            CoUninitialize();
        }
    }
}
struct ComString(PWSTR);
impl Drop for ComString {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(Some(self.0.0.cast()));
        }
    }
}
pub fn choose_folder(language: &str) -> Result<Option<String>> {
    let _com = Com::init()?;
    unsafe {
        let dialog: IFileOpenDialog =
            CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
        dialog.SetOptions(
            dialog.GetOptions()? | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_NOCHANGEDIR,
        )?;
        dialog.SetTitle(PCWSTR(
            wide(crate::localization::text(
                language,
                "Choose screenshot folder",
            ))
            .as_ptr(),
        ))?;
        if let Err(e) = dialog.Show(None) {
            if e.code().0 as u32 == 0x800704c7 {
                return Ok(None);
            }
            return Err(e.into());
        }
        let name = ComString(dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?);
        Ok(Some(name.0.to_string()?))
    }
}
pub fn set_startup(enabled: bool, base: &Path) -> Result<()> {
    let _com = Com::init()?;
    unsafe {
        let folder = ComString(SHGetKnownFolderPath(
            &FOLDERID_Startup,
            KF_FLAG_DEFAULT,
            None,
        )?);
        let path = PathBuf::from(folder.0.to_string()?).join("ScheduledScreenshot.lnk");
        if !enabled {
            if path.exists() {
                std::fs::remove_file(path)?;
            }
            return Ok(());
        }
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(wide(std::env::current_exe()?).as_ptr()))?;
        link.SetArguments(w!("--background"))?;
        link.SetWorkingDirectory(PCWSTR(wide(base).as_ptr()))?;
        link.SetDescription(w!("Scheduled Screenshot — portable screenshot scheduler"))?;
        let persist: IPersistFile = link.cast()?;
        let tmp = path.with_extension(format!("{}.tmp", uuid::Uuid::new_v4()));
        persist.Save(PCWSTR(wide(&tmp).as_ptr()), true)?;
        let result = crate::storage::atomic_write(&path, &std::fs::read(&tmp)?);
        let _ = std::fs::remove_file(tmp);
        result
    }
}

struct Desktop(HDESK);
impl Drop for Desktop {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseDesktop(self.0);
        }
    }
}
pub fn desktop_available() -> bool {
    unsafe {
        let mut buffer = PWSTR::null();
        let mut length = 0;
        if WTSQuerySessionInformationW(
            None,
            WTS_CURRENT_SESSION,
            WTSConnectState,
            &mut buffer,
            &mut length,
        )
        .is_err()
        {
            return false;
        }
        let active = length as usize >= size_of::<WTS_CONNECTSTATE_CLASS>()
            && *(buffer.0 as *const WTS_CONNECTSTATE_CLASS) == WTSActive;
        WTSFreeMemory(buffer.0.cast());
        if !active {
            return false;
        }
        let Ok(desk) = OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_READOBJECTS)
        else {
            return false;
        };
        let desk = Desktop(desk);
        let mut name = [0u16; 256];
        if GetUserObjectInformationW(
            HANDLE(desk.0.0),
            UOI_NAME,
            Some(name.as_mut_ptr().cast()),
            size_of_val(&name) as u32,
            None,
        )
        .is_err()
        {
            return false;
        }
        String::from_utf16_lossy(&name[..name.iter().position(|c| *c == 0).unwrap_or(name.len())])
            .eq_ignore_ascii_case("Default")
    }
}
#[derive(Clone)]
struct Monitor {
    rect: RECT,
    name: String,
}
unsafe extern "system" fn enum_monitor(
    monitor: HMONITOR,
    _: HDC,
    _: *mut RECT,
    param: LPARAM,
) -> windows::core::BOOL {
    unsafe {
        let list = &mut *(param.0 as *mut Vec<Monitor>);
        let mut info = MONITORINFOEXW::default();
        info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
        if GetMonitorInfoW(monitor, &mut info.monitorInfo).as_bool() {
            let len = info
                .szDevice
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(info.szDevice.len());
            list.push(Monitor {
                rect: info.monitorInfo.rcMonitor,
                name: String::from_utf16_lossy(&info.szDevice[..len]),
            });
        } else {
            return false.into();
        }
        true.into()
    }
}
struct Surface {
    screen: HDC,
    memory: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
}
impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            if !self.old.is_invalid() {
                SelectObject(self.memory, self.old);
            }
            if !self.bitmap.is_invalid() {
                let _ = DeleteObject(self.bitmap.into());
            }
            if !self.memory.is_invalid() {
                let _ = DeleteDC(self.memory);
            }
            if !self.screen.is_invalid() {
                ReleaseDC(None, self.screen);
            }
        }
    }
}
struct CursorIcon {
    icon: HICON,
    info: ICONINFO,
}
impl Drop for CursorIcon {
    fn drop(&mut self) {
        unsafe {
            if !self.info.hbmColor.is_invalid() {
                let _ = DeleteObject(self.info.hbmColor.into());
            }
            if !self.info.hbmMask.is_invalid() {
                let _ = DeleteObject(self.info.hbmMask.into());
            }
            let _ = DestroyIcon(self.icon);
        }
    }
}
fn capture_monitor(m: &Monitor, cursor: bool) -> Result<image::RgbImage> {
    let width = m.rect.right - m.rect.left;
    let height = m.rect.bottom - m.rect.top;
    ensure!(
        width > 0 && height > 0 && i64::from(width) * i64::from(height) <= 80_000_000,
        "Invalid or excessively large monitor dimensions."
    );
    unsafe {
        let mut s = Surface {
            screen: GetDC(None),
            memory: HDC::default(),
            bitmap: HBITMAP::default(),
            old: HGDIOBJ::default(),
        };
        ensure!(!s.screen.is_invalid(), "Could not access the desktop.");
        s.memory = CreateCompatibleDC(Some(s.screen));
        ensure!(!s.memory.is_invalid(), "Could not create capture surface.");
        s.bitmap = CreateCompatibleBitmap(s.screen, width, height);
        ensure!(!s.bitmap.is_invalid(), "Could not allocate capture bitmap.");
        s.old = SelectObject(s.memory, s.bitmap.into());
        ensure!(!s.old.is_invalid(), "Could not select capture bitmap.");
        BitBlt(
            s.memory,
            0,
            0,
            width,
            height,
            Some(s.screen),
            m.rect.left,
            m.rect.top,
            SRCCOPY | CAPTUREBLT,
        )?;
        if cursor {
            let mut ci = CURSORINFO {
                cbSize: size_of::<CURSORINFO>() as u32,
                ..Default::default()
            };
            GetCursorInfo(&mut ci)?;
            if ci.flags == CURSOR_SHOWING {
                let icon = CopyIcon(HICON(ci.hCursor.0))?;
                let mut guard = CursorIcon {
                    icon,
                    info: ICONINFO::default(),
                };
                GetIconInfo(icon, &mut guard.info)?;
                DrawIconEx(
                    s.memory,
                    ci.ptScreenPos.x - m.rect.left - guard.info.xHotspot as i32,
                    ci.ptScreenPos.y - m.rect.top - guard.info.yHotspot as i32,
                    icon,
                    0,
                    0,
                    0,
                    None,
                    DI_NORMAL,
                )?;
            }
        }
        SelectObject(s.memory, s.old);
        s.old = HGDIOBJ::default();
        let mut bitmap_info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bytes = vec![0u8; width as usize * height as usize * 4];
        ensure!(
            GetDIBits(
                s.screen,
                s.bitmap,
                0,
                height as u32,
                Some(bytes.as_mut_ptr().cast()),
                &mut bitmap_info,
                DIB_RGB_COLORS
            ) == height,
            "Could not read the captured monitor."
        );
        // Reuse the same buffer, retaining one monitor in memory at a time.
        for n in 0..width as usize * height as usize {
            let b = bytes[n * 4];
            let g = bytes[n * 4 + 1];
            let r = bytes[n * 4 + 2];
            bytes[n * 3] = r;
            bytes[n * 3 + 1] = g;
            bytes[n * 3 + 2] = b;
        }
        bytes.truncate(width as usize * height as usize * 3);
        image::RgbImage::from_raw(width as u32, height as u32, bytes)
            .context("Invalid captured image")
    }
}
pub fn capture(
    base: &Path,
    task: &Task,
    available: &AtomicBool,
    epoch: &std::sync::atomic::AtomicU64,
    expected_epoch: u64,
) -> Result<Vec<String>> {
    let allowed = || {
        available.load(Ordering::SeqCst)
            && epoch.load(Ordering::SeqCst) == expected_epoch
            && desktop_available()
    };
    let mut files: Vec<PathBuf> = vec![];
    let result = (|| -> Result<()> {
        ensure!(
            allowed(),
            "The interactive desktop is locked, secure, or disconnected."
        );
        let mut monitors = Vec::<Monitor>::new();
        unsafe {
            EnumDisplayMonitors(
                None,
                None,
                Some(enum_monitor),
                LPARAM(&mut monitors as *mut _ as isize),
            )
            .ok()?;
        }
        monitors.sort_by_key(|m| (m.rect.left, m.rect.top));
        ensure!(
            !monitors.is_empty() && monitors.len() <= 32,
            "No supported monitors are available."
        );
        let at = Local::now();
        for (index, monitor) in monitors.iter().enumerate() {
            ensure!(allowed(), "Desktop became unavailable.");
            let pixels = capture_monitor(monitor, task.capture.include_cursor)?;
            ensure!(
                allowed(),
                "Desktop became unavailable during capture; images discarded."
            );
            let path = output::path_for(base, task, at, &monitor.name, index + 1)?;
            files.push(output::save_image(&path, &pixels, task)?);
        }
        ensure!(
            allowed(),
            "Desktop or display configuration changed during capture; images discarded."
        );
        Ok(())
    })();
    if let Err(e) = result {
        for file in files {
            let _ = std::fs::remove_file(file);
        }
        return Err(e);
    }
    Ok(files
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

pub struct Instance {
    handle: HANDLE,
}
impl Drop for Instance {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}
// All portable copies share one scheduler in the interactive Windows session.
const INSTANCE_CLASS: PCWSTR = w!("ScheduledScreenshot.Instance");
pub fn single_instance() -> Result<Option<Instance>> {
    let handle = unsafe { CreateMutexW(None, false, w!("Local\\ScheduledScreenshot.Instance"))? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            if let Ok(hwnd) = FindWindowW(INSTANCE_CLASS, None) {
                let _ = PostMessageW(Some(hwnd), WM_APP + 2, WPARAM(0), LPARAM(0));
            }
            let _ = CloseHandle(handle);
        }
        return Ok(None);
    }
    Ok(Some(Instance { handle }))
}
const CAPTURE_HOTKEY_ID: i32 = 1;
pub fn restart_window() -> Result<HWND> {
    unsafe {
        let hwnd = FindWindowW(INSTANCE_CLASS, None)?;
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        ensure!(
            pid == std::process::id(),
            "Application restart is unavailable."
        );
        Ok(hwnd)
    }
}
pub fn request_restart() -> Result<()> {
    unsafe {
        PostMessageW(Some(restart_window()?), WM_APP + 4, WPARAM(0), LPARAM(0))?;
    }
    Ok(())
}
// Hotkeys belong to the window thread. Settings saves only post a refresh request.
pub fn refresh_capture_shortcut() {
    unsafe {
        if let Ok(hwnd) = FindWindowW(INSTANCE_CLASS, None) {
            let _ = PostMessageW(Some(hwnd), WM_APP + 3, WPARAM(0), LPARAM(0));
        }
    }
}
struct CaptureHotkey(HWND);
impl Drop for CaptureHotkey {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.0), CAPTURE_HOTKEY_ID);
        }
    }
}
fn apply_capture_shortcut(hwnd: HWND, app: &App) {
    let mut state = app.state.lock().unwrap();
    unsafe {
        let _ = UnregisterHotKey(Some(hwnd), CAPTURE_HOTKEY_ID);
    }
    state.shortcut_active = None;
    state.shortcut_error = None;
    let shortcut = &state.settings.capture_shortcut;
    if !shortcut.enabled {
        return;
    }
    let result = (|| -> Result<()> {
        unsafe {
            RegisterHotKey(
                Some(hwnd),
                CAPTURE_HOTKEY_ID,
                HOT_KEY_MODIFIERS(shortcut.modifiers_bits()?) | MOD_NOREPEAT,
                shortcut.virtual_key()?,
            )?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => state.shortcut_active = Some(shortcut.label()),
        Err(error) => {
            app.store.log(&format!(
                "Could not register capture shortcut {}: {error:#}",
                shortcut.label()
            ));
            state.shortcut_error = Some("Shortcut unavailable. It may be in use by another app. Choose another combination and save.".into());
        }
    }
}
struct Tray {
    app: App,
    url: String,
    icon: HICON,
    explorer_message: u32,
    restart: Cell<bool>,
}
thread_local! { static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) }; }
fn tray_icon() -> Result<HICON> {
    // Embedded camera glyph: BGRA 32x32, no external icon assets.
    let mut pixels = vec![0u8; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let body = (4..28).contains(&x) && (9..25).contains(&y)
                || (10..19).contains(&x) && (6..10).contains(&y);
            let lens = (x as i32 - 16).pow(2) + (y as i32 - 17).pow(2) < 29;
            if body {
                pixels[(y * 32 + x) * 4..(y * 32 + x) * 4 + 4].copy_from_slice(if lens {
                    &[255, 255, 255, 255]
                } else {
                    &[178, 96, 0, 255]
                });
            }
        }
    }
    Ok(unsafe { CreateIcon(None, 32, 32, 1, 32, [0u8; 128].as_ptr(), pixels.as_ptr())? })
}
fn notify_icon(hwnd: HWND, icon: HICON, op: NOTIFY_ICON_MESSAGE) -> Result<()> {
    let mut data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_APP + 1,
        hIcon: icon,
        ..Default::default()
    };
    let title = wide("Scheduled Screenshot — open task manager");
    data.szTip[..title.len()].copy_from_slice(&title);
    unsafe {
        Shell_NotifyIconW(op, &data).ok()?;
    }
    Ok(())
}
unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message_id: u32,
    wp: WPARAM,
    lp: LPARAM,
) -> LRESULT {
    TRAY.with(|cell| {
        let borrowed = cell.borrow();
        let Some(tray) = borrowed.as_ref() else {
            return unsafe { DefWindowProcW(hwnd, message_id, wp, lp) };
        };
        if message_id == tray.explorer_message {
            let _ = notify_icon(hwnd, tray.icon, NIM_ADD);
            return LRESULT(0);
        }
        match message_id {
            m if m == WM_APP + 4 => {
                tray.restart.set(true);
                unsafe {
                    let _ = PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                }
            }
            m if m == WM_APP + 3 => apply_capture_shortcut(hwnd, &tray.app),
            WM_HOTKEY if wp.0 == CAPTURE_HOTKEY_ID as usize => {
                let matches = {
                    let state = tray.app.state.lock().unwrap();
                    let shortcut = &state.settings.capture_shortcut;
                    shortcut.enabled
                        && state.shortcut_active.as_deref() == Some(&shortcut.label())
                        && ((lp.0 as u32 >> 16) == shortcut.virtual_key().unwrap_or(0))
                        && ((lp.0 as u32 & 0xffff) == shortcut.modifiers_bits().unwrap_or(0))
                };
                if matches && let Err(error) = tray.app.capture_now() {
                    tray.app.store.log(&format!("Capture shortcut: {error:#}"));
                }
            }
            m if m == WM_APP + 2 => {
                let _ = open(&tray.url);
            }
            m if m == WM_APP + 1 => {
                if lp.0 as u32 == WM_LBUTTONUP {
                    let _ = open(&tray.url);
                }
                if lp.0 as u32 == WM_RBUTTONUP {
                    unsafe {
                        if let Ok(menu) = CreatePopupMenu() {
                            let (paused, language) = {
                                let state = tray.app.state.lock().unwrap();
                                (state.settings.paused, state.settings.language.clone())
                            };
                            let append = |id, key| {
                                let label = wide(crate::localization::text(&language, key));
                                let _ = AppendMenuW(menu, MF_STRING, id, PCWSTR(label.as_ptr()));
                            };
                            append(1, "Open task manager");
                            append(
                                2,
                                if paused {
                                    "Resume scheduling"
                                } else {
                                    "Pause scheduling"
                                },
                            );
                            let shortcut = tray.app.state.lock().unwrap().shortcut_active.clone();
                            let mut capture_label =
                                crate::localization::text(&language, "Capture now").to_string();
                            if let Some(shortcut) = shortcut {
                                capture_label.push_str(&format!("\t{shortcut}"));
                            }
                            let _ = AppendMenuW(
                                menu,
                                MF_STRING,
                                3,
                                PCWSTR(wide(capture_label).as_ptr()),
                            );
                            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                            append(4, "Open output folder");
                            append(5, "Open logs");
                            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                            append(7, "Restart application");
                            append(6, "Exit");
                            let mut point = POINT::default();
                            let _ = GetCursorPos(&mut point);
                            let _ = SetForegroundWindow(hwnd);
                            let selected = TrackPopupMenu(
                                menu,
                                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                                point.x,
                                point.y,
                                None,
                                hwnd,
                                None,
                            )
                            .0 as usize;
                            let _ = DestroyMenu(menu);
                            let result = match selected {
                                1 => open(&tray.url),
                                2 => {
                                    let revision = tray.app.state.lock().unwrap().settings.revision;
                                    tray.app.pause(!paused, revision).map(|_| ())
                                }
                                3 => tray.app.capture_now(),
                                4 => tray
                                    .app
                                    .manual_output_folder()
                                    .and_then(|folder| open_folder(&folder)),
                                5 => open(&tray.app.store.data.join("logs").to_string_lossy()),
                                7 => request_restart(),
                                6 => {
                                    let _ =
                                        PostMessageW(Some(hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                                    Ok(())
                                }
                                _ => Ok(()),
                            };
                            if let Err(e) = result {
                                message(&format!("{e:#}"));
                            }
                        }
                    }
                }
            }
            WM_WTSSESSION_CHANGE => {
                let event = wp.0 as u32;
                if matches!(
                    event,
                    WTS_SESSION_LOCK
                        | WTS_CONSOLE_DISCONNECT
                        | WTS_REMOTE_DISCONNECT
                        | WTS_SESSION_LOGOFF
                ) {
                    tray.app.notify(Some(false));
                } else if matches!(
                    event,
                    WTS_SESSION_UNLOCK
                        | WTS_CONSOLE_CONNECT
                        | WTS_REMOTE_CONNECT
                        | WTS_SESSION_LOGON
                ) {
                    tray.app.notify(Some(desktop_available()));
                }
            }
            WM_POWERBROADCAST => {
                if wp.0 as u32 == PBT_APMSUSPEND {
                    tray.app.notify(Some(false));
                } else if matches!(wp.0 as u32, PBT_APMRESUMEAUTOMATIC | PBT_APMRESUMESUSPEND) {
                    tray.app.notify(Some(desktop_available()));
                }
            }
            WM_TIMECHANGE | WM_SETTINGCHANGE | WM_DISPLAYCHANGE => tray.app.notify(None),
            WM_QUERYENDSESSION => {
                tray.app.stop();
                return LRESULT(1);
            }
            WM_CLOSE => {
                tray.app.stop();
                let _ = notify_icon(hwnd, tray.icon, NIM_DELETE);
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
            WM_DESTROY => unsafe {
                let _ = UnregisterHotKey(Some(hwnd), CAPTURE_HOTKEY_ID);
                PostQuitMessage(0);
            },
            _ => return unsafe { DefWindowProcW(hwnd, message_id, wp, lp) },
        }
        LRESULT(0)
    })
}
pub fn run_tray(app: App, url: String, _instance: &Instance, background: bool) -> Result<bool> {
    let _com = Com::init()?;
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
    let icon = tray_icon()?;
    unsafe {
        let module = GetModuleHandleW(None)?;
        let wc = WNDCLASSW {
            lpfnWndProc: Some(window_proc),
            hInstance: module.into(),
            lpszClassName: INSTANCE_CLASS,
            hIcon: icon,
            ..Default::default()
        };
        ensure!(RegisterClassW(&wc) != 0, "Could not register tray window.");
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            INSTANCE_CLASS,
            w!("Scheduled Screenshot"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(module.into()),
            None,
        )?;
        TRAY.with(|cell| {
            *cell.borrow_mut() = Some(Tray {
                app: app.clone(),
                url: url.clone(),
                icon,
                explorer_message: RegisterWindowMessageW(w!("TaskbarCreated")),
                restart: Cell::new(false),
            })
        });
        WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION)?;
        notify_icon(hwnd, icon, NIM_ADD)?;
        app.notify(Some(desktop_available()));
        let _hotkey = CaptureHotkey(hwnd);
        apply_capture_shortcut(hwnd, &app);
        if !background {
            open(&url)?;
        }
        let mut msg = MSG::default();
        loop {
            let result = GetMessageW(&mut msg, None, 0, 0).0;
            if result <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        app.stop();
        let _ = WTSUnRegisterSessionNotification(hwnd);
        let _ = notify_icon(hwnd, icon, NIM_DELETE);
        let restart = TRAY.with(|cell| {
            cell.borrow_mut()
                .take()
                .is_some_and(|tray| tray.restart.get())
        });
        let _ = DestroyIcon(icon);
        Ok(restart)
    }
}
