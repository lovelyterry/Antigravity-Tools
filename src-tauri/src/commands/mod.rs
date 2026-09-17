use crate::modules;

// 导出 proxy 命令
pub mod proxy;
// 导出 security 命令 (IP 监控)
pub mod security;
// 导出 proxy_pool 命令
pub mod proxy_pool;
// 导出 user_token 命令
pub mod user_token;
// 导出 patch 命令
pub mod patch;

pub use modules::account::RefreshStats;
pub use crate::modules::update_checker::UpdateInfo;

/// 刷新所有账号配额 (内部实现)
pub async fn refresh_all_quotas_internal(
    proxy_state: &crate::commands::proxy::ProxyServiceState,
) -> Result<RefreshStats, String> {
    let stats = modules::account::refresh_all_quotas_logic().await?;

    // 同步到运行中的反代服务（如果已启动）
    let instance_lock = proxy_state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        let _ = instance.token_manager.reload_all_accounts().await;
    }

    Ok(stats)
}

/// 获取 Antigravity 可执行文件路径
pub async fn get_antigravity_path(bypass_config: Option<bool>) -> Result<String, String> {
    // 1. 优先从配置查询 (除非明确要求绕过)
    if bypass_config != Some(true) {
        if let Ok(config) = crate::modules::config::load_app_config() {
            if let Some(path) = config.antigravity_executable {
                if std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    // 2. 执行实时探测
    match crate::modules::process::get_antigravity_executable_path(None) {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("未找到 Antigravity 安装路径".to_string()),
    }
}

/// 获取 Antigravity CLI (agy) 可执行文件路径
pub async fn get_antigravity_cli_path(bypass_config: Option<bool>) -> Result<String, String> {
    // 1. 优先从配置查询 (除非明确要求绕过)
    if bypass_config != Some(true) {
        if let Ok(config) = crate::modules::config::load_app_config() {
            if let Some(path) = config.antigravity_cli_executable {
                if std::path::Path::new(&path).exists() {
                    return Ok(path);
                }
            }
        }
    }

    // 2. 执行实时探测
    match crate::modules::process::get_antigravity_cli_executable_path() {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("未找到 Antigravity CLI (agy) 安装路径".to_string()),
    }
}

/// 获取 Antigravity 启动参数
pub async fn get_antigravity_args() -> Result<Vec<String>, String> {
    match crate::modules::process::get_args_from_running_process(None) {
        Some(args) => Ok(args),
        None => Err("未找到正在运行的 Antigravity 进程".to_string()),
    }
}

/// 检测 GitHub releases 更新
pub async fn check_for_updates() -> Result<UpdateInfo, String> {
    modules::logger::log_info("收到更新检查请求");
    crate::modules::update_checker::check_for_updates().await
}

pub async fn should_check_updates() -> Result<bool, String> {
    let settings = crate::modules::update_checker::load_update_settings()?;
    Ok(crate::modules::update_checker::should_check_for_updates(
        &settings,
    ))
}

pub async fn update_last_check_time() -> Result<(), String> {
    crate::modules::update_checker::update_last_check_time()
}

/// 检测是否通过 Homebrew Cask 安装
pub async fn check_homebrew_installation() -> Result<bool, String> {
    Ok(crate::modules::update_checker::is_homebrew_installed())
}

/// 检测是否以 AppImage 方式运行（Linux 专用）
pub async fn check_appimage_installation() -> Result<bool, String> {
    Ok(crate::modules::update_checker::is_appimage_running())
}

/// 通过 Homebrew Cask 升级应用
pub async fn brew_upgrade_cask() -> Result<String, String> {
    modules::logger::log_info("收到 Homebrew 升级请求");
    crate::modules::update_checker::brew_upgrade_cask().await
}

/// 获取更新设置
pub async fn get_update_settings() -> Result<crate::modules::update_checker::UpdateSettings, String> {
    crate::modules::update_checker::load_update_settings()
}

/// 保存更新设置
pub async fn save_update_settings(
    settings: crate::modules::update_checker::UpdateSettings,
) -> Result<(), String> {
    crate::modules::update_checker::save_update_settings(&settings)
}

/// 清理日志缓存
pub async fn clear_log_cache() -> Result<(), String> {
    modules::logger::clear_logs()
}

/// 清理 Antigravity 应用缓存
pub async fn clear_antigravity_cache() -> Result<modules::cache::ClearResult, String> {
    modules::cache::clear_antigravity_cache(None)
}

/// 获取 Antigravity 缓存路径列表（用于预览）
pub async fn get_antigravity_cache_paths() -> Result<Vec<String>, String> {
    Ok(modules::cache::get_existing_cache_paths()
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect())
}

/// 打开数据目录
pub async fn open_data_folder() -> Result<(), String> {
    let path = modules::account::get_data_dir()?;

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "windows")]
    {
        use crate::utils::command::CommandExtWrapper;
        std::process::Command::new("explorer")
            .creation_flags_windows()
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|e| format!("打开文件夹失败: {}", e))?;
    }

    Ok(())
}

/// 获取数据目录绝对路径
pub async fn get_data_dir_path() -> Result<String, String> {
    let path = modules::account::get_data_dir()?;
    Ok(modules::account::format_data_dir_path(&path))
}

/// 预热所有可用账号
pub async fn warm_up_all_accounts() -> Result<String, String> {
    modules::quota::warm_up_all_accounts().await
}

/// 预热指定账号
pub async fn warm_up_account(account_id: String) -> Result<String, String> {
    modules::quota::warm_up_account(&account_id).await
}
