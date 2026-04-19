import { useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { invoke } from "@tauri-apps/api/core";

type Props = {
  onAdded: () => void;
};

export default function AddDirectoryButton({ onAdded }: Props) {
  const [indexing, setIndexing] = useState(false);

  const handleClick = async () => {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;

    setIndexing(true);
    try {
      await invoke("index_directory", { dirPath: selected });
      onAdded();
    } finally {
      setIndexing(false);
    }
  };

  return (
    <button
      onClick={handleClick}
      disabled={indexing}
      className="rounded-lg bg-blue-500 px-4 py-2 text-sm text-white hover:bg-blue-600 disabled:opacity-50"
    >
      {indexing ? "処理中..." : "＋ ディレクトリを追加"}
    </button>
  );
}
