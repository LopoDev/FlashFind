import { useState, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import DirectoryList from "./DirectoryList";
import AddDirectoryButton from "./AddDirectoryButton";

type Props = {
  onClose: () => void;
};

export default function Sidebar({ onClose }: Props) {
  const [refreshTrigger, setRefreshTrigger] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);

  const handleAdded = () => {
    setRefreshTrigger((n) => n + 1);
  };

  // インデックス済みスキップ通知
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<string>("index_skipped", (event) => {
      const parts = event.payload.replace(/\/g, "/").split("/");
      const dirName = parts[parts.length - 1] || event.payload;
      setNotice(`「${dirName}」は既にインデックス中です`);
      setTimeout(() => setNotice(null), 3000);
    }).then((fn) => { unlisten = fn; });
    return () => { unlisten?.(); };
  }, []);

  return (
    <aside className="flex w-64 flex-col gap-4 border-r border-gray-200 bg-white p-4">
      {/* ヘッダー */}
      <div className="flex items-center justify-between">
        <h2 className="text-sm font-semibold text-gray-500 uppercase tracking-wider">
          ディレクトリ
        </h2>
        <button
          onClick={onClose}
          className="text-gray-400 hover:text-gray-600 text-lg leading-none"
          aria-label="サイドバーを閉じる"
        >
          ✕
        </button>
      </div>

      {/* スキップ通知 */}
      {notice && (
        <div className="rounded-lg border border-yellow-200 bg-yellow-50 px-3 py-2 text-xs text-yellow-700">
          ⏳ {notice}
        </div>
      )}

      {/* ディレクトリ一覧 */}
      <div className="flex-1 overflow-y-auto">
        <DirectoryList refreshTrigger={refreshTrigger} />
      </div>

      {/* 追加ボタン */}
      <AddDirectoryButton onAdded={handleAdded} />
    </aside>
  );
}