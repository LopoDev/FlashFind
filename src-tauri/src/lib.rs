#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

use tauri::Manager;
use tauri_plugin_shell::ShellExt;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let db_path  = pipeline::get_db_path(&app.handle()).expect("db_path 取得失敗");
            let qdrant_dir = db_path.join("qdrant");
            std::fs::create_dir_all(&qdrant_dir).expect("qdrant ディレクトリ作成失敗");

            // Qdrant サイドカーを起動
            match app.shell().sidecar("qdrant") {
                Ok(cmd) => {
                    let cmd = cmd
                        .current_dir(&qdrant_dir)
                        .env("QDRANT__STORAGE__PATH",          qdrant_dir.join("storage").to_str().unwrap())
                        .env("QDRANT__STORAGE__SNAPSHOTS_PATH", qdrant_dir.join("snapshots").to_str().unwrap());

                    match cmd.spawn() {
                        Ok(_) => println!("[setup] Qdrant 起動成功"),
                        Err(e) => eprintln!("[setup] Qdrant 起動失敗: {:?}", e),
                    }
                },
                Err(e) => eprintln!("[setup] Qdrant サイドカーが見つかりません: {:?}", e),
            }

            // Qdrant の準備が整ってから、起動時の再インデックスを非同期で実行する。
            // アプリが閉じている間に変更されたファイルをここで取り込む。
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // Qdrant の起動を待つ（gRPC サーバーが立ち上がるまで数秒かかる）
                tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;
                println!("[setup] 起動時再インデックス開始");
                if let Err(e) = pipeline::reindex_saved_dirs(app_handle).await {
                    eprintln!("[setup] 起動時再インデックス失敗: {}", e);
                }
            });

            #[cfg(debug_assertions)]
            { let window = app.get_webview_window("main").unwrap(); window.open_devtools(); }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            pipeline::index_directory,
            pipeline::delete_directory,
            pipeline::search,
            pipeline::get_directories,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
