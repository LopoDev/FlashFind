// 検索対象にできる拡張子の定義。
// グループ化して UI に表示する。
const EXTENSION_GROUPS = [
  {
    label: "コード",
    extensions: ["rs", "py", "cpp", "c", "h", "cs"],
  },
  {
    label: "Excel",
    extensions: ["xlsx", "xls", "xlsm", "xlsb"],
  },
];

type Props = {
  // 現在選択されている拡張子のセット（空 = 全て対象）
  selected: Set<string>;
  // チェックボックスのトグル時に呼ばれる
  onChange: (ext: string) => void;
};

export default function ExtensionFilter({ selected, onChange }: Props) {
  const allSelected = selected.size === 0;

  return (
    <div className="flex flex-wrap items-center gap-x-4 gap-y-1 px-1 py-2 text-sm text-gray-600">
      {/* "すべて" ボタン: 選択をリセットして全拡張子を対象にする */}
      <label className="flex cursor-pointer items-center gap-1">
        <input
          type="checkbox"
          checked={allSelected}
          onChange={() => {
            // すべてにチェックが入っているときはクリックしても何もしない
            // それ以外は onChange("") を使って親でリセット処理する
            if (!allSelected) onChange("__all__");
          }}
          className="accent-blue-500"
        />
        <span className="font-medium">すべて</span>
      </label>

      {/* 区切り */}
      <span className="text-gray-300">|</span>

      {/* グループごとに拡張子チェックボックスを表示 */}
      {EXTENSION_GROUPS.map((group) => (
        <div key={group.label} className="flex items-center gap-2">
          <span className="text-xs text-gray-400">{group.label}:</span>
          {group.extensions.map((ext) => (
            <label key={ext} className="flex cursor-pointer items-center gap-0.5">
              <input
                type="checkbox"
                checked={!allSelected && selected.has(ext)}
                onChange={() => onChange(ext)}
                className="accent-blue-500"
              />
              <span>.{ext}</span>
            </label>
          ))}
        </div>
      ))}
    </div>
  );
}
