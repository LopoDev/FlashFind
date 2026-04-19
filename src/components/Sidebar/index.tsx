import { useState } from "react";
import DirectoryList from "./DirectoryList";
import AddDirectoryButton from "./AddDirectoryButton";

type Props = {
  onClose: () => void;
};

export default function Sidebar({ onClose }: Props) {
  const [refreshTrigger, setRefreshTrigger] = useState(0);

  const handleAdded = () => {
    setRefreshTrigger((n) => n + 1);
  };

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

      {/* ディレクトリ一覧 */}
      <div className="flex-1 overflow-y-auto">
        <DirectoryList refreshTrigger={refreshTrigger} />
      </div>

      {/* 追加ボタン */}
      <AddDirectoryButton onAdded={handleAdded} />
    </aside>
  );
}
