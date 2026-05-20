use crate::models::AppConfig;
use crate::services::config_service;

/// 加载配置
#[tauri::command]
pub fn load_config() -> Result<AppConfig, String> {
    config_service::load_config().map_err(|e| e.to_string())
}

/// 保存配置
#[tauri::command]
pub fn save_config(config: AppConfig) -> Result<(), String> {
    config_service::save_config(config).map_err(|e| e.to_string())
}
