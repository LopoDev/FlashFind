//! # extractor モジュール
//!
//! tree-sitter のASTを走査して、意味のあるノード（関数・クラスなど）を
//! テキストとして抽出する内部ロジック。
//! このモジュールは treesitter.rs からのみ呼ばれる。

use tree_sitter::Node;

/// 拡張子に応じて「抽出したいASTノードの種類」のリストを返す。
///
/// tree-sitter のノード種別名（node.kind()）は言語グラマーで定義されている。
/// ここでは関数・クラス・構造体など「意味の塊」になるノードを指定する。
pub fn target_node_kinds(ext: &str) -> &'static [&'static str] {
    match ext {
        "rs" => &[
            "function_item",  // fn foo() { ... }
            "impl_item",      // impl Foo { ... }
            "struct_item",    // struct Foo { ... }
            "enum_item",      // enum Foo { ... }
            "trait_item",     // trait Foo { ... }
        ],
        "py" => &[
            "function_definition",  // def foo(): ...
            "class_definition",     // class Foo: ...
        ],
        "cpp" | "cc" | "c" | "h" | "hpp" => &[
            "function_definition",  // void foo() { ... }
            "class_specifier",      // class Foo { ... }
            "struct_specifier",     // struct Foo { ... }
        ],
        "cs" => &[
            "method_declaration",    // void Foo() { ... }
            "class_declaration",     // class Foo { ... }
            "property_declaration",  // int Foo { get; set; }
        ],
        _ => &[],
    }
}

/// ASTを再帰的に走査し、ターゲットノードのソーステキストを収集する。
///
/// # 動作
/// - ノードの種類がターゲットに一致したら → テキストを result に追加して再帰終了
/// - 一致しなければ → 子ノードを再帰的に探索する
///
/// # 引数
/// - `node`         : 現在のASTノード
/// - `source`       : 元のソースコード文字列（バイト列で参照するため必要）
/// - `target_kinds` : 抽出対象のノード種別リスト
/// - `result`       : 抽出したテキストの格納先
pub fn collect_nodes<'a>(
    node: Node<'a>,
    source: &str,
    target_kinds: &[&str],
    result: &mut Vec<String>,
) {
    if target_kinds.contains(&node.kind()) {
        // ターゲットノードに一致 → ソースのバイト範囲からテキストを切り出す
        let text = &source[node.byte_range()];
        result.push(text.to_string());
        // このノードの子は探索不要（定義の中身ごと取れているため）
        return;
    }

    // ターゲット外 → 子ノードを再帰探索
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_nodes(child, source, target_kinds, result);
    }
}
