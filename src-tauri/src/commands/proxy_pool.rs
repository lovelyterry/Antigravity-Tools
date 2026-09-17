use crate::commands::proxy::ProxyServiceState;
use std::collections::HashMap;

/// Bind an account to a specific proxy
pub async fn bind_account_proxy(
    state: &ProxyServiceState,
    account_id: String,
    proxy_id: String,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance
            .axum_server
            .proxy_pool_manager
            .bind_account_to_proxy(account_id, proxy_id)
            .await
    } else {
        Err("Service not running".to_string())
    }
}

/// Unbind an account from its proxy
pub async fn unbind_account_proxy(
    state: &ProxyServiceState,
    account_id: String,
) -> Result<(), String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        instance
            .axum_server
            .proxy_pool_manager
            .unbind_account_proxy(account_id)
            .await;
        Ok(())
    } else {
        Err("Service not running".to_string())
    }
}

/// Get the proxy binding for a specific account
pub async fn get_account_proxy_binding(
    state: &ProxyServiceState,
    account_id: String,
) -> Result<Option<String>, String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        Ok(instance
            .axum_server
            .proxy_pool_manager
            .get_account_binding(&account_id))
    } else {
        Err("Service not running".to_string())
    }
}

/// Get all account proxy bindings
pub async fn get_all_account_bindings(
    state: &ProxyServiceState,
) -> Result<HashMap<String, String>, String> {
    let instance_lock = state.instance.read().await;
    if let Some(instance) = instance_lock.as_ref() {
        Ok(instance
            .axum_server
            .proxy_pool_manager
            .get_all_bindings_snapshot())
    } else {
        Err("Service not running".to_string())
    }
}
