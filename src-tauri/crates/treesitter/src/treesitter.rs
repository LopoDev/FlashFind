//! # treesitter モジュール
//!
//! ソースコードを tree-sitter でパースし、
//! 関数・クラスなどの定義を抽出する公開APIを提供する。
//!
//! ## 公開関数
//! - `parse_chunks`     : ソースコードをパースして定義ごとの Vec<String> を返す
//! - `is_supported_ext` : 対応している拡張子かどうかを返す

use anyhow::{anyhow, Result};
use tree_sitter::{Language, Parser};
use crate::extractor::{collect_nodes, target_node_kinds};

/// ファイル拡張子から tree-sitter の Language を取得する。
fn get_language(ext: &str) -> Option<Language> {
    match ext {
        "rs"                              => Some(tree_sitter_rust::language()),
        "py"                              => Some(tree_sitter_python::language()),
        "cpp" | "cc" | "c" | "h" | "hpp" => Some(tree_sitter_cpp::language()),
        "cs"                              => Some(tree_sitter_c_sharp::language()),
        _                                 => None,
    }
}

/// ソースコードをパースし、定義（関数・クラスなど）ごとに分割した Vec<String> を返す。
///
/// # なぜ Vec<String> を返すのか
/// Qdrant に「1定義 = 1ポイント」として保存することで、
/// ファイルが変更されたとき「消えた定義だけ削除・新しい定義だけ追加」という
/// チャンク粒度の差分管理が可能になる。
///
/// # フォールバック
/// 定義が1つも抽出できない場合は、ファイル全体を1チャンクとして返す。
pub fn parse_chunks(source: &str, ext: &str) -> Result<Vec<String>> {
    let language = get_language(ext)
        .ok_or_else(|| anyhow!("対応していない拡張子です: {}", ext))?;

    let mut parser = Parser::new();
    parser
        .set_language(&language)
        .map_err(|e| anyhow!("グラマーのセットに失敗しました: {}", e))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow!("パースに失敗しました"))?;

    let kinds = target_node_kinds(ext);
    let mut chunks: Vec<String> = Vec::new();
    collect_nodes(tree.root_node(), source, kinds, &mut chunks);

    if chunks.is_empty() {
        // 定義が抽出できなければファイル全体を1チャンクとして扱う
        Ok(vec![source.to_string()])
    } else {
        Ok(chunks)
    }
}

/// この拡張子が tree-sitter による解析対象かどうかを返す。
pub fn is_supported_ext(ext: &str) -> bool {
    get_language(ext).is_some()
}
