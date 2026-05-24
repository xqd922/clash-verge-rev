use crate::cmds::import_profile;
use crate::config::IVerge;
use crate::{config::Config, core::*, utils::init, utils::server};
use crate::{log_err, trace_err};
use anyhow::Result;
use once_cell::sync::OnceCell;
use serde_yaml::Mapping;
use std::net::TcpListener;
use tauri::api::notification;
use tauri::{App, AppHandle, Manager};
#[cfg(not(target_os = "linux"))]
use window_shadows::set_shadow;

pub static VERSION: OnceCell<String> = OnceCell::new();
/// 当前启动是否为 warm-to-tray 模式(开机自启 --silent + enable_silent_start)。
/// true 时窗口已被预创建并移到屏幕外保活,前端启动后不应再主动 setFocus 抢焦点。
pub static WARM_TO_TRAY: OnceCell<bool> = OnceCell::new();

pub fn find_unused_port() -> Result<u16> {
    match TcpListener::bind("127.0.0.1:0") {
        Ok(listener) => {
            let port = listener.local_addr()?.port();
            Ok(port)
        }
        Err(_) => {
            let port = Config::verge()
                .latest()
                .verge_mixed_port
                .unwrap_or(Config::clash().data().get_mixed_port());
            log::warn!(target: "app", "use default port: {}", port);
            Ok(port)
        }
    }
}

/// handle something when start app
pub async fn resolve_setup(app: &mut App) {
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory);
    let version = app.package_info().version.to_string();
    handle::Handle::global().init(app.app_handle());
    VERSION.get_or_init(|| version.clone());

    log_err!(init::init_resources());
    log_err!(init::init_scheme());
    log_err!(init::startup_script());
    // 处理随机端口
    let enable_random_port = Config::verge().latest().enable_random_port.unwrap_or(false);

    let mut port = Config::verge()
        .latest()
        .verge_mixed_port
        .unwrap_or(Config::clash().data().get_mixed_port());

    if enable_random_port {
        port = find_unused_port().unwrap_or(
            Config::verge()
                .latest()
                .verge_mixed_port
                .unwrap_or(Config::clash().data().get_mixed_port()),
        );
    }

    Config::verge().data().patch_config(IVerge {
        verge_mixed_port: Some(port),
        ..IVerge::default()
    });
    let _ = Config::verge().data().save_file();
    let mut mapping = Mapping::new();
    mapping.insert("mixed-port".into(), port.into());
    Config::clash().data().patch_config(mapping);
    let _ = Config::clash().data().save_config();

    // 启动核心
    log::trace!("init config");

    log_err!(Config::init_config().await);

    log::trace!("launch core");
    log_err!(CoreManager::global().init());

    // setup a simple http server for singleton
    log::trace!("launch embed server");
    server::embed_server(app.app_handle());

    log::trace!("init system tray");
    log_err!(tray::Tray::update_systray(&app.app_handle()));

    let silent_start = Config::verge().data().enable_silent_start.unwrap_or(false);
    // 仅在开机自启（auto-launch 注册项带 --silent 参数）时进入 warm-to-tray 模式;
    // 手动双击 / 命令行启动总是正常显示窗口。
    let launched_silent = std::env::args().any(|a| a == "--silent");
    let warm_to_tray = silent_start && launched_silent;
    WARM_TO_TRAY.set(warm_to_tray).ok();
    // 始终创建窗口预热 WebView2 + 前端,避免用户首次从托盘打开承担冷启动延迟。
    // warm-to-tray 模式下随后移到屏幕外保活,用户首次点托盘走 create_window
    // 还原分支(set_position 回保存的可见位置)瞬间显示。
    create_window(&app.app_handle());
    #[cfg(target_os = "windows")]
    if warm_to_tray {
        if let Some(window) = app.app_handle().get_window("main") {
            set_window_taskbar_skip(&window, true);
            let _ = window.set_position(tauri::PhysicalPosition::new(-32000, -32000));
            // tauri WindowBuilder visible(false) 创建的窗口在 Windows 上是 lazy 的:
            // WebView2 渲染进程不会启动,前端 JS 也不会加载,_layout.tsx 里的
            // isWarmToTray 检查永远跑不到。必须 ShowWindow 触发实际渲染,但用
            // SW_SHOWNOACTIVATE 而非默认 SW_SHOW,避免开机时偷走用户当前活动
            // 窗口的焦点。
            if let Ok(hwnd) = window.hwnd() {
                use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
                unsafe { ShowWindow(hwnd.0, SW_SHOWNOACTIVATE) };
            }
        }
    }

    log_err!(sysopt::Sysopt::global().init_launch());
    log_err!(sysopt::Sysopt::global().init_sysproxy());

    log_err!(handle::Handle::update_systray_part());
    log_err!(hotkey::Hotkey::global().init(app.app_handle()));
    log_err!(timer::Timer::global().init());

    let argvs: Vec<String> = std::env::args().collect();
    if argvs.len() > 1 {
        let param = argvs[1].as_str();
        if param.starts_with("clash:") {
            log_err!(resolve_scheme(argvs[1].to_owned()).await);
        }
    }
}

/// reset system proxy
pub fn resolve_reset() {
    log_err!(sysopt::Sysopt::global().reset_sysproxy());
    tauri::async_runtime::block_on(async move {
        log_err!(CoreManager::global().stop_core().await);
        log_err!(service::unset_dns_by_service().await);
    });
}

/// Windows: 直接通过 SetWindowLongPtrW 改 WS_EX_TOOLWINDOW / WS_EX_APPWINDOW
/// 风格,绕开 tauri::Window::set_skip_taskbar —— 后者在 Windows 上为应用 ex_style
/// 改动会调 SW_HIDE,导致 DWM 拆合成层 + WebView2 GPU 合成器停止 → 还原冷启动。
/// 直接改 ex_style + SetWindowPos(SWP_FRAMECHANGED) 让任务栏更新但窗口保持
/// visible 状态,close-to-tray 屏幕外保活才能真生效。
#[cfg(target_os = "windows")]
pub fn set_window_taskbar_skip(window: &tauri::Window, skip: bool) {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
        SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
    };
    if let Ok(hwnd) = window.hwnd() {
        unsafe {
            let cur = GetWindowLongPtrW(hwnd.0, GWL_EXSTYLE);
            #[cfg(target_pointer_width = "32")]
            let (toolwindow, appwindow) =
                (WS_EX_TOOLWINDOW as i32, WS_EX_APPWINDOW as i32);
            #[cfg(target_pointer_width = "64")]
            let (toolwindow, appwindow) =
                (WS_EX_TOOLWINDOW as isize, WS_EX_APPWINDOW as isize);
            let new = if skip {
                (cur | toolwindow) & !appwindow
            } else {
                (cur & !toolwindow) | appwindow
            };
            SetWindowLongPtrW(hwnd.0, GWL_EXSTYLE, new);
            SetWindowPos(
                hwnd.0,
                0,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

/// create main window
pub fn create_window(app_handle: &AppHandle) {
    if let Some(window) = app_handle.get_window("main") {
        #[cfg(target_os = "windows")]
        set_window_taskbar_skip(&window, false);
        #[cfg(not(target_os = "windows"))]
        trace_err!(
            window.set_skip_taskbar(false),
            "set win skip_taskbar(false)"
        );
        // Windows 还原:把屏幕外的窗口移回上次保存的可见位置(配对 CloseRequested 中的 offscreen)
        #[cfg(target_os = "windows")]
        if let Some(pos) = Config::verge().latest().window_size_position.clone() {
            if pos.len() == 4 {
                trace_err!(
                    window.set_position(tauri::LogicalPosition::new(pos[2], pos[3])),
                    "set win position"
                );
            }
        }
        #[cfg(not(target_os = "windows"))]
        trace_err!(window.show(), "set win visible");
        #[cfg(target_os = "windows")]
        trace_err!(window.unminimize(), "set win unminimize");
        #[cfg(target_os = "windows")]
        trace_err!(window.show(), "set win visible");
        trace_err!(window.set_focus(), "set win focus");
        return;
    }

    let mut builder = tauri::window::WindowBuilder::new(
        app_handle,
        "main".to_string(),
        tauri::WindowUrl::App("index.html".into()),
    )
    .title("Clash Verge")
    .visible(false)
    .fullscreen(false)
    .min_inner_size(600.0, 520.0);

    match Config::verge().latest().window_size_position.clone() {
        Some(size_pos) if size_pos.len() == 4 => {
            let size = (size_pos[0], size_pos[1]);
            let pos = (size_pos[2], size_pos[3]);
            let w = size.0.clamp(600.0, f64::INFINITY);
            let h = size.1.clamp(520.0, f64::INFINITY);
            builder = builder.inner_size(w, h).position(pos.0, pos.1);
        }
        _ => {
            #[cfg(target_os = "windows")]
            {
                builder = builder.inner_size(800.0, 636.0).center();
            }

            #[cfg(target_os = "macos")]
            {
                builder = builder.inner_size(800.0, 642.0).center();
            }

            #[cfg(target_os = "linux")]
            {
                builder = builder.inner_size(800.0, 642.0).center();
            }
        }
    };
    #[cfg(target_os = "windows")]
    let window = builder
        .decorations(false)
        .additional_browser_args("--enable-features=msWebView2EnableDraggableRegions --disable-features=OverscrollHistoryNavigation,msExperimentalScrolling")
        .transparent(true)
        .visible(false)
        .build();
    #[cfg(target_os = "macos")]
    let window = builder
        .decorations(true)
        .hidden_title(true)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .build();
    #[cfg(target_os = "linux")]
    let window = builder.decorations(false).transparent(true).build();

    match window {
        Ok(win) => {
            let is_maximized = Config::verge()
                .latest()
                .window_is_maximized
                .unwrap_or(false);
            log::trace!("try to calculate the monitor size");
            let center = (|| -> Result<bool> {
                let mut center = false;
                let monitor = win.current_monitor()?.ok_or(anyhow::anyhow!(""))?;
                let size = monitor.size();
                let pos = win.outer_position()?;

                if pos.x < -400
                    || pos.x > (size.width - 200) as i32
                    || pos.y < -200
                    || pos.y > (size.height - 200) as i32
                {
                    center = true;
                }
                Ok(center)
            })();
            if center.unwrap_or(true) {
                trace_err!(win.center(), "set win center");
            }

            #[cfg(not(target_os = "linux"))]
            trace_err!(set_shadow(&win, true), "set win shadow");
            if is_maximized {
                trace_err!(win.maximize(), "set win maximize");
            }
        }
        Err(_) => {
            log::error!("failed to create window");
        }
    }
}

/// save window size and position
pub fn save_window_size_position(app_handle: &AppHandle, save_to_file: bool) -> Result<()> {
    let verge = Config::verge();
    let mut verge = verge.latest();

    if save_to_file {
        verge.save_file()?;
    }

    let win = app_handle
        .get_window("main")
        .ok_or(anyhow::anyhow!("failed to get window"))?;

    let scale = win.scale_factor()?;
    let size = win.inner_size()?;
    let size = size.to_logical::<f64>(scale);
    let pos = win.outer_position()?;
    // 窗口处于 close-to-tray 的屏幕外保活状态时,不要把 -32000 这种位置写回配置,
    // 避免 Moved 事件污染下次启动/还原使用的可见位置
    if pos.x < -10000 || pos.y < -10000 {
        return Ok(());
    }
    let pos = pos.to_logical::<f64>(scale);
    let is_maximized = win.is_maximized()?;
    verge.window_is_maximized = Some(is_maximized);
    if !is_maximized && size.width >= 600.0 && size.height >= 520.0 {
        verge.window_size_position = Some(vec![size.width, size.height, pos.x, pos.y]);
    }
    Ok(())
}

pub async fn resolve_scheme(param: String) -> Result<()> {
    let url = param
        .trim_start_matches("clash://install-config/?url=")
        .trim_start_matches("clash://install-config?url=");
    match import_profile(url.to_string(), None).await {
        Ok(_) => {
            notification::Notification::new(crate::utils::dirs::APP_ID)
                .title("Clash Verge")
                .body("Import profile success")
                .show()
                .unwrap();
        }
        Err(e) => {
            notification::Notification::new(crate::utils::dirs::APP_ID)
                .title("Clash Verge")
                .body(format!("Import profile failed: {e}"))
                .show()
                .unwrap();
            log::error!("Import profile failed: {e}");
        }
    }
    Ok(())
}
