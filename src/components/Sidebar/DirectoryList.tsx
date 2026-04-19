import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

type Progress = {
  current: number;
  total: number;
};

type IndexProgressEvent = {
  dir_path: string;
  current: number;
  total: number;
};

type Props = {
  refreshTrigger: number;
};

export default function DirectoryList({ refreshTrigger }: Props) {
  const [directories, setDirectories] = useState<string[]>([]);
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [reparsing, setReparsing] = useState<string | null>(null);

  // ディレクトリ一覧をRustから取得
  useEffect(() => {
    invoke<string[]>("get_directories").then(setDirectories);
  }, [refreshTrigger]);

  // 進捗イベントを購読
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<IndexProgressEvent>("index_progress", (event) => {
      const { dir_path, current, total } = event.payload;
      setProgress((prev) => ({ ...prev, [dir_path]: { current, total } }));
      // 完了したら進捗を消す
      if (current === total) {
        setTimeout(() => {
          setProgress((prev) => {
            const next = { ...prev };
            delete next[dir_path];
            return next;
          });
        }, 1500);
      }
    }).then((fn) => { unlisten = fn; });

    return () => { unlisten?.(); };
  }, []);

  const handleReparse = async (dir: string) => {
    setReparsing(dir);
    try {
      await invoke("index_directory", { dirPath: dir });
    } finally {
      setReparsing(null);
    }
  };

  if (directories.length === 0) {
    return <p className="text-xs text-gray-400 px-2">ディレクトリが未登録です</p>;
  }

  return (
    <ul className="flex flex-col gap-2">
      {directories.map((dir) => {
        const prog = progress[dir];
        const isReparsing = reparsing === dir;
        const percent = prog ? Math.round((prog.current / prog.total) * 100) : null;

        return (
          <li key={dir} className="rounded-lg bg-gray-100 px-3 py-2">
            <div className="flex items-center justify-between">
              <span className="truncate text-sm text-gray-700" title={dir}>
                {dir.split("/").pop()}
              </span>
              <button
                onClick={() => handleReparse(dir)}
                disabled={!!prog || isReparsing}
                className="ml-2 shrink-0 text-xs text-blue-500 hover:text-blue-700 disabled:opacity-40"
              >
                再パース
              </button>
            </div>

            {/* 進捗バー */}
            {prog && (
              <div className="mt-1">
                <div className="h-1.5 w-full rounded-full bg-gray-200">
                  <div
                    className="h-1.5 rounded-full bg-blue-400 transition-all"
                    style={{ width: `${percent}%` }}
                  />
                </div>
                <p className="mt-0.5 text-xs text-gray-400">
                  {prog.current} / {prog.total} ({percent}%)
                </p>
              </div>
            )}
          </li>
        );
      })}
    </ul>
  );
}
