use anyhow::{anyhow, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, DeletePointsBuilder, Distance,
    Filter, PointStruct, ScrollPointsBuilder, UpsertPointsBuilder,
    VectorParamsBuilder, QueryPointsBuilder, PointsIdsList, PointId,
};
use qdrant_client::{Payload, Qdrant};
use std::collections::{HashMap, HashSet};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{LazyLock, Mutex};
use serde_json;
use serde::Serialize;

// ──────────────────────────────────────────────
// 定数
// ──────────────────────────────────────────────

const EMBEDDING_MODEL: EmbeddingModel = EmbeddingModel::MultilingualE5Small;
const VECTOR_SIZE: u64 = 384;
const COLLECTION_NAME: &str = "files";
const CHUNK_SIZE: usize = 64;

// ──────────────────────────────────────────────
// モデルシングルトン
// ──────────────────────────────────────────────

/// 埋め込みモデルをアプリ起動後に一度だけ初期化して使い回す。
/// Mutex で包むのは embed() が &mut self を要求するため。
/// ロックは embed() 中のみ保持し、await の前に必ず解放する。
static MODEL: LazyLock<Mutex<TextEmbedding>> = LazyLock::new(|| {
    Mutex::new(
        TextEmbedding::try_new(
            InitOptions::new(EMBEDDING_MODEL).with_show_download_progress(true),
        ).expect("埋め込みモデルの初期化に失敗しました")
    )
});

// ──────────────────────────────────────────────
// 公開型
// ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub file: String,
    pub content: String,
    pub score: f32,
}

/// pipeline クレートから受け取るパース済みチャンク。
/// 1ファイルから複数の ParsedFile が生成されることがある（1定義 = 1チャンク）。
pub struct ParsedFile {
    pub path: String,
    pub content: String,
}

// ──────────────────────────────────────────────
// 内部ユーティリティ
// ──────────────────────────────────────────────

fn make_client() -> Result<Qdrant> {
    Qdrant::from_url("http://localhost:6334")
        .build()
        .map_err(|e| anyhow!("{:?}", e))
}

/// チャンクの Qdrant ポイント ID。
/// hash(path + content) にすることでコンテンツアドレッシングになる。
/// - 同じ内容 → 同じ ID → upsert が冪等（再埋め込みなし）
/// - 内容変更 → 異なる ID → 古い ID を削除、新しい ID を挿入
fn chunk_to_id(path: &str, content: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    path.hash(&mut hasher);
    content.hash(&mut hasher);
    hasher.finish()
}

/// ファイルパスから拡張子を取得する。
fn get_ext(path: &str) -> String {
    path.split('.').last().unwrap_or("").to_lowercase()
}

async fn ensure_collection(client: &Qdrant) -> Result<()> {
    if !client.collection_exists(COLLECTION_NAME).await
        .map_err(|e| anyhow!("{:?}", e))?
    {
        client.create_collection(
            CreateCollectionBuilder::new(COLLECTION_NAME)
                .vectors_config(VectorParamsBuilder::new(VECTOR_SIZE, Distance::Cosine)),
        ).await.map_err(|e| anyhow!("{:?}", e))?;
    }
    Ok(())
}

/// 指定ファイルパスの既存チャンク ID を全件取得する（差分計算に使う）。
async fn scroll_chunk_ids_for_file(
    client: &Qdrant,
    file_path: &str,
) -> Result<HashSet<u64>> {
    let response = client.scroll(
        ScrollPointsBuilder::new(COLLECTION_NAME)
            .filter(Filter::must([Condition::matches("file", file_path.to_string())]))
            .limit(100_000u32)
            .with_payload(false)
            .with_vectors(false),
    ).await.map_err(|e| anyhow!("scroll 失敗: {:?}", e))?;

    let ids: HashSet<u64> = response.result
        .into_iter()
        .filter_map(|point| {
            use qdrant_client::qdrant::point_id::PointIdOptions;
            point.id?.point_id_options.and_then(|opt| {
                if let PointIdOptions::Num(n) = opt { Some(n) } else { None }
            })
        })
        .collect();

    Ok(ids)
}

/// チャンクを embed して Qdrant に upsert する。
/// CHUNK_SIZE 件ずつ処理してメモリを抑える。
/// payload に `ext` フィールドを含めることで、検索時に拡張子フィルターが使える。
async fn embed_and_upsert(client: &Qdrant, chunks: Vec<&ParsedFile>) -> Result<()> {
    for batch in chunks.chunks(CHUNK_SIZE) {
        let texts: Vec<String> = batch.iter().map(|f| f.content.clone()).collect();

        // embed() はブロッキングなので Mutex ロックは await 前に解放する
        let embeddings = {
            let mut model = MODEL.lock()
                .map_err(|_| anyhow!("モデルのロック取得に失敗"))?;
            model.embed(texts, None)
                .map_err(|e| anyhow!("embedding 失敗: {:?}", e))?
        };

        let mut points: Vec<PointStruct> = Vec::new();
        for (file, vector) in batch.iter().zip(embeddings) {
            let ext = get_ext(&file.path);
            let payload: Payload = serde_json::json!({
                "file":    file.path,
                "content": file.content,
                "ext":     ext,          // 拡張子フィルター用
            }).try_into().map_err(|e| anyhow!("{:?}", e))?;
            points.push(PointStruct::new(chunk_to_id(&file.path, &file.content), vector, payload));
        }

        client.upsert_points(
            UpsertPointsBuilder::new(COLLECTION_NAME, points)
        ).await.map_err(|e| anyhow!("upsert 失敗: {:?}", e))?;
    }
    Ok(())
}

// ──────────────────────────────────────────────
// 公開関数
// ──────────────────────────────────────────────

/// 新規ファイルのチャンクを保存する（差分チェックなし）。
pub async fn save(files: Vec<ParsedFile>) -> Result<()> {
    if files.is_empty() { return Ok(()); }
    let client = make_client()?;
    ensure_collection(&client).await?;
    embed_and_upsert(&client, files.iter().collect()).await?;
    Ok(())
}

/// 変更ファイルを差分ベースで同期する。
///
/// 1. 既存チャンク ID を Qdrant から取得
/// 2. 新チャンク ID を計算
/// 3. 旧にあって新にない → 削除（消えた定義）
/// 4. 新にあって旧にない → embed して追加（新しい定義）
/// 5. 両方にある → スキップ（変更なし）
pub async fn sync_file(file_path: &str, new_chunks: Vec<ParsedFile>) -> Result<()> {
    let client = make_client()?;
    ensure_collection(&client).await?;

    let existing_ids = scroll_chunk_ids_for_file(&client, file_path).await?;

    let new_chunk_map: HashMap<u64, &ParsedFile> = new_chunks.iter()
        .map(|f| (chunk_to_id(&f.path, &f.content), f))
        .collect();
    let new_ids: HashSet<u64> = new_chunk_map.keys().cloned().collect();

    // 削除: 旧にあって新にないチャンク（ファイルから消えた定義）
    let to_delete: Vec<PointId> = existing_ids.difference(&new_ids)
        .map(|&id| id.into())
        .collect();
    if !to_delete.is_empty() {
        println!("[qdrant] 削除チャンク: {} 件 ({})", to_delete.len(), file_path);
        client.delete_points(
            DeletePointsBuilder::new(COLLECTION_NAME)
                .points(PointsIdsList { ids: to_delete })
        ).await.map_err(|e| anyhow!("チャンク削除失敗: {:?}", e))?;
    }

    // 追加: 新にあって旧にないチャンク（新しく追加された定義）
    let to_insert: Vec<&ParsedFile> = new_ids.difference(&existing_ids)
        .filter_map(|id| new_chunk_map.get(id).copied())
        .collect();
    if !to_insert.is_empty() {
        println!("[qdrant] 追加チャンク: {} 件 ({})", to_insert.len(), file_path);
        embed_and_upsert(&client, to_insert).await?;
    }

    Ok(())
}

/// ファイルが削除されたとき、そのファイルの全チャンクを Qdrant から消す。
pub async fn delete_by_file_paths(file_paths: Vec<String>) -> Result<()> {
    if file_paths.is_empty() { return Ok(()); }
    let client = make_client()?;
    if !client.collection_exists(COLLECTION_NAME).await.map_err(|e| anyhow!("{:?}", e))? {
        return Ok(());
    }
    for path in &file_paths {
        let ids = scroll_chunk_ids_for_file(&client, path).await?;
        if ids.is_empty() { continue; }
        let point_ids: Vec<PointId> = ids.into_iter().map(|id| id.into()).collect();
        client.delete_points(
            DeletePointsBuilder::new(COLLECTION_NAME)
                .points(PointsIdsList { ids: point_ids })
        ).await.map_err(|e| anyhow!("削除失敗 {}: {:?}", path, e))?;
    }
    Ok(())
}

/// ベクトル検索。
///
/// `extensions` が空でなければ、payload の `ext` フィールドでフィルタリングする。
/// 例: extensions = ["rs", "py"] → Rust と Python のチャンクだけが候補になる。
pub async fn search(query: &str, extensions: Vec<String>) -> Result<Vec<SearchResult>> {
    let client = make_client()?;
    if !client.collection_exists(COLLECTION_NAME).await.map_err(|e| anyhow!("{:?}", e))? {
        return Ok(vec![]);
    }

    let query_vector = {
        let mut model = MODEL.lock().map_err(|_| anyhow!("モデルのロック取得に失敗"))?;
        model.embed(vec![query.to_string()], None)
            .map_err(|e| anyhow!("embedding 失敗: {:?}", e))?
            .into_iter()
            .next()
            .ok_or_else(|| anyhow!("embedding 結果が空です"))?
    };

    // 拡張子フィルター: 選択された拡張子のいずれかに一致するチャンクだけを検索
    // extensions が空の場合はフィルターなし（全拡張子が対象）
    let mut builder = QueryPointsBuilder::new(COLLECTION_NAME)
        .query(query_vector)
        .limit(10)
        .with_payload(true);

    if !extensions.is_empty() {
        // should = OR 条件（どれか1つに一致すれば通る）
        let filter = Filter::should(
            extensions.iter()
                .map(|ext| Condition::matches("ext", ext.clone()))
                .collect::<Vec<_>>()
        );
        builder = builder.filter(filter);
    }

    let results = client.query(builder)
        .await.map_err(|e| anyhow!("検索失敗: {:?}", e))?;

    let hits = results.result
        .into_iter()
        .filter_map(|point| {
            let payload = point.payload;
            let file    = payload.get("file")?.as_str()?.to_string();
            let content = payload.get("content")?.as_str()?.to_string();
            Some(SearchResult { file, content, score: point.score })
        })
        .collect();

    Ok(hits)
}
