type Props = {
  value: string;
  onChange: (value: string) => void;
};

export default function SearchBox({ value, onChange }: Props) {
  return (
    <input
      type="text"
      value={value}
      onChange={(e) => onChange(e.target.value)}
      placeholder="検索..."
      className="w-full rounded-xl border border-gray-300 bg-white px-6 py-4 text-lg shadow-sm outline-none focus:border-blue-400 focus:ring-2 focus:ring-blue-100"
    />
  );
}
