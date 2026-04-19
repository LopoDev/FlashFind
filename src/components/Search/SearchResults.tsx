export type SearchResult = {
  file: string;
  content: string;
  score: number;
};

type Props = {
  results: SearchResult[];
};

export default function SearchResults({ results }: Props) {
  if (results.length === 0) return null;

  return (
    <ul className="mt-2 w-full rounded-xl border border-gray-200 bg-white shadow-lg overflow-hidden">
      {results.map((r) => (
        <li
          key={r.file}
          className="px-6 py-3 border-b last:border-b-0 hover:bg-gray-50 cursor-pointer"
        >
          <p className="text-sm font-medium text-gray-800 truncate">
            {r.file.split("/").pop()}
          </p>
          <p className="text-xs text-gray-400 truncate">{r.file}</p>
          <p className="text-xs text-gray-500 mt-1 line-clamp-2">{r.content}</p>
        </li>
      ))}
    </ul>
  );
}
