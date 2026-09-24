use crate::models::Account;
use crate::modules::{account, config, logger, quota};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use tokio::time::{self, Duration};

// Warmup history: key = "email:bucket_or_model:weekly:cycle_id", value = warmup timestamp
static WARMUP_HISTORY: Lazy<Mutex<HashMap<String, i64>>> =
    Lazy::new(|| Mutex::new(load_warmup_history()));

fn get_warmup_history_path() -> Result<PathBuf, String> {
    let data_dir = account::get_data_dir()?;
    Ok(data_dir.join("warmup_history.json"))
}

fn load_warmup_history() -> HashMap<String, i64> {
    match get_warmup_history_path() {
        Ok(path) if path.exists() => match std::fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
            Err(_) => HashMap::new(),
        },
        _ => HashMap::new(),
    }
}

fn save_warmup_history(history: &HashMap<String, i64>) {
    if let Ok(path) = get_warmup_history_path() {
        if let Ok(content) = serde_json::to_string_pretty(history) {
            let _ = std::fs::write(&path, content);
        }
    }
}

pub fn record_warmup_history(key: &str, timestamp: i64) {
    let mut history = WARMUP_HISTORY.lock().unwrap();
    history.insert(key.to_string(), timestamp);
    save_warmup_history(&history);
}

pub fn check_cooldown(key: &str, cooldown_seconds: i64) -> bool {
    let history = WARMUP_HISTORY.lock().unwrap();
    if let Some(&last_ts) = history.get(key) {
        let now = chrono::Utc::now().timestamp();
        now - last_ts < cooldown_seconds
    } else {
        false
    }
}

/// Helper to parse ISO8601 / RFC3339 string to timestamp
fn parse_reset_time_ts(s: &str) -> Option<i64> {
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.timestamp());
    }
    if let Ok(dt) = DateTime::parse_from_str(s, "%Y/%m/%d %H:%M:%S") {
        return Some(dt.timestamp());
    }
    None
}

/// Select a model to ping for a given group, honoring user's monitored_models preference if available
fn pick_model_for_group(group_name: &str, bucket_id: &str, monitored_models: &[String]) -> String {
    let is_3p = bucket_id.to_lowercase().contains("3p")
        || group_name.to_lowercase().contains("claude")
        || group_name.to_lowercase().contains("gpt");

    if is_3p {
        if let Some(m) = monitored_models.iter().find(|m| {
            let l = m.to_lowercase();
            l.contains("claude") || l.contains("gpt")
        }) {
            return m.clone();
        }
        "claude-sonnet-4-6".to_string()
    } else {
        if let Some(m) = monitored_models.iter().find(|m| {
            let l = m.to_lowercase();
            l.contains("gemini")
        }) {
            return m.clone();
        }
        "gemini-3-flash".to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuotaWindowType {
    Weekly,
    FiveHour,
}

impl QuotaWindowType {
    fn from_window_and_bucket(window: &str, bucket_id: &str) -> Option<Self> {
        let win_lower = window.to_lowercase();
        let bid_lower = bucket_id.to_lowercase();
        if win_lower.contains("week")
            || bid_lower.contains("week")
            || win_lower.contains("7d")
            || bid_lower.contains("7d")
        {
            Some(Self::Weekly)
        } else if win_lower.contains("5h")
            || bid_lower.contains("5h")
            || win_lower.contains("5 hour")
            || win_lower.contains("5-hour")
            || win_lower.contains("300m")
        {
            Some(Self::FiveHour)
        } else {
            None
        }
    }

    fn from_reset_diff(seconds_until_reset: i64) -> Self {
        if seconds_until_reset <= 6 * 3600 {
            Self::FiveHour
        } else {
            Self::Weekly
        }
    }

    fn cooldown_seconds(&self) -> i64 {
        match self {
            Self::Weekly => 6 * 86400,          // 6 days
            Self::FiveHour => 4 * 3600 + 1800,  // 4.5 hours
        }
    }

    fn tag(&self) -> &'static str {
        match self {
            Self::Weekly => "weekly",
            Self::FiveHour => "5h",
        }
    }

    fn label(&self) -> &'static str {
        match self {
            Self::Weekly => "7-Day Weekly",
            Self::FiveHour => "5-Hour Rolling",
        }
    }
}

/// Start smart weekly & 5-hour scheduler
pub fn start_scheduler(
    proxy_state: crate::commands::proxy::ProxyServiceState,
) {
    tokio::spawn(async move {
        logger::log_info("[Scheduler] Smart Warmup Scheduler started. Monitoring 7-day weekly & 5-hour quota windows...");

        // Scan every 5 minutes (300s) to check for accounts reaching weekly reset time
        let mut interval = time::interval(Duration::from_secs(300));

        loop {
            interval.tick().await;

            // Load configuration
            let Ok(app_config) = config::load_app_config() else {
                continue;
            };

            // Must be enabled by user in Settings
            if !app_config.scheduled_warmup.enabled {
                continue;
            }

            let Ok(accounts) = account::list_accounts() else {
                continue;
            };

            if accounts.is_empty() {
                continue;
            }

            let now_ts = Utc::now().timestamp();
            let mut tasks_to_run = Vec::new();

            for acc in &accounts {
                if acc.disabled || acc.proxy_disabled {
                    continue;
                }

                let Ok((token, pid)) = quota::get_valid_token_for_warmup(acc).await else {
                    continue;
                };

                let Ok((fresh_quota, _)) =
                    quota::fetch_quota_with_cache(&token, &acc.email, Some(&pid), Some(&acc.id))
                        .await
                else {
                    continue;
                };

                if fresh_quota.is_forbidden {
                    continue;
                }

                // Check quota_groups for WEEKLY buckets
                if let Some(groups) = &fresh_quota.quota_groups {
                    for group in groups {
                        for bucket in &group.buckets {
                            let Some(win_type) = QuotaWindowType::from_window_and_bucket(&bucket.window, &bucket.bucket_id) else {
                                continue;
                            };

                            if win_type == QuotaWindowType::FiveHour && !app_config.scheduled_warmup.warmup_5h {
                                continue;
                            }

                            // If fraction is 1.0 (100% full)
                            if bucket.remaining_fraction >= 0.999 {
                                let reset_ts_opt = parse_reset_time_ts(&bucket.reset_time);
                                let should_warmup = match reset_ts_opt {
                                    Some(reset_ts) => {
                                        now_ts >= reset_ts - 60
                                    }
                                    None => {
                                        true
                                    }
                                };

                                if should_warmup {
                                    let history_key = match reset_ts_opt {
                                        Some(reset_ts) => format!(
                                            "{}:{}:{}:{}",
                                            acc.email, bucket.bucket_id, win_type.tag(), reset_ts
                                        ),
                                        None => format!(
                                            "{}:{}:{}:initial",
                                            acc.email, bucket.bucket_id, win_type.tag()
                                        ),
                                    };

                                    if !check_cooldown(&history_key, win_type.cooldown_seconds()) {
                                        let model_to_ping = pick_model_for_group(
                                            &group.display_name,
                                            &bucket.bucket_id,
                                            &app_config.scheduled_warmup.monitored_models,
                                        );

                                        tasks_to_run.push((
                                            acc.id.clone(),
                                            acc.email.clone(),
                                            model_to_ping,
                                            token.clone(),
                                            pid.clone(),
                                            history_key,
                                            win_type.label(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    // Fallback to models if quota_groups is not populated
                    for model in &fresh_quota.models {
                        if model.percentage == 100 {
                            if !app_config
                                .scheduled_warmup
                                .monitored_models
                                .contains(&model.name)
                            {
                                continue;
                            }
                            if let Some(reset_ts) = parse_reset_time_ts(&model.reset_time) {
                                let win_type = QuotaWindowType::from_reset_diff(reset_ts - now_ts);
                                if win_type == QuotaWindowType::FiveHour && !app_config.scheduled_warmup.warmup_5h {
                                    continue;
                                }
                                if now_ts >= reset_ts - 60 {
                                    let history_key =
                                        format!("{}:{}:{}:{}", acc.email, model.name, win_type.tag(), reset_ts);
                                    if !check_cooldown(&history_key, win_type.cooldown_seconds()) {
                                        tasks_to_run.push((
                                            acc.id.clone(),
                                            acc.email.clone(),
                                            model.name.clone(),
                                            token.clone(),
                                            pid.clone(),
                                            history_key,
                                            win_type.label(),
                                        ));
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Execute weekly warmup tasks
            if !tasks_to_run.is_empty() {
                logger::log_info(&format!(
                    "[Scheduler] 🎯 Reached weekly reset for {} account targets. Triggering warmup...",
                    tasks_to_run.len()
                ));

                let state_for_warmup = proxy_state.clone();

                tokio::spawn(async move {
                    for (acc_id, email, model, token, pid, history_key, win_label) in tasks_to_run {
                        logger::log_info(&format!(
                            "[SmartWarmup] 🚀 Triggering {} warmup for {} @ {}",
                            win_label, model, email
                        ));

                        let success = quota::warmup_model_directly(
                            &token,
                            &model,
                            &pid,
                            &email,
                            100,
                            Some(&acc_id),
                        )
                        .await;

                        if success {
                            logger::log_info(&format!(
                                "[SmartWarmup] ✅ Successfully warmed up ({}) {} for {}",
                                win_label, model, email
                            ));
                            record_warmup_history(&history_key, chrono::Utc::now().timestamp());
                        } else {
                            logger::log_warn(&format!(
                                "[SmartWarmup] ❌ Warmup failed ({}) for {} on {}",
                                win_label, model, email
                            ));
                        }
                        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    }

                    // Refresh UI
                    tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
                    let _ = crate::commands::refresh_all_quotas_internal(
                        &state_for_warmup,
                    )
                    .await;
                });
            }

            // Regularly clean up history (keep last 30 days)
            {
                let now_ts = Utc::now().timestamp();
                let mut history = WARMUP_HISTORY.lock().unwrap();
                let cutoff = now_ts - 30 * 86400;
                history.retain(|_, &mut ts| ts > cutoff);
            }
        }
    });
}

/// Trigger immediate smart warmup check for a single account (e.g. on manual trigger / recovered event)
pub async fn trigger_warmup_for_account(account: &Account) {
    let Ok((token, pid)) = quota::get_valid_token_for_warmup(account).await else {
        return;
    };

    let Ok((fresh_quota, _)) =
        quota::fetch_quota_with_cache(&token, &account.email, Some(&pid), Some(&account.id)).await
    else {
        return;
    };

    if fresh_quota.is_forbidden {
        return;
    }

    let Ok(app_config) = config::load_app_config() else {
        return;
    };

    if !app_config.scheduled_warmup.enabled {
        return;
    }

    let now_ts = Utc::now().timestamp();
    if let Some(groups) = fresh_quota.quota_groups {
        for group in groups {
            for bucket in group.buckets {
                let Some(win_type) = QuotaWindowType::from_window_and_bucket(&bucket.window, &bucket.bucket_id) else {
                    continue;
                };

                if win_type == QuotaWindowType::FiveHour && !app_config.scheduled_warmup.warmup_5h {
                    continue;
                }

                if bucket.remaining_fraction >= 0.999 {
                    let reset_ts_opt = parse_reset_time_ts(&bucket.reset_time);
                    let should_warmup = match reset_ts_opt {
                        Some(reset_ts) => now_ts >= reset_ts - 60,
                        None => true,
                    };

                    if should_warmup {
                        let history_key = match reset_ts_opt {
                            Some(reset_ts) => {
                                format!(
                                    "{}:{}:{}:{}",
                                    account.email, bucket.bucket_id, win_type.tag(), reset_ts
                                )
                            }
                            None => {
                                format!("{}:{}:{}:initial", account.email, bucket.bucket_id, win_type.tag())
                            }
                        };

                        if !check_cooldown(&history_key, win_type.cooldown_seconds()) {
                            let model_to_ping = pick_model_for_group(
                                &group.display_name,
                                &bucket.bucket_id,
                                &app_config.scheduled_warmup.monitored_models,
                            );

                            let success = quota::warmup_model_directly(
                                &token,
                                &model_to_ping,
                                &pid,
                                &account.email,
                                100,
                                Some(&account.id),
                            )
                            .await;

                            if success {
                                record_warmup_history(&history_key, now_ts);
                            }
                        }
                    }
                }
            }
        }
    }
}
