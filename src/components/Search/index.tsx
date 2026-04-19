import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import SearchBox from "./SearchBox";
import SearchResults, { SearchResult } from "./SearchResults";
import ExtensionFilter from "./ExtensionFilter";

export default function Search() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);

  // 選択された拡張子のセット。
  // 空 = 全拡張子が対象（フィルターなし）
  const [selectedExts, setSelectedExts] = useState<Set<string>>(new Set());

  // 拡張子チェックボックスのトグル処理
  const handleExtChange = (ext: string) => {
    if (ext === "__all__") {
      // "すべて" が押されたら選択をリセット
      setSelectedExts(new Set());
      return;
    }
    setSelectedExts((prev) => {
      const next = new Set(prev);
      if (next.has(ext)) {
        next.delete(ext);
      } else {
        next.add(ext);
      }
      return next;
    });
  };

  // クエリまたは選択拡張子が変わったら 300ms デバウンスして検索
  useEffect(() => {
    if (!query.trim()) {
      setResults([]);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const res = await invoke<SearchResult[]>("search", {
          query,
          // 選択なし = [] を渡す（Rust 側でフィルターなしと判断）
          extensions: Array.from(selectedExts),
        });
        setResults(res);
      } catch (e) {
        console.error("search error:", e);
        setResults([]);
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [query, selectedExts]);

  return (
    <div className="w-2/3">
      <SearchBox value={query} onChange={setQuery} />
      {/* 拡張子フィルター: 検索ボックスの下に表示 */}
      <ExtensionFilter selected={selectedExts} onChange={handleExtChange} />
      <SearchResults results={results} />
    </div>
  );
}
