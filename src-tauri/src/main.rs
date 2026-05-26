// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod models;
mod services;
mod commands;
mod db;

use commands::{
    // 书籍相关命令
    create_book, get_books_list, load_book, save_book, delete_book,
    create_volume, delete_volume,
    create_chapter, delete_chapter,
    load_chapter_content, save_chapter_content, update_chapter_title,
    // AI 相关命令
    generate_chapter_summary, load_chapter_summary,
    send_chat_message_stream, get_chat_history, get_chat_sessions, delete_chat_session, update_message_polish_handled, send_polish_request,
    // 快照相关命令
    create_chapter_snapshot, list_chapter_snapshots, get_chapter_snapshot,
    delete_chapter_snapshot, cleanup_chapter_snapshots, compare_snapshots,
    // 角色卡相关命令
    create_character_card, list_character_cards, get_character_card,
    update_character_card, delete_character_card, search_character_cards,
    // 大纲相关命令
    save_outline, get_outline, get_outline_by_level, list_book_outlines,
    list_volume_outlines, list_chapter_outlines, delete_outline, get_outline_stats,
    // 配置相关命令
    load_config, save_config,
    // 写作风格相关命令
    learn_writing_style, get_style_prompt, update_style_config,
    get_user_writing_style, get_book_writing_style, check_should_learn_style,
    trigger_style_analysis,
    // 冲突检测相关命令
    run_conflict_detection, ignore_conflict, get_active_conflicts,
    // 故事记忆相关命令
    get_story_memory, update_story_memory, get_story_memory_text,
};

fn main() {
    // 初始化日志 - 设置默认日志级别为 info
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .init();
    log::info!("应用启动");
    
    // 初始化数据库
    tauri::async_runtime::block_on(async {
        if let Err(e) = db::init_db().await {
            log::error!("数据库初始化失败: {}", e);
        }
    });
    
    // 启动后台自动风格分析服务
    tauri::async_runtime::spawn(async {
        services::auto_style_service::start_auto_style_service().await;
    });

    // 启动后台故事记忆自动更新服务
    tauri::async_runtime::spawn(async {
        services::auto_story_memory::start_auto_story_memory_service().await;
    });
    
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            // 书籍
            create_book,
            get_books_list,
            load_book,
            save_book,
            delete_book,
            // 卷
            create_volume,
            delete_volume,
            // 章节
            create_chapter,
            delete_chapter,
            // 章节内容
            load_chapter_content,
            save_chapter_content,
            update_chapter_title,
            // 配置
            load_config,
            save_config,
            // AI 摘要
            generate_chapter_summary,
            load_chapter_summary,
            // AI 聊天
            send_chat_message_stream,
            get_chat_history,
            get_chat_sessions,
            delete_chat_session,
            update_message_polish_handled,
            send_polish_request,
            // 章节快照
            create_chapter_snapshot,
            list_chapter_snapshots,
            get_chapter_snapshot,
            delete_chapter_snapshot,
            cleanup_chapter_snapshots,
            compare_snapshots,
            // 角色卡
            create_character_card,
            list_character_cards,
            get_character_card,
            update_character_card,
            delete_character_card,
            search_character_cards,
            // 大纲
            save_outline,
            get_outline,
            get_outline_by_level,
            list_book_outlines,
            list_volume_outlines,
            list_chapter_outlines,
            delete_outline,
            get_outline_stats,
            // 写作风格
            learn_writing_style,
            get_style_prompt,
            update_style_config,
            get_user_writing_style,
            get_book_writing_style,
            check_should_learn_style,
            trigger_style_analysis,
            // 冲突检测
            run_conflict_detection,
            ignore_conflict,
            get_active_conflicts,
            // 故事记忆
            get_story_memory,
            update_story_memory,
            get_story_memory_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
