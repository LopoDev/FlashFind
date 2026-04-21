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
  onDeleted: () => void;
};

export default function DirectoryList({ refreshTrigger, onDeleted }: Props) {
  const [directories, setDirectories] = useState<string[]>([]);
  const [progress, setProgress] = useState<Record<string, Progress>>({});
  const [reparsing, setReparsing] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);

  useEffect(() => {
    invoke<string[]>("get_directories").then(setDirectories);
  }, [refreshTrigger]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    listen<IndexProgressEvent>("index_progress", (event) => {
      const { dir_path, current, total } = event.payload;
      setProgress((prev) => ({ ...prev, [dir_path]: { current, total } }));
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

  const handleDelete = async (dir: string) => {
    setDeleting(dir);
    setConfirmDelete(null);
    try {
      await invoke("delete_directory", { dirPath: dir });
      onDeleted();
    } finally {
      setDeleting(null);
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
        const isDeleting = deleting === dir;
        const isConfirming = confirmDelete === dir;
        const percent = prog ? Math.round((prog.current / prog.total) * 100) : null;
        const busy = !!prog || isReparsing || isDeleting;

        return (
          <li key={dir} className="rounded-lg bg-gray-100 px-3 py-2">
            <div className="flex items-center justify-between">
              <span className="truncate text-sm text-gray-700" title={dir}>
                {dir.split("\\").pop()?.split("/").pop() ?? dir}
              </span>
              <div className="ml-2 flex shrink-0 gap-1">
                <button
                  onClick={() => handleReparse(dir)}
                  disabled={busy || isConfirming}
                  className="text-xs text-blue-500 hover:text-blue-700 disabled:opacity-40"
                >
                  再パース
                </button>
                <button
                  onClick={() => setConfirmDelete(dir)}
                  disabled={busy || isConfirming}
                  className="text-xs text-red-400 hover:text-red-600 disabled:opacity-40"
                >
                  削除
                </button>
              </div>
            </div>

            {isConfirming && (
              <div className="mt-2 flex items-center gap-2 text-xs">
                <span className="text-gray-500">本当に削除しますか？</span>
                <button
                  onClick={() => handleDelete(dir)}
                  className="rounded bg-red-500 px-2 py-0.5 text-white hover:bg-red-600"
                >
                  削除
                </button>
                <button
                  onClick={() => setConfirmDelete(null)}
                  className="rounded bg-gray-200 px-2 py-0.5 text-gray-600 hover:bg-gray-300"
                >
                  キャンセル
                </button>
              </div>
            )}

            {isDeleting && (
              <p className="mt-1 text-xs text-red-400">削除中...</p>
            )}

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
