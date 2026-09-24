pub mod commands;
pub mod constants;
pub mod error;
pub mod models;
pub mod modules;
pub mod proxy; // Proxy service module
pub mod utils;

use modules::logger;
use tracing::{error, info, warn};

/// Increase file descriptor limit for macOS to prevent "Too many open files" errors
#[cfg(target_os = "macos")]
fn increase_nofile_limit() {
    unsafe {
        let mut rl = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };

        if libc::getrlimit(libc::RLIMIT_NOFILE, &mut rl) == 0 {
            info!(
                "Current open file limit: soft={}, hard={}",
                rl.rlim_cur, rl.rlim_max
            );

            // Attempt to increase to 4096 or maximum hard limit
            let target = 4096.min(rl.rlim_max);
            if rl.rlim_cur < target {
                rl.rlim_cur = target;
                if libc::setrlimit(libc::RLIMIT_NOFILE, &rl) == 0 {
                    info!("Successfully increased hard file limit to {}", target);
                } else {
                    warn!("Failed to increase file descriptor limit");
                }
            }
        }
    }
}

/// Windows FFI calls to disable Efficiency Mode (EcoQoS / Power Throttling)
/// to prevent background freezes when minimized/hidden.
#[cfg(target_os = "windows")]
mod windows_api {
    type Bool = i32;
    type Handle = *mut std::ffi::c_void;

    #[repr(C)]
    struct ProcessPowerThrottlingState {
        version: u32,
        control_mask: u32,
        state_mask: u32,
    }

    #[link(name = "Kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn SetProcessInformation(
            h_process: Handle,
            process_information_class: u32,
            process_information: *mut std::ffi::c_void,
            process_information_size: u32,
        ) -> Bool;
    }

    pub fn disable_efficiency_mode() {
        unsafe {
            let mut state = ProcessPowerThrottlingState {
                version: 1,        // PROCESS_POWER_THROTTLING_STATE::VERSION
                control_mask: 0x1, // PROCESS_POWER_THROTTLING_CURRENT_EXECUTION_SPEED
                state_mask: 0,
            };
            let process_handle = GetCurrentProcess();
            // ProcessPowerThrottling = 4
            let res = SetProcessInformation(
                process_handle,
                4,
                &mut state as *mut _ as *mut std::ffi::c_void,
                std::mem::size_of::<ProcessPowerThrottlingState>() as u32,
            );
            if res == 0 {
                let err = std::io::Error::last_os_error();
                tracing::warn!(
                    "Failed to disable Windows Power Throttling / EcoQoS: {}",
                    err
                );
            } else {
                tracing::info!(
                    "Successfully disabled Windows Power Throttling / EcoQoS for the process."
                );
            }
        }
    }
}

pub fn run() {
    // Disable Windows background throttling/EcoQoS
    #[cfg(target_os = "windows")]
    windows_api::disable_efficiency_mode();

    // Increase file descriptor limit (macOS only)
    #[cfg(target_os = "macos")]
    increase_nofile_limit();

    // Initialize logger
    logger::init_logger();

    // Initialize token stats database
    if let Err(e) = modules::token_stats::init_db() {
        error!("Failed to initialize token stats database: {}", e);
    }

    // Initialize security database
    if let Err(e) = modules::security_db::init_db() {
        error!("Failed to initialize security database: {}", e);
    }

    // Initialize user token database
    if let Err(e) = modules::user_token_db::init_db() {
        error!("Failed to initialize user token database: {}", e);
    }

    // Initialize discovered common models from existing accounts
    modules::common_models::init_from_existing_accounts();

    info!("Starting Antigravity-Tools Headless Web Server...");

    let rt = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    rt.block_on(async {
        let proxy_state = commands::proxy::ProxyServiceState::new();

        // Load config
        match modules::config::load_app_config() {
            Ok(mut config) => {
                let mut modified = false;
                // 默认允许 LAN 访问（绑定 0.0.0.0）
                // 若设置 ABV_BIND_LOCAL_ONLY，则仅绑定 127.0.0.1
                let bind_local_only = std::env::var("ABV_BIND_LOCAL_ONLY")
                    .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
                    .unwrap_or(false);
                if bind_local_only {
                    config.proxy.allow_lan_access = false;
                    modified = true;
                } else {
                    config.proxy.allow_lan_access = true;
                }

                // Force auth mode to AllExceptHealth in headless mode if it's Off or Auto
                if matches!(config.proxy.auth_mode, crate::proxy::ProxyAuthMode::Off | crate::proxy::ProxyAuthMode::Auto) {
                    info!("Headless mode: Forcing auth_mode to AllExceptHealth for Web UI security");
                    config.proxy.auth_mode = crate::proxy::ProxyAuthMode::AllExceptHealth;
                    modified = true;
                }

                // 支持通过环境变量注入 API Key
                let env_key = std::env::var("ABV_API_KEY")
                    .or_else(|_| std::env::var("API_KEY"))
                    .ok();

                if let Some(key) = env_key {
                    if !key.trim().is_empty() {
                        info!("Using API Key from environment variable");
                        config.proxy.api_key = key;
                        modified = true;
                    }
                }

                // 支持通过环境变量注入 Web UI 密码
                let env_web_password = std::env::var("ABV_WEB_PASSWORD")
                    .or_else(|_| std::env::var("WEB_PASSWORD"))
                    .ok();

                if let Some(pwd) = env_web_password {
                    if !pwd.trim().is_empty() {
                        info!("Using Web UI Password from environment variable");
                        config.proxy.admin_password = Some(pwd);
                        modified = true;
                    }
                }

                // 支持通过环境变量注入鉴权模式
                let env_auth_mode = std::env::var("ABV_AUTH_MODE")
                    .or_else(|_| std::env::var("AUTH_MODE"))
                    .ok();

                if let Some(mode_str) = env_auth_mode {
                    let mode = match mode_str.to_lowercase().as_str() {
                        "off" => Some(crate::proxy::ProxyAuthMode::Off),
                        "strict" => Some(crate::proxy::ProxyAuthMode::Strict),
                        "all_except_health" => Some(crate::proxy::ProxyAuthMode::AllExceptHealth),
                        "auto" => Some(crate::proxy::ProxyAuthMode::Auto),
                        _ => {
                            warn!("Invalid AUTH_MODE: {}, ignoring", mode_str);
                            None
                        }
                    };
                    if let Some(m) = mode {
                        info!("Using Auth Mode from environment variable: {:?}", m);
                        config.proxy.auth_mode = m;
                        modified = true;
                    }
                }

                info!("--------------------------------------------------");
                info!("🚀 Headless mode proxy service starting...");
                info!("📍 Web UI: http://localhost:{}", config.proxy.port);
                info!("🔑 Current API Key: {}", config.proxy.api_key);
                if let Some(ref pwd) = config.proxy.admin_password {
                    info!("🔐 Web UI Password: {}", pwd);
                } else {
                    info!("🔐 Web UI Password: {}", config.proxy.api_key);
                }
                info!("💡 Tips: You can use these keys to login to Web UI and access AI APIs.");
                info!("--------------------------------------------------");

                // Persist environment overrides
                if modified {
                    if let Err(e) = modules::config::save_app_config(&config) {
                        error!("Failed to persist environment overrides: {}", e);
                    } else {
                        info!("Environment overrides persisted to gui_config.json");
                    }
                }

                // Start proxy service (Axum HTTP server with Web UI and APIs)
                if let Err(e) = commands::proxy::internal_start_proxy_service(
                    config.proxy,
                    &proxy_state,
                    crate::modules::integration::SystemManager::Headless,
                ).await {
                    error!("Failed to start server: {}", e);
                    std::process::exit(1);
                }

                info!("Server is running.");

                // Start smart scheduler for 7-day weekly reset warmup
                modules::scheduler::start_scheduler(proxy_state.clone());
                info!("Smart scheduler (7-Day Weekly Reset Warmup) started.");
            }
            Err(e) => {
                error!("Failed to load config: {}", e);
                std::process::exit(1);
            }
        }

        // Wait for Ctrl-C
        tokio::signal::ctrl_c().await.ok();
        info!("Server shutting down gracefully...");
    });
}
