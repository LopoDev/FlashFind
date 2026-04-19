//! # treesitter クレート
//!
//! ## モジュール構成
//! - `treesitter` : 公開API（parse_chunks, is_supported_ext）
//! - `extractor`  : 内部ロジック（AST走査・ノード抽出）

mod extractor;
mod treesitter;

pub use crate::treesitter::*;
