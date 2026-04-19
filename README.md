<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" alt="FlashFind Logo" width="96" />
</p>

<h1 align="center">⚡ FlashFind</h1>

<p align="center">
 AIが「意味」を理解する、ローカルファイル向けセマンティック検索デスクトップアプリ
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Tauri-2.x-24C8D8?style=for-the-badge&logo=tauri&logoColor=white" alt="Tauri" />
  <img src="https://img.shields.io/badge/Rust-1.80+-000000?style=for-the-badge&logo=rust&logoColor=white" alt="Rust" />
  <img src="https://img.shields.io/badge/React-19-61DAFB?style=for-the-badge&logo=react&logoColor=black" alt="React" />
  <img src="https://img.shields.io/badge/TypeScript-5.8-3178C6?style=for-the-badge&logo=typescript&logoColor=white" alt="TypeScript" />
  <img src="https://img.shields.io/badge/Tailwind_CSS-4-06B6D4?style=for-the-badge&logo=tailwindcss&logoColor=white" alt="Tailwind CSS" />
  <img src="https://img.shields.io/badge/Qdrant-latest-EE2B7B?style=for-the-badge" alt="Qdrant" />
</p>

---

## 📖 目次

1. [概要](#-概要)
2. [アーキテクチャ](#-アーキテクチャ)
3. [技術スタック](#-技術スタック)
4. [プロジェクト構成](#-プロジェクト構成)
5. [主要機能](#-主要機能)
6. [対応ファイル形式](#-対応ファイル形式)
7. [セットアップ](#-セットアップ)
8. [開発コマンド](#-開発コマンド)
9. [データフロー詳細](#-データフロー詳細)
10. [SQLite スキーマ](#-sqlite-スキーマ)
11. [Qdrant コレクション構造](#-qdrant-コレクション構造)
12. [コントリビューション](#-コントリビューション)
13. [ライセンス](#-ライセンス)

---

## 🔍 概要

**FlashFind** は、ローカルのコードや Excel ファイルを **AI の意味理解** で検索できるデスクトップアプリです。
従来のキーワード検索ではなく、埋め込みベクトル（Embeddings）と Qdrant ベクトル DB を活用した **セマンティック検索** を実現します。

> 「あの関数どこに書いたっけ？」「売上集計のシートを探したい」——自然言語で一発検索。

### ✨ 特徴

| 特徴 | 説明 |
|------|------|
| 🧠 セマンティック検索 | キーワード一致ではなく、クエリの「意味」をベクトル化して類似検索 |
| 🌐 多言語対応 | `MultilingualE5Small` モデルで日本語・英語の混在コードにも対応 |
| ⚡ 差分インデックス | 変更・追加・削除ファイルのみを自動検出して効率的に更新 |
| 🔒 完全ローカル | クラウド不使用。すべての処理がローカルマシン内で完結 |
| 📄 コード + Excel | tree-sitter による AST 解析と markitdown による Excel 変換に対応 |

---

## 🏗 アーキテクチャ

### システム全体図

```
┌─────────────────────────────────────────────────────────────────┐
│                     FlashFind Desktop App                       │
│                                                                 │
│  ┌────────────────────────────────────────────────────────┐   │
│  │  フロントエンド (React 19 / TypeScript / Tailwind CSS)    │   │
│  │                                                          │   │
│  │  ┌─────────────────┐    ┌──────────────────────────┐    │   │
│  │  │  Sidebar        │    │  Search                  │    │   │
│  │  │  ─────────────  │    │  ────────────────────    │    │   │
│  │  │  ディレクトリ管理 │    │  SearchBox               │    │   │
│  │  │  進捗バー表示    │    │  ExtensionFilter         │    │   │
│  │  │  再パースボタン  │    │  SearchResults           │    │   │
│  │  └────────┼────────┘    └───────────┼──────────────┘    │   │
│  └───────────┼────────────────────────┼───────────────────┘   │
│              │  Tauri IPC (invoke)    │                        │
│  ┌───────────▼────────────────────────▼───────────────────┐    │
│  │  バックエンド (Rust / Tauri 2)                           │    │
│  │                                                         │    │
│  │  ┌──────────┐  ┌──────────┐  ┌───────────┐             │    │
│  │  │ pipeline │  │  qdrant  │  │  sqlite   │             │    │
│  │  │ クレート  │  │  クレート │  │  クレート  │             │    │
│  │  └────┼─────┘  └────┼─────┘  └─────┼─────┘             │    │
│  │       │             │              │                    │    │
│  │  ┌────▼─────┐  ┌────▼──────┐  ┌───▼──────────────┐    │    │
│  │  │treesitter│  │fastembed  │  │  SQLite (app.db)  │    │    │
│  │  │  クレート │  │Embeddings │  │  root_folders    │    │    │
│  │  └──────────┘  └───────────┘  │  folders         │    │    │
│  │                               └──────────────────┘    │    │
│  └────────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌─────────────────────┐   ┌──────────────────────────────┐    │
│  │  Qdrant サイドカー   │   │  markitdown_sidecar          │    │
│  │  ─────────────────  │   │  ──────────────────────────  │    │
│  │  ベクトル DB         │   │  Python 製                   │    │
│  │  gRPC :6334         │   │  Excel → Markdown 変換       │    │
│  └─────────────────────┘   └──────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### インデックス作成フロー

```
ディレクトリ走査
      │
      ▼
SQLite と比較
      │
  変更あり？
  ┌───┴────────┬────────────┐
  │              │              │
新規ファイル   変更ファイル    削除ファイル
  │              │              │
パース         差分パース     Qdrant から削除
  │              │              │
埋め込み生成   差分 upsert   SQLite から削除
  │              │
Qdrant upsert  完了
  │
SQLite 更新
```

---

## 🛠 技術スタック

| レイヤー | 技術 | バージョン |
|---------|------|-----------|
| フロントエンド | React | 19.x |
| フロントエンド | TypeScript | 5.8 |
| フロントエンド | Tailwind CSS | 4.x |
| フロントエンド | Vite | 7.x |
| バックエンド | Rust + Tauri | 2.x |
| ベクトル DB | Qdrant（サイドカー） | latest |
| 埋め込みモデル | fastembed / MultilingualE5Small | 384 次元・コサイン類似度 |
| メタデータ DB | SQLite（rusqlite） | 0.39 |
| コード解析 | tree-sitter | 0.26 |
| Excel パース | markitdown（Python サイドカー） | — |

---

## 📁 プロジェクト構成

```
FlashFind/
├── src/                          # フロントエンド (React / TypeScript)
│   ├── components/
│   │   ├── Search/
│   │   │   ├── index.tsx           # 検索ロジック統合（状態管理・Tauri invoke）
│   │   │   ├── SearchBox.tsx       # 検索入力ボックス
│   │   │   ├── SearchResults.tsx   # 検索結果リスト表示
│   │   │   └── ExtensionFilter.tsx # 拡張子フィルター
│   │   └── Sidebar/
│   │       ├── index.tsx             # サイドバー全体
│   │       ├── DirectoryList.tsx     # ディレクトリ一覧 + 進捗バー
│   │       └── AddDirectoryButton.tsx # ディレクトリ追加ボタン
│   ├── App.tsx
│   └── main.tsx
│
├── src-tauri/                    # バックエンド (Rust / Tauri)
│   ├── src/                      # Tauri コマンドハンドラ
│   ├── crates/
│   │   ├── pipeline/             # インデックス作成・検索のオーケストレーション
│   │   ├── qdrant/               # ベクトル DB 操作 + 埋め込み生成
│   │   ├── sqlite/               # ファイルメタデータ管理
│   │   ├── treesitter/           # AST を使ったコードのチャンク分割
│   │   └── parser/               # 基底パーサトレイト
│   ├── binaries/                 # サイドカーバイナリ格納先
│   │   ├── qdrant                # Qdrant バイナリ
│   │   └── markitdown_sidecar    # Python 製 Excel パーサー
│   └── tauri.conf.json
│
├── src-python/                   # markitdown サイドカーのソース
├── package.json
└── README.md
```

### Rust クレート依存関係

```
flashfind (main crate)
    └── pipeline
            ├── qdrant      ← fastembed, qdrant-client
            ├── sqlite      ← rusqlite
            ├── treesitter  ← tree-sitter-{rust,python,cpp,c-sharp}
            └── parser      ← 基底トレイト定義
```

---

## 🚀 主要機能

### 1. セマンティック検索

キーワードではなく **意味** で検索します。クエリを `MultilingualE5Small`（384 次元）でベクトル化し、Qdrant のコサイン類似度検索で上位 10 件を返します。

```
検索ボックス入力
      │
  300ms デバウンス
      │
      ▼
MultilingualE5Small でクエリをベクトル化（384 次元）
      │
      ▼
Qdrant コサイン類似度検索
      │
      ▼
上位 10 件を UI に表示
```

### 2. 拡張子フィルター

| フィルター | 対象拡張子 |
|-----------|-----------|
| すべて | フィルターなし（全件対象） |
| コード | `.rs` `.py` `.cpp` `.c` `.h` `.cs` |
| Excel | `.xlsx` `.xls` `.xlsm` `.xlsb` |

Qdrant のペイロードフィルターを利用するため、DB 全件スキャンは発生しません。

### 3. チャンク分割戦略

| ファイル種別 | 分割単位 | 特徴 |
|-------------|---------|------|
| コードファイル | 関数 / クラス / 構造体 / トレイトごと | 定義単位で差分管理が可能 |
| Excel ファイル | 1 ファイル = 1 チャンク | 全シートのテキストをまとめて保存 |

チャンク ID は `hash(path + content)` によるコンテンツアドレッシング。同一内容は再埋め込みせず冪等 upsert します。

### 4. 差分インデックス更新

アプリが閉じている間の変更も、次回起動時に自動で取り込みます。

```
1. ディレクトリを再帰走査 → 現在のファイル一覧を取得
2. SQLite の前回記録と突合せ

        ┌──────────┬────────────┐
       new       modified      deleted
        │          │              │
        │       差分チャンク特定   │
        │          ├─ 消えた定義 → Qdrant 削除
        │          └─ 新しい定義 → Qdrant upsert
        │                         │
   パース＆全チャンク upsert    Qdrant 全チャンク削除
                                 SQLite レコード削除

3. SQLite を最新状態に更新
```

### 5. 起動時自動再インデックス

- アプリ起動後、Qdrant の準備完了（3 秒待機）後に全登録ディレクトリを自動再インデックス
- アプリが閉じている間に変更されたファイルを自動的に取り込みます

### 6. OOM 対策（Excel パース時）

大量の Excel ファイルを処理する際のメモリ不足を防ぐため、以下の制御を行います。

| 対策 | 内容 |
|------|------|
| 並列数制限 | markitdown サイドカーの同時起動数を最大 **8** に制限（Tokio セマフォ） |
| バッチ処理 | 埋め込み生成を **64 件単位** でバッチ処理し、メモリ使用量を抑制 |

### 7. ディレクトリ管理（サイドバー）

| 操作 | 説明 |
|------|------|
| ディレクトリ追加 | OS のフォルダー選択ダイアログからディレクトリを登録 |
| 一覧表示 | 登録済みディレクトリを一覧表示 |
| 再パース | 各ディレクトリ単位で手動再インデックスを実行 |
| 進捗バー | インデックス処理の current / total をリアルタイム表示 |

---

## 📂 対応ファイル形式

### コードファイル（tree-sitter による AST 解析）

| 言語 | 拡張子 | 抽出するノード |
|------|--------|--------------|
| Rust | `.rs` | `function_item`, `impl_item`, `struct_item`, `enum_item`, `trait_item` |
| Python | `.py` | `function_definition`, `class_definition` |
| C / C++ | `.c` `.cpp` `.h` `.hpp` | `function_definition`, `class_specifier`, `struct_specifier` |
| C# | `.cs` | `method_declaration`, `class_declaration`, `property_declaration` |

### Excel ファイル（markitdown サイドカー経由）

| 形式 | 拡張子 |
|------|--------|
| Excel（OOXML） | `.xlsx` |
| Excel（旧形式） | `.xls` |
| Excel マクロ有効 | `.xlsm` |
| Excel バイナリ | `.xlsb` |

---

## ⚙️ セットアップ

### 必要な環境

| ツール | バージョン | 備考 |
|--------|-----------|------|
| Rust | 1.80 以上 | `rustup` でインストール推奨 |
| Node.js | 20 以上 | `fnm` / `nvm` 推奨 |
| npm | 10 以上 | Node.js に同桁 |
| Python | 3.11 以上 | markitdown サイドカーのビルドに必要 |
| Xcode CLT (macOS) | — | `xcode-select --install` |

### 1. リポジトリのクローン

```sh
git clone https://github.com/your-username/FlashFind.git
cd FlashFind
```

### 2. フロントエンド依存関係のインストール

```sh
npm install
```

### 3. Rust ツールチェーンの確認

```sh
rustup update stable
rustup target list --installed
```

### 4. Python サイドカーのビルド（任意）

markitdown サイドカーのバイナリを自前でビルドする場合：

```sh
cd src-python
pip install pyinstaller markitdown
pyinstaller --onefile markitdown_sidecar.py
# ビルド生成物を src-tauri/binaries/ にコピー
cp dist/markitdown_sidecar ../src-tauri/binaries/
```

> **Note:** ビルド済みバイナリがリポジトリに含まれている場合、この手順は不要です。

### 5. 開発サーバーの起動

```sh
npm run tauri dev
```

初回起動時は以下が自動実行されます：

- Rust クレートのコンパイル（数分かかる場合があります）
- Qdrant サイドカーの起動（gRPC ポート 6334）
- fastembed モデル（`MultilingualE5Small`）のダウンロード・キャッシュ

---

## 🖥 開発コマンド

| コマンド | 説明 |
|---------|------|
| `npm run tauri dev` | 開発サーバー起動（ホットリロード対応） |
| `npm run tauri build` | プロダクションビルド（インストーラー生成） |
| `npm run dev` | フロントエンドのみ Vite 起動 |
| `npm run build` | フロントエンドのみビルド |

### 推奨 IDE 設定

| エディタ | 拡張機能 |
|---------|---------|
| VS Code | [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode), [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) |
| Zed | Rust 言語サーバー（組み込み） |

---

## 🔄 データフロー詳細

### 検索リクエスト

```
UI: SearchBox (入力)
  │  300ms デバウンス
  ▼
Search/index.tsx
  │  tauri::invoke("search_files", { query, extensions })
  ▼
src-tauri/src/ (Tauri コマンドハンドラ)
  │
  ▼
pipeline クレート: search()
  │
  ├─ qdrant クレート: embed_query(query)
  │     └─ fastembed: MultilingualE5Small → Vec<f32> (384次元)
  │
  └─ qdrant クレート: search_vectors(vector, filter)
        └─ Qdrant gRPC: コサイン類似度検索 → top-10
  │
  ▼
SearchResult 構造体のリストを返却
  │
  ▼
UI: SearchResults.tsx (結果表示)
```

### インデックス作成リクエスト

```
UI: AddDirectoryButton / 再パースボタン
  │  tauri::invoke("index_directory", { path })
  ▼
pipeline クレート: index_directory()
  │
  ├─ sqlite クレート: get_indexed_files()         ← 前回の記録取得
  ├─ walkdir: ディレクトリ再帰走査
  │
  ├─ [新規・変更ファイル]
  │     ├─ treesitter クレート: parse_chunks()    ← コードの場合
  │     │     └─ AST 解析 → Vec<Chunk>
  │     │
  │     ├─ markitdown_sidecar: excel_to_md()      ← Excel の場合
  │     │     └─ サイドカー起動 → Markdown テキスト
  │     │
  │     └─ qdrant クレート: upsert_chunks()
  │           └─ fastembed: バッチ埋め込み生成 (64件単位)
  │                 └─ Qdrant: upsert
  │
  ├─ [削除ファイル]
  │     └─ qdrant クレート: delete_by_path()
  │
  └─ sqlite クレート: update_records()
```

---

## 🗄 SQLite スキーマ

データベースファイル: `src-tauri/app.db`

### `root_folders` テーブル

| カラム | 型 | 説明 |
|--------|-----|------|
| `id` | INTEGER PRIMARY KEY | 自動採番 |
| `path` | TEXT NOT NULL UNIQUE | 登録済みルートディレクトリの絶対パス |
| `created_at` | TEXT | 登録日時（ISO 8601） |

### `folders` テーブル

| カラム | 型 | 説明 |
|--------|-----|------|
| `id` | INTEGER PRIMARY KEY | 自動採番 |
| `root_dir` | TEXT NOT NULL | 所属するルートディレクトリ |
| `path` | TEXT NOT NULL UNIQUE | インデックス済みファイルの絶対パス |
| `updated_at` | TEXT | 最終更新日時（変更検知に使用） |
| `created_at` | TEXT | 初回インデックス日時 |

---

## 🗂 Qdrant コレクション構造

| 項目 | 値 |
|------|-----|
| コレクション名 | `files` |
| ベクトル次元数 | 384 |
| 距離関数 | Cosine |
| ペイロードキー | `file`, `content`, `ext` |

チャンク ID は `hash(path + content)` で生成されるため、同一コンテンツに対しては重複 upsert が発生しません。

---

## 🤝 コントリビューション

1. このリポジトリをフォーク
2. フィーチャーブランチを作成 (`git checkout -b feature/amazing-feature`)
3. 変更をコミット (`git commit -m 'feat: add amazing feature'`)
4. ブランチにプッシュ (`git push origin feature/amazing-feature`)
5. Pull Request を作成

---

## 📄 ライセンス

MIT License — 詳細は [LICENSE](LICENSE) を参照してください。

---

<p align="center">
  Built with ❤️ using Tauri + React + Rust
</p>

