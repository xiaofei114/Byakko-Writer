use crate::db::get_pool;
use tokio::time::{interval, Duration};

const CHECK_INTERVAL_MINUTES: u64 = 30;

/// 启动后台故事记忆自动更新服务
pub async fn start_auto_story_memory_service() {
    log::info!("[Auto StoryMemory] 启动后台故事记忆更新服务");

    let mut ticker = interval(Duration::from_secs(CHECK_INTERVAL_MINUTES * 60));

    // 首次延迟 2 分钟
    tokio::time::sleep(Duration::from_secs(120)).await;

    loop {
        ticker.tick().await;

        if let Err(e) = check_and_update_all_books().await {
            log::error!("[Auto StoryMemory] 自动更新失败: {}", e);
        }
    }
}

async fn check_and_update_all_books() -> anyhow::Result<()> {
    // 尊重设置中的「自动更新故事记忆」开关，未开启时直接跳过
    match crate::services::config_service::load_config() {
        Ok(config) => {
            if !config.auto_story_memory {
                log::debug!("[Auto StoryMemory] 自动更新已在设置中关闭，跳过本轮检查");
                return Ok(());
            }
            if config.ai.api_key.is_empty() {
                log::debug!("[Auto StoryMemory] 未配置 API Key，跳过本轮检查");
                return Ok(());
            }
        }
        Err(e) => {
            log::warn!("[Auto StoryMemory] 读取配置失败，跳过本轮检查: {}", e);
            return Ok(());
        }
    }

    let pool = get_pool().await?;

    let books = sqlx::query_scalar::<_, String>("SELECT id FROM books")
        .fetch_all(pool)
        .await?;

    for book_id in books {
        match crate::services::story_memory_service::check_should_update_story_memory(&book_id).await {
            Ok(true) => {
                log::info!("[Auto StoryMemory] 书籍 {} 的故事记忆需要更新", &book_id[..book_id.len().min(8)]);
                // 后台更新不传 AppHandle（无法从前端获取反馈）
                match crate::services::story_memory_service::update_story_memory_auto(&book_id).await {
                    Ok(result) => {
                        if result.success {
                            log::info!("[Auto StoryMemory] 书籍 {} 故事记忆更新成功: {}",
                                &book_id[..book_id.len().min(8)], result.message);
                        } else {
                            log::warn!("[Auto StoryMemory] 书籍 {} 故事记忆更新失败: {}",
                                &book_id[..book_id.len().min(8)], result.message);
                        }
                    }
                    Err(e) => {
                        log::error!("[Auto StoryMemory] 书籍 {} 故事记忆更新出错: {}", &book_id[..book_id.len().min(8)], e);
                    }
                }
            }
            Ok(false) => {}
            Err(e) => {
                log::error!("[Auto StoryMemory] 检查书籍 {} 时出错: {}", &book_id[..book_id.len().min(8)], e);
            }
        }
    }

    Ok(())
}
