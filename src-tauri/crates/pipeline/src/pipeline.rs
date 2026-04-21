//! # pipeline クレート
//
use tauri::{AppHandle, Emitter, Manager};
use anyhow::{anyhow, Result};
use calamine::{open_workbook_auto, Data, Reader};
use qdrant::ParsedFile;
use qdrant::SearchResult;
use walkdir::WalkDir;
use std::fs;
use chrono::{DateTime, Local};
use sqlite::FileResult;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};
use std::collections::HashSet;

/// 現在インデックス中のディレクトリを追跡するグローバルロック。
/// 同一ディレクトリへの並行インデックス（起動時再インデックス + 手動再パース）を防ぐ。
static INDEXING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));
use std::sync::atomic::{AtomicUsize, Ordering};
use futures::future::join_all;
use serde::Serialize;


#[derive(Clone, Serialize)]
struct IndexProgress {
    dir_path: String,
    current: usize,
    total: usize,
}

#[tauri::command]
pub async fn index_directory(app: tauri::AppHandle, dir_path: &str) -> Result<(), String> {
    index_directory_impl(app, dir_path).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_directory(app: tauri::AppHandle, dir_path: &str) -> Result<(), String> {
    delete_directory_impl(app, dir_path).await.map_err(|e| e.to_string())
}

async fn delete_directory_impl(app: AppHandle, dir_path: &str) -> Result<()> {
    let db_path = get_db_path(&app)?;

    // SQLite からこのルート下の全ファイルパスを取得して Qdrant から削除
    let files = sqlite::get_files_by_root(dir_path, &db_path)?;
    let file_paths: Vec<String> = files.into_iter().map(|f| f.path).collect();
    qdrant::delete_by_file_paths(file_paths).await?;

    // SQLite から root_folders + folders を削除
    sqlite::delete_root(dir_path, &db_path).map_err(|e| anyhow!("{}", e))?;

    println!("[pipeline] ディレクトリ削除完了: {}", dir_path);
    Ok(())
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

async fn index_directory_impl(app: AppHandle, dir_path: &str) -> Result<()> {
    // 同一ディレクトリの並行インデックスを防ぐ
    {
        let mut set = INDEXING.lock().map_err(|_| anyhow!("インデックスロック取得失敗"))?;
        if !set.insert(dir_path.to_string()) {
            println!("[pipeline] {} は既にインデックス中のためスキップ", dir_path);
            app.emit("index_skipped", dir_path).ok();
            return Ok(());
        }
    }
    let result = run_index_directory(&app, dir_path).await;
    if let Ok(mut set) = INDEXING.lock() { set.remove(dir_path); }
    result
}

async fn run_index_directory(app: &AppHandle, dir_path: &str) -> Result<()> {
    println!("[pipeline] インデックス開始: {}", dir_path);

    let db_path = get_db_path(&app)?;
    sqlite::regist_root(dir_path, &db_path)?;

    let current_files  = scan_directory(dir_path)?;
    let recorded_files = sqlite::get_files_by_root(dir_path, &db_path)?;

    let recorded_map: HashMap<String, String> = recorded_files
        .into_iter().map(|f| (f.path, f.updated_at)).collect();
    let current_map: HashMap<String, String> = current_files
        .into_iter().map(|f| (f.path, f.updated_at)).collect();

    let deleted: Vec<String> = recorded_map.keys()
        .filter(|k| !current_map.contains_key(k.as_str()))
        .cloned().collect();

    let new_files: Vec<String> = current_map.keys()
        .filter(|k| !recorded_map.contains_key(k.as_str()))
        .cloned().collect();

    let modified_files: Vec<String> = current_map.iter()
        .filter(|(path, updated_at)| {
            recorded_map.get(*path).map_or(false, |r| r != *updated_at)
        })
        .map(|(path, _)| path.clone()).collect();

    println!(
        "[pipeline] 削除: {}件 / 新規: {}件 / 変更: {}件",
        deleted.len(), new_files.len(), modified_files.len()
    );

    qdrant::delete_by_file_paths(deleted.clone()).await?;
    sqlite::delete_file(
        &deleted.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &db_path,
    )?;

    let total = new_files.iter().filter(|p| is_excel_ext(p) || is_code_ext(p)).count()
            + modified_files.iter().filter(|p| is_excel_ext(p) || is_code_ext(p)).count();
    let counter = Arc::new(AtomicUsize::new(0));

    let new_parsed = parse_files(&app, dir_path, new_files.clone(), total, counter.clone()).await?;
    qdrant::save(new_parsed).await?;

    let modified_parsed = parse_files(&app, dir_path, modified_files.clone(), total, counter.clone()).await?;

    let mut chunks_by_file: HashMap<String, Vec<ParsedFile>> = HashMap::new();
    for chunk in modified_parsed {
        chunks_by_file.entry(chunk.path.clone()).or_default().push(chunk);
    }
    for (file_path, chunks) in chunks_by_file {
        qdrant::sync_file(&file_path, chunks).await?;
    }

    let all_processed: Vec<String> = new_files.into_iter().chain(modified_files).collect();
    sqlite::mark_as_indexed(
        dir_path,
        &all_processed.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
        &db_path,
    )?;

    println!("[pipeline] インデックス完了: {}", dir_path);
    Ok(())
}

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

async fn parse_excel_files(
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
            let path_clone = path.clone();
            let content = tokio::task::spawn_blocking(move || {
                parse_excel_with_calamine(&path_clone)
            }).await??;

            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
            app.emit("index_progress", IndexProgress { dir_path, current, total }).ok();

            Ok::<_, anyhow::Error>(ParsedFile { path, content })
        }
    }).collect();

    join_all(futures).await.into_iter().collect()
}

fn parse_excel_with_calamine(path: &str) -> Result<String> {
    let mut workbook = open_workbook_auto(path)
        .map_err(|e| anyhow::anyhow!("Excel 読み込み失敗 {}: {}", path, e))?;
    let mut text = String::new();

    for sheet_name in workbook.sheet_names().to_vec() {
        if let Ok(range) = workbook.worksheet_range(&sheet_name) {
            text.push_str(&format!("# {}
", sheet_name));
            for row in range.rows() {
                let cells: Vec<String> = row.iter()
                    .map(|cell| match cell {
                        Data::Empty => String::new(),
                        other       => other.to_string(),
                    })
                    .collect();
                if cells.iter().any(|c| !c.is_empty()) {
                    text.push_str(&cells.join("	"));
                    text.push('\n');
                }
            }
            text.push('\n');
        }
    }

    Ok(text)
}

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
            let path_clone = path.clone();
            let chunks = tokio::task::spawn_blocking(move || {
                let source = match fs::read_to_string(&path_clone) {
                    Ok(s)  => s,
                    Err(e) => {
                        eprintln!("[treesitter] 読み込み失敗 {}: {}", path_clone, e);
                        return Ok::<_, anyhow::Error>(vec![]);
                    }
                };
                let ext    = path_clone.split('.').last().unwrap_or("").to_string();
                let chunks = match treesitter::parse_chunks(&source, &ext) {
                    Ok(c)  => c,
                    Err(e) => {
                        eprintln!("[treesitter] パース失敗 {}: {}", path_clone, e);
                        vec![source]
                    }
                };
                Ok(chunks)
            }).await??;

            let current = counter.fetch_add(1, Ordering::SeqCst) + 1;
            app.emit("index_progress", IndexProgress { dir_path, current, total }).ok();

            Ok::<_, anyhow::Error>(
                chunks.into_iter()
                    .map(|content| ParsedFile { path: path.clone(), content })
                    .collect::<Vec<_>>()
            )
        }
    }).collect();

    let results: Result<Vec<Vec<ParsedFile>>> = join_all(futures).await.into_iter().collect();
    Ok(results?.into_iter().flatten().collect())
}

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

pub async fn reindex_saved_dirs(app: AppHandle) -> Result<()> {
    let db_path = get_db_path(&app)?;

    let dirs = sqlite::get_roots(&db_path)
        .map_err(|e| anyhow!("ルートディレクトリ取得失敗: {}", e))?;

    println!("[pipeline] 起動時再インデックス: {} ディレクトリ", dirs.len());

    for dir in dirs {
        println!("[pipeline] 再インデックス開始: {}", dir);
        if let Err(e) = index_directory_impl(app.clone(), &dir).await {
            eprintln!("[pipeline] 再インデックス失敗 {}: {}", dir, e);
        }
    }

    Ok(())
}
