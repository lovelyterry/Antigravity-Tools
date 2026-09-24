use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::modules::account;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredModel {
    pub id: String,
    pub display_name: String,
    pub short_label: String,
    pub group: String,
    pub supports_thinking: bool,
    pub supports_images: bool,
    pub recommended: bool,
    pub last_seen: i64,
}

static DISCOVERED_MODELS: Lazy<RwLock<HashMap<String, DiscoveredModel>>> =
    Lazy::new(|| RwLock::new(load_models_from_disk()));

fn get_storage_path() -> Result<PathBuf, String> {
    let data_dir = account::get_data_dir()?;
    Ok(data_dir.join("discovered_models.json"))
}

fn load_models_from_disk() -> HashMap<String, DiscoveredModel> {
    if let Ok(path) = get_storage_path() {
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(map) = serde_json::from_str::<HashMap<String, DiscoveredModel>>(&content) {
                    return map;
                }
            }
        }
    }
    HashMap::new()
}

fn save_models_to_disk(models: &HashMap<String, DiscoveredModel>) {
    if let Ok(path) = get_storage_path() {
        if let Ok(content) = serde_json::to_string_pretty(models) {
            let _ = fs::write(&path, content);
        }
    }
}

/// 自动推断模型的系列分组 (Group)
pub fn detect_group(id: &str) -> String {
    let lower = id.to_lowercase();
    if lower.starts_with("gemini-3") {
        "Gemini 3".to_string()
    } else if lower.starts_with("gemini-2.5") {
        "Gemini 2.5".to_string()
    } else if lower.starts_with("gemini-2") {
        "Gemini 2".to_string()
    } else if lower.starts_with("gemini") {
        "Gemini".to_string()
    } else if lower.starts_with("claude") {
        "Claude".to_string()
    } else if lower.starts_with("gpt") {
        "OpenAI".to_string()
    } else {
        "Other".to_string()
    }
}

/// 自动生成精炼短标签 (如 "G3.1 Pro", "Claude 3.5 Sonnet")
pub fn generate_short_label(id: &str, display_name: &str) -> String {
    if !display_name.is_empty()
        && !display_name.eq_ignore_ascii_case("Dynamic Extracted Model")
        && !display_name.eq_ignore_ascii_case(id)
    {
        let dn = display_name.trim();
        // 针对 Gemini 做简洁精炼处理
        if let Some(rest) = dn.strip_prefix("Gemini ") {
            return format!("G{}", rest);
        }
        return dn.to_string();
    }

    // 从 ID 回退推断
    let lower = id.to_lowercase();
    if lower.contains("claude-sonnet-4-6") {
        return "Claude 4.6".to_string();
    }
    if lower.contains("claude-opus-4-6") {
        return "Claude Opus 4.6".to_string();
    }
    if lower.contains("claude-3-5-sonnet") {
        return "Claude 3.5".to_string();
    }
    if lower.contains("gpt-oss") {
        return "GPT-OSS".to_string();
    }

    // gemini-x.y-xxx
    if let Some(stripped) = lower.strip_prefix("gemini-") {
        let parts: Vec<&str> = stripped.split('-').collect();
        if !parts.is_empty() {
            let ver = parts[0];
            let tier = parts.get(1).copied().unwrap_or("");
            let sub = parts.get(2).copied().unwrap_or("");
            let mut label = format!("G{}", ver);
            if !tier.is_empty() {
                label.push(' ');
                let mut c = tier.chars();
                if let Some(f) = c.next() {
                    label.push_str(&f.to_uppercase().collect::<String>());
                    label.push_str(c.as_str());
                }
            }
            if !sub.is_empty() && sub != "tiered" {
                label.push(' ');
                let mut c = sub.chars();
                if let Some(f) = c.next() {
                    label.push_str(&f.to_uppercase().collect::<String>());
                    label.push_str(c.as_str());
                }
            }
            return label;
        }
    }

    id.to_string()
}

fn register_model_internal(
    cache: &mut HashMap<String, DiscoveredModel>,
    id: &str,
    display_name: Option<&str>,
    supports_thinking: Option<bool>,
    supports_images: Option<bool>,
    recommended: Option<bool>,
    now: i64,
) -> bool {
    let id_trimmed = id.trim();
    if id_trimmed.is_empty() {
        return false;
    }

    let lower = id_trimmed.to_lowercase();
    if !lower.starts_with("gemini") && !lower.starts_with("claude") && !lower.starts_with("gpt") && !lower.starts_with("image") {
        return false;
    }

    let display = display_name
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or(id_trimmed);

    let short_label = generate_short_label(id_trimmed, display);
    let group = detect_group(id_trimmed);

    if let Some(existing) = cache.get_mut(id_trimmed) {
        let mut changed = false;
        if !display_name.unwrap_or("").is_empty() && existing.display_name != display {
            existing.display_name = display.to_string();
            existing.short_label = short_label;
            changed = true;
        }
        if let Some(st) = supports_thinking {
            if existing.supports_thinking != st {
                existing.supports_thinking = st;
                changed = true;
            }
        }
        if let Some(si) = supports_images {
            if existing.supports_images != si {
                existing.supports_images = si;
                changed = true;
            }
        }
        if let Some(rec) = recommended {
            if existing.recommended != rec {
                existing.recommended = rec;
                changed = true;
            }
        }
        existing.last_seen = now;
        changed
    } else {
        cache.insert(
            id_trimmed.to_string(),
            DiscoveredModel {
                id: id_trimmed.to_string(),
                display_name: display.to_string(),
                short_label,
                group,
                supports_thinking: supports_thinking.unwrap_or(false),
                supports_images: supports_images.unwrap_or(false),
                recommended: recommended.unwrap_or(false),
                last_seen: now,
            },
        );
        true
    }
}

/// 注册单个从 Google 获取到的模型信息
pub fn register_single_model(
    id: &str,
    display_name: Option<&str>,
    supports_thinking: Option<bool>,
    supports_images: Option<bool>,
    recommended: Option<bool>,
) {
    let now = chrono::Utc::now().timestamp();
    let mut cache = DISCOVERED_MODELS.write().unwrap();
    if register_model_internal(&mut cache, id, display_name, supports_thinking, supports_images, recommended, now) {
        save_models_to_disk(&cache);
    }
}

/// 批量注册模型信息（一次性刷盘，避免 I/O 风暴）
pub fn register_models_batch<I, S>(models: I)
where
    I: IntoIterator<Item = (S, Option<S>, Option<bool>, Option<bool>, Option<bool>)>,
    S: AsRef<str>,
{
    let now = chrono::Utc::now().timestamp();
    let mut cache = DISCOVERED_MODELS.write().unwrap();
    let mut any_changed = false;

    for (id, dn, st, si, rec) in models {
        if register_model_internal(
            &mut cache,
            id.as_ref(),
            dn.as_ref().map(|s| s.as_ref()),
            st,
            si,
            rec,
            now,
        ) {
            any_changed = true;
        }
    }

    if any_changed {
        save_models_to_disk(&cache);
    }
}

/// 获取全局发现的公用模型列表（按组和名称排序）
pub fn list_discovered_models() -> Vec<DiscoveredModel> {
    let cache = DISCOVERED_MODELS.read().unwrap();
    let mut list: Vec<DiscoveredModel> = cache.values().cloned().collect();
    list.sort_by(|a, b| {
        if a.group != b.group {
            a.group.cmp(&b.group)
        } else {
            a.id.cmp(&b.id)
        }
    });
    list
}

/// 从已存在的 accounts/*.json 中初始化（冷启动恢复，批量保存）
pub fn init_from_existing_accounts() {
    let Ok(accounts) = account::list_accounts() else {
        return;
    };
    let now = chrono::Utc::now().timestamp();
    let mut cache = DISCOVERED_MODELS.write().unwrap();
    let mut any_changed = false;

    for acc in accounts {
        if let Some(quota) = acc.quota {
            for m in quota.models {
                if register_model_internal(
                    &mut cache,
                    &m.name,
                    m.display_name.as_deref(),
                    m.supports_thinking,
                    m.supports_images,
                    m.recommended,
                    now,
                ) {
                    any_changed = true;
                }
            }
        }
    }

    if any_changed {
        save_models_to_disk(&cache);
    }
}
