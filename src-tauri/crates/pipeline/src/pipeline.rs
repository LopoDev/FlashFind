//! # pipeline クレート
//!
//! ## インデックス作成の流れ
//! 1. ディレクトリを走査して現在のファイル一覧を取得
//! 2. SQLite の前回一覧と比較して new/modified/deleted に分類
//! 3. 削除ファイル → Qdrant から全チャンクをフィルター削除・SQLite から削除
//! 4. 新規ファイル → パース → Qdrant に全チャンク upsert
//! 5. 変更ファイル → パース → Qdrant で差分同期（消えた定義を削除・新定義を追加）
//! 6. SQLite に記録

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::ShellExt;
use anyhow::{anyhow, Result};
use qdrant::ParsedFile;
use qdrant::SearchResult;
use walkdir::WalkDir;
use std::fs;
use chrono::{DateTime, Local};
use sqlite::FileResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::future::join_all;
use tokio::sync::Semaphore;
use serde::Serialize;

// ──────────────────────────────────────────────
// 定数
// ──────────────────────────────────────────────

/// markitdown サイドカーの最大同時起動数。
/// 全件を一気に起動すると数千の Python プロセスが走り OOM になる。
const MAX_CONCURRENT_SIDECARS: usize = 8;

// ──────────────────────────────────────────────
// イベントペイロード
// ──────────────────────────────────────────────

#[derive(Clone, Serialize)]
struct IndexProgress {
    dir_path: String,
    current: usize,
    total: usize,
}

// ──────────────────────────────────────────────
// Tauri コマンド
// ──────────────────────────────────────────────

#[tauri::command]
pub async fn index_directory(app: tauri::AppHandle, dir_path: &str) -> Result<(), String> {
    index_directory_impl(app, dir_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search(query: &str, extensions: Vec<String>) -> Result<Vec<SearchResult>, String> {
    qdrant::search(query, extensions).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_directories(app: AppHandle) -> Result<Vec<String>, String> {
    let db_path = get_db_path(&app).map_err(|e| e.to_string())?;
    sqlite::get_roots(&db_path).map_err(|e| e.to_string())
}

// ──────────────────────────────────────────────
// インデックス作成
// ──────────────────────────────────────────────

async fn index_directory_impl(app: AppHandle, dir_path: &str) -> Result<()> {
    println!("[pipeline] インデックス開始: {}", dir_path);

    let db_path = get_db_path(&app)?;
    sqlite::regist_root(dir_path, &db_path)?;

    // ──── ファイル差分の検出 ────

    let current_files  = scan_directory(dir_path)?;
    let recorded_files = sqlite::get_files_by_root(dir_path, &db_path)?;

    let recorded_map: HashMap<String, String> = recorded_files
        .into_iter().map(|f| (f.path, f.updated_at)).collect();
    let current_map: HashMap<String, String> = current_files
        .into_iter().map(|f| (f.path, f.updated_at)).collect();

    // 削除: SQLiteにあってディスクにないもの
    let deleted: Vec<String> = recorded_map.keys()
        .filter(|k| !current_map.contains_key(k.as_str()))
        .cloned().collect();

    // 新規: ディスクにあってSQLiteにないもの
    let new_files: Vec<String> = current_map.keys()
        .filter(|k| !recorded_map.contains_key(k.as_str()))
        .cloned().collect();

    // 変更: 両方にあるが更新日時が違うもの
    let modified_files: Vec<String> = current_map.iter()
        .filter(|(path, updated_at)| {
            recorded_map.get(*path).map_or(false, |r| r != *updated_at)
        })
        .map(|(path, _)| path.clone()).collect();

    println!(
        "[pipeline] 削除: {}件 / 新規: {}件 / 変更: {}件",
        deleted.len(), new_files.len(), modified_files.len()
    );

    // ──── 削除処理 ────
    // ファイル単位でフィルター削除（1ファイルに複数チャンクがあっても全て消える）
    qdrant::delete_by_file_paths(deleted.clone()).await?;
    sqlite::delete_file(
        &deleted.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &db_path,
    )?;

    // ──── 進捗カウンター ────
    let total   = new_files.len() + modified_files.len();
    let counter = Arc::new(AtomicUsize::new(0));

    // ──── 新規ファイル: 全チャンクを保存 ────
    let new_parsed = parse_files(&app, dir_path, new_files.clone(), total, counter.clone()).await?;
    qdrant::save(new_parsed).await?;

    // ──── 変更ファイル: 差分同期 ────
    // ファイルごとに「消えた定義を削除・新しい定義を追加」する
    let modified_parsed = parse_files(&app, dir_path, modified_files.clone(), total, counter.clone()).await?;

    // ファイルパスでグループ化して1ファイルずつ sync_file を呼ぶ
    let mut chunks_by_file: HashMap<String, Vec<ParsedFile>> = HashMap::new();
    for chunk in modified_parsed {
        chunks_by_file.entry(chunk.path.clone()).or_default().push(chunk);
    }
    for (file_path, chunks) in chunks_by_file {
        qdrant::sync_file(&file_path, chunks).await?;
    }

    // ──── SQLite に記録 ────
    let all_processed: Vec<String> = new_files.into_iter().chain(modified_files).collect();
    sqlite::mark_as_indexed(
        dir_path,
        &all_processed.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &db_path,
    )?;

    println!("[pipeline] インデックス完了: {}", dir_path);
    Ok(())
}

// ──────────────────────────────────────────────
// パース処理（拡張子で振り分け）
// ──────────────────────────────────────────────

/// ファイルリストを拡張子で振り分けてパースし、チャンクの Vec を返す。
/// - Excel → markitdown サイドカー（1ファイル = 1チャンク）
/// - コード → tree-sitter（1ファイル = N チャンク、定義ごと）
async fn parse_files(
    app: &AppHandle,
    dir_path: &str,
    paths: Vec<String>,
    total: usize,
    counter: Arc<AtomicUsize>,
) -> Result<Vec<ParsedFile>> {
    let excel_paths: Vec<String> = paths.iter().filter(|p| is_excel_ext(p)).cloned().collect();
    let code_paths:  Vec<String> = paths.iter().filter(|p| is_code_ext(p) ).cloned().collect();

    let mut result = parse_excel_files(app, dir_path, excel_paths, total, counter.clone()).await?;
    result.extend(parse_code_files(app, dir_path, code_paths, total, counter).await?);
    Ok(result)
}

fn is_excel_ext(path: &str) -> bool {
    ["xlsx", "xls", "xlsm", "xlsb"].contains(&path.split('.').last().unwrap_or(""))
}

fn is_code_ext(path: &str) -> bool {
    treesitter::is_supported_ext(path.split('.').last().unwrap_or(""))
}

// ──────────────────────────────────────────────
// Excel パース（markitdown + セマフォ）
// ──────────────────────────────────────────────

async fn parse_excel_files(
    app: &AppHandle,
    dir_path: &str,
    paths: Vec<String>,
    total: usize,
    counter: Arc<AtomicUsize>,
) -> Result<Vec<ParsedFile>> {
    // セマフォで同時起動数を制限（全件一気に起動すると OOM になる）
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENT_SIDECARS));

    let futures: Vec<_> = paths.into_iter().map(|path| {
        let app       = app.clone();
        let semaphore = semaphore.clone();
        let counter   = counter.clone();
        let dir_path  = dir_path.to_string();

        async move {
            let _permit: tokio::sync::SemaphorePermit = semaphore.acquire().await
                .map_err(|e| anyhow!("セマフォ取得失敗: {}", e))?;

            let output = app.shell()
                .sidecar("markitdown_sidecar")?
                .args([&path])
                .output()
                .await?;

            // _permit ドロップ → 次のタスクが実行可能に

            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
            app.emit("index_progress", IndexProgress { dir_path: dir_path.clone(), current, total }).ok();

            let content = String::from_utf8(output.stdout)?;
            // Excel は1ファイル = 1チャンク
            Ok::<_, anyhow::Error>(ParsedFile { path, content })
        }
    }).collect();

    join_all(futures).await.into_iter().collect()
}

// ──────────────────────────────────────────────
// コードファイルパース（tree-sitter）
// ──────────────────────────────────────────────

/// コードファイルを tree-sitter でパースし、定義ごとのチャンクを返す。
///
/// 1ファイルから複数の ParsedFile が生成される（1関数/クラス = 1チャンク）。
/// これにより Qdrant での差分管理が関数単位で行える。
async fn parse_code_files(
    app: &AppHandle,
    dir_path: &str,
    paths: Vec<String>,
    total: usize,
    counter: Arc<AtomicUsize>,
) -> Result<Vec<ParsedFile>> {
    let futures: Vec<_> = paths.into_iter().map(|path| {
        let app      = app.clone();
        let counter  = counter.clone();
        let dir_path = dir_path.to_string();

        async move {
            let source = match fs::read_to_string(&path) {
                Ok(s)  => s,
                Err(e) => {
                    eprintln!("[treesitter] 読み込み失敗 {}: {}", path, e);
                    return Ok::<_, anyhow::Error>(vec![]);
                }
            };

            let ext = path.split('.').last().unwrap_or("");

            // parse_chunks は定義ごとに分割した Vec<String> を返す
            let chunks = match treesitter::parse_chunks(&source, ext) {
                Ok(c)  => c,
                Err(e) => {
                    eprintln!("[treesitter] パース失敗 {}: {}", path, e);
                    vec![source] // フォールバック: ファイル全体を1チャンクとして扱う
                }
            };

            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
            app.emit("index_progress", IndexProgress { dir_path: dir_path.clone(), current, total }).ok();

            // 同じ path に複数チャンクを持つ ParsedFile のリストを返す
            Ok(chunks.into_iter().map(|content| ParsedFile { path: path.clone(), content }).collect())
        }
    }).collect();

    let results: Result<Vec<Vec<ParsedFile>>> = join_all(futures).await.into_iter().collect();
    Ok(results?.into_iter().flatten().collect())
}

// ──────────────────────────────────────────────
// ユーティリティ
// ──────────────────────────────────────────────

pub fn get_db_path(app: &AppHandle) -> Result<PathBuf> {
    app.path()
        .app_data_dir()
        .map_err(|e| anyhow!("app_data_dir の取得に失敗: {:?}", e))
}

fn scan_directory(root_path: &str) -> Result<Vec<FileResult>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root_path) {
        let entry = match entry {
            Ok(e)    => e,
            Err(err) => { eprintln!("[scan] アクセスエラー: {}", err); continue; }
        };
        let path = entry.path();
        if !path.is_file() { continue; }
        let metadata = match fs::metadata(path) {
            Ok(m)    => m,
            Err(err) => { eprintln!("[scan] メタデータ取得失敗: {}", err); continue; }
        };
        let updated_at = metadata.modified()
            .map(|t| { let dt: DateTime<Local> = t.into(); dt.format("%Y-%m-%d %H:%M:%S").to_string() })
            .unwrap_or_else(|_| "1970-01-01 00:00:00".to_string());
        files.push(FileResult { path: path.to_string_lossy().into_owned(), updated_at });
    }
    Ok(files)
}

/// 保存済み全ディレクトリを再インデックスする。
/// アプリ起動時に Qdrant が準備できた後に呼ばれ、
/// アプリが閉じている間に変更されたファイルを取り込む。
pub async fn reindex_saved_dirs(app: AppHandle) -> Result<()> {
    let db_path = get_db_path(&app)?;

    // SQLite から前回登録されたルートディレクトリ一覧を取得
    let dirs = sqlite::get_roots(&db_path)
        .map_err(|e| anyhow!("ルートディレクトリ取得失敗: {}", e))?;

    println!("[pipeline] 起動時再インデックス: {} ディレクトリ", dirs.len());

    for dir in dirs {
        println!("[pipeline] 再インデックス開始: {}", dir);
        if let Err(e) = index_directory_impl(app.clone(), &dir).await {
            // 1ディレクトリが失敗しても他のディレクトリは続行する
            eprintln!("[pipeline] 再インデックス失敗 {}: {}", dir, e);
        }
    }

    Ok(())
}
