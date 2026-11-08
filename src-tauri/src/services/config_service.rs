use std::fs;
use std::path::PathBuf;
use crate::models::AppConfig;
use crate::services::secret_service;

/// 获取应用数据目录
fn get_app_data_dir() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("无法获取数据目录"))?;
    Ok(data_dir.join("byakko-writer"))
}

/// 获取配置路径
fn get_config_path() -> anyhow::Result<PathBuf> {
    let app_dir = get_app_data_dir()?;
    Ok(app_dir.join("config.json"))
}

/// 把配置写入磁盘（先写临时文件再原子替换，避免并发写坏配置）
fn write_config(config: &AppConfig) -> anyhow::Result<()> {
    let config_path = get_config_path()?;

    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(config)?;
    let tmp_path = config_path.with_extension("json.tmp");
    fs::write(&tmp_path, content)?;
    fs::rename(&tmp_path, &config_path)?;

    Ok(())
}

/// 加载配置
///
/// API Key 在磁盘上是加密的（Windows DPAPI），这里解密后再返回给上层。
/// 如果发现是历史明文，会顺带迁移为加密存储。
pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = get_config_path()?;

    if !config_path.exists() {
        return Ok(AppConfig::default());
    }

    let content = fs::read_to_string(&config_path)?;
    let mut config: AppConfig = serde_json::from_str(&content)?;

    let raw_key = config.ai.api_key.clone();
    let plain_key = secret_service::unprotect_secret(&raw_key);
    config.ai.api_key = plain_key.clone();

    // 历史明文 Key：读取时顺便迁移成加密存储；失败不影响本次使用
    if !plain_key.is_empty() && !secret_service::is_protected(&raw_key) {
        let encrypted = secret_service::protect_secret(&plain_key);
        if secret_service::is_protected(&encrypted) {
            let mut migrated = config.clone();
            migrated.ai.api_key = encrypted;
            if let Err(e) = write_config(&migrated) {
                log::warn!("[Config] API Key 加密迁移失败: {}", e);
            }
        }
    }

    Ok(config)
}

/// 保存配置（API Key 加密后写盘）
pub fn save_config(mut config: AppConfig) -> anyhow::Result<()> {
    config.ai.api_key = secret_service::protect_secret(&config.ai.api_key);
    write_config(&config)
}
