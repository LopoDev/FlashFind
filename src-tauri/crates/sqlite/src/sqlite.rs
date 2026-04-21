use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::path::Path;

// app.dbパス取得
fn get_db_path_from_save_dir(save_dir: &Path) -> PathBuf {
    PathBuf::from(save_dir).join("app.db")
}

// ルートパス保存
pub fn regist_root(root_path: &str, save_dir: &Path) -> Result<()> {

    let create_root_folder_sql =
        "CREATE TABLE IF NOT EXISTS root_folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            created_at DateTime DEFAULT CURRENT_TIMESTAMP
        )
        ";

    let insert_root_folder_sql = "INSERT OR IGNORE INTO root_folders (path) VALUES (?1)";

    let mut conn = Connection::open(get_db_path_from_save_dir(save_dir))?;

    // トランザクション開始
    let tx = conn.transaction()?;

    // ルートフォルダーテーブル作成
    tx.execute(&create_root_folder_sql, [])?;

    // トランザクションに対してINSERT
    tx.execute(&insert_root_folder_sql, [root_path])?;

    // コミット
    tx.commit()?;

    Ok(())
}

// フォルダー保存
pub fn mark_as_indexed(dir_path: &str, new_or_modified: &[&str], save_dir: &Path) -> Result<()> {
    let create_folders_sql =
        "CREATE TABLE IF NOT EXISTS folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            root_dir TEXT NOT NULL,
            path TEXT NOT NULL UNIQUE,
            updated_at DateTime,
            created_at DateTime DEFAULT CURRENT_TIMESTAMP
        )";

    let insert_folders_sql = "INSERT OR REPLACE INTO folders (root_dir, path, updated_at) VALUES (?1, ?2, ?3)";

    let mut conn = Connection::open(get_db_path_from_save_dir(save_dir))?;

    // トランザクション開始
    let tx = conn.transaction()?;

    // フォルダーテーブル作成
    tx.execute(&create_folders_sql, [])?;

    // 現在の時刻を保持
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

    // トランザクションに対してINSERT
    for path in new_or_modified {
        tx.execute(&insert_folders_sql, [dir_path, path, &now])?;
    }

    // コミット
    tx.commit()?;

    Ok(())
}

// ファイル削除
pub fn delete_file(delete_path: &[&str], save_dir: &Path) -> Result<()> {
    let delete_folders_sql = "DELETE FROM folders WHERE path = ?1";

    let mut conn = Connection::open(get_db_path_from_save_dir(save_dir))?;

    // トランザクション開始
    let tx = conn.transaction()?;

    // トランザクションに対してDELETE
    for path in delete_path {
        tx.execute(&delete_folders_sql, [path])?;
    }

    // コミット
    tx.commit()?;

    Ok(())
}

// file走査した際に返す型
#[derive(Debug)]
pub struct FileResult {
    pub path: String,
    pub updated_at: String,
}

// ルート基準で登録されているフォルダ一覧を取得
pub fn get_files_by_root(root_path: &str, save_dir: &Path) -> Result<Vec<FileResult>> {
    let select_folders_sql = "SELECT path, updated_at FROM folders WHERE root_dir LIKE ?1";

    let conn = Connection::open(get_db_path_from_save_dir(save_dir))?;

    // 初回でテーブルがないことはあるので確認
    let exists: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM sqlite_master
            WHERE type='table' AND name='folders'
        )",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(vec![]);
    }

    let mut stmt = conn.prepare(select_folders_sql)?;
    let vec = stmt.query_map([format!("{}%", root_path)], |row| {
        let path: String = row.get(0)?;
        let updated_at: String = row.get(1)?;

        println!("path = {}, updated_at = {}", path, updated_at);
        Ok(
            FileResult {
                path: row.get(0)?,
                updated_at: row.get(1)?,
            }
        )
    })?
    .collect::<Result<Vec<FileResult>>>()?;

    Ok(vec)
}

// 登録済みルートディレクトリ一覧を取得
pub fn get_roots(save_dir: &Path) -> Result<Vec<String>> {
    let conn = Connection::open(get_db_path_from_save_dir(save_dir))?;

    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='root_folders')",
        [],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(vec![]);
    }

    let mut stmt = conn.prepare("SELECT path FROM root_folders ORDER BY created_at")?;
    let vec = stmt.query_map([], |row| row.get(0))?
        .collect::<Result<Vec<String>>>()?;
    Ok(vec)
}


/// ルートディレクトリを削除する（root_folders + folders の両テーブルから）
pub fn delete_root(root_path: &str, save_dir: &Path) -> Result<()> {
    let mut conn = Connection::open(get_db_path_from_save_dir(save_dir))?;
    let tx = conn.transaction()?;
    tx.execute("DELETE FROM root_folders WHERE path = ?1", [root_path])?;
    tx.execute("DELETE FROM folders WHERE root_dir = ?1", [root_path])?;
    tx.commit()?;
    Ok(())
}
