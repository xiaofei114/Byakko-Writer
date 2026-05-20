use std::fs;
use std::path::PathBuf;
use crate::models::AppConfig;

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

/// 加载配置
pub fn load_config() -> anyhow::Result<AppConfig> {
    let config_path = get_config_path()?;
    
    if config_path.exists() {
        let content = fs::read_to_string(&config_path)?;
        let config: AppConfig = serde_json::from_str(&content)?;
        Ok(config)
    } else {
        Ok(AppConfig::default())
    }
}

/// 保存配置
pub fn save_config(config: AppConfig) -> anyhow::Result<()> {
    let config_path = get_config_path()?;
    
    // 确保目录存在
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }
    
    let content = serde_json::to_string_pretty(&config)?;
    fs::write(&config_path, content)?;
    
    Ok(())
}
