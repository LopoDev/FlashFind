import { useState } from "react";
import Sidebar from "./components/Sidebar/index";
import Search from "./components/Search/index";

function App() {
  const [isSidebarOpen, setIsSidebarOpen] = useState(true);

  return (
    <div className="flex h-screen w-screen bg-gray-50 text-gray-900 font-mono">
      {isSidebarOpen && <Sidebar onClose={() => setIsSidebarOpen(false)} />}

      <main className="flex flex-1 flex-col">
        {!isSidebarOpen && (
          <button
            onClick={() => setIsSidebarOpen(true)}
            className="m-3 w-fit rounded-lg border border-gray-200 bg-white px-3 py-1 text-sm text-gray-500 hover:text-gray-800"
          >
            ☰ メニュー
          </button>
        )}
        <div className="flex flex-1 items-center justify-center">
          <Search />
        </div>
      </main>
    </div>
  );
}

export default App;
