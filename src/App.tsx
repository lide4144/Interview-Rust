import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import "./App.css";

interface ConfigStatus {
  has_siliconflow: boolean;
  has_deepseek: boolean;
}

interface AnalyzeResult {
  code: string;
  solution: string;
}

type Page = "main" | "settings";

function App() {
  const [page, setPage] = useState<Page>("main");
  const [config, setConfig] = useState<ConfigStatus | null>(null);

  useEffect(() => {
    invoke<ConfigStatus>("get_config")
      .then(setConfig)
      .catch(() => setConfig({ has_siliconflow: false, has_deepseek: false }));
  }, []);

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900 text-gray-900 dark:text-gray-100">
      <nav className="flex items-center justify-between px-4 py-3 border-b border-gray-200 dark:border-gray-700 bg-white dark:bg-gray-800">
        <h1 className="text-lg font-semibold flex items-center gap-2">
          <span className="text-blue-500">&#128187;</span>
          Interview Helper
        </h1>
        <div className="flex items-center gap-3">
          {config && (!config.has_siliconflow || !config.has_deepseek) && (
            <span className="text-xs text-amber-500">&#9888; 未配置 Key</span>
          )}
          <button
            onClick={() => setPage(page === "main" ? "settings" : "main")}
            className="text-sm px-3 py-1.5 rounded-md bg-gray-100 dark:bg-gray-700 hover:bg-gray-200"
          >
            {page === "main" ? "⚙️ 设置" : "← 返回"}
          </button>
        </div>
      </nav>
      <main className="p-4 max-w-4xl mx-auto">
        {page === "settings" ? (
          <SettingsPage onSaved={() => invoke<ConfigStatus>("get_config").then(setConfig)} />
        ) : (
          <MainPage />
        )}
      </main>
    </div>
  );
}

function SettingsPage({ onSaved }: { onSaved: () => void }) {
  const [sfKey, setSfKey] = useState("");
  const [dsKey, setDsKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [msg, setMsg] = useState("");

  const handleSave = async () => {
    setSaving(true); setMsg("");
    try {
      await invoke("save_config", { siliconflowKey: sfKey || null, deepseekKey: dsKey || null });
      setMsg("✅ 已保存"); onSaved();
    } catch (e) { setMsg("❌ " + e); }
    setSaving(false);
  };

  return (
    <div className="space-y-6">
      <h2 className="text-lg font-medium">API 配置</h2>
      <p className="text-sm text-gray-500">Key 保存在本地，不会上传。</p>
      <div className="space-y-4">
        <div>
          <label className="block text-sm font-medium mb-1">SiliconFlow API Key</label>
          <input type="password" value={sfKey} onChange={(e) => setSfKey(e.target.value)} placeholder="sk-..."
            className="w-full px-3 py-2 border rounded-md bg-white dark:bg-gray-800 text-sm font-mono" />
        </div>
        <div>
          <label className="block text-sm font-medium mb-1">DeepSeek API Key</label>
          <input type="password" value={dsKey} onChange={(e) => setDsKey(e.target.value)} placeholder="sk-..."
            className="w-full px-3 py-2 border rounded-md bg-white dark:bg-gray-800 text-sm font-mono" />
        </div>
      </div>
      <button onClick={handleSave} disabled={saving}
        className="px-4 py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 text-sm">
        {saving ? "保存中..." : "保存"}
      </button>
      {msg && <p className="text-sm">{msg}</p>}
    </div>
  );
}

function MainPage() {
  const [code, setCode] = useState("");
  const [solution, setSolution] = useState("");
  const [loading, setLoading] = useState(false);
  const [mode, setMode] = useState<"code" | "image">("code");
  const [error, setError] = useState("");
  const fileRef = useRef<HTMLInputElement>(null);

  const handleAnalyze = async () => {
    if (!code.trim()) return;
    setLoading(true); setError(""); setSolution("");
    try {
      const result = await invoke<string>("solve_code", { code });
      setSolution(result);
    } catch (e) { setError(String(e)); }
    setLoading(false);
  };

  const handleImageSelect = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    setLoading(true); setError(""); setSolution("");
    const reader = new FileReader();
    reader.onload = async () => {
      const base64 = (reader.result as string).split(",")[1];
      setCode("正在识别...");
      try {
        const result = await invoke<AnalyzeResult>("analyze_screenshot", { imageBase64: base64 });
        setCode(result.code); setSolution(result.solution);
      } catch (e) { setError(String(e)); setCode(""); }
      setLoading(false);
    };
    reader.readAsDataURL(file);
  };

  return (
    <div className="space-y-4">
      <div className="flex gap-2">
        <button onClick={() => setMode("code")}
          className={`px-3 py-1.5 text-sm rounded-md ${mode === "code" ? "bg-blue-600 text-white" : "bg-gray-100 dark:bg-gray-700"}`}>✏️ 输入代码</button>
        <button onClick={() => setMode("image")}
          className={`px-3 py-1.5 text-sm rounded-md ${mode === "image" ? "bg-blue-600 text-white" : "bg-gray-100 dark:bg-gray-700"}`}>📷 上传截图</button>
      </div>

      {mode === "code" ? (
        <textarea value={code} onChange={(e) => setCode(e.target.value)} placeholder="在此粘贴或输入代码..."
          className="w-full h-40 px-3 py-2 border rounded-md bg-white dark:bg-gray-800 text-sm font-mono resize-y" />
      ) : (
        <div onClick={() => fileRef.current?.click()}
          className="border-2 border-dashed border-gray-300 dark:border-gray-600 rounded-md p-8 text-center cursor-pointer hover:border-blue-400">
          <input ref={fileRef} type="file" accept="image/*" onChange={handleImageSelect} className="hidden" />
          <p className="text-gray-400 text-sm">点击选择截图文件</p>
          <p className="text-gray-400 text-xs mt-1">PNG / JPG</p>
          {code && !solution && <p className="text-sm text-gray-500 mt-2">{code}</p>}
        </div>
      )}

      {code.trim() && mode === "code" && (
        <button onClick={handleAnalyze} disabled={loading}
          className="w-full py-2 bg-blue-600 text-white rounded-md hover:bg-blue-700 disabled:opacity-50 text-sm font-medium">
          {loading ? <span className="flex items-center justify-center gap-2"><span className="animate-spin">&#9696;</span>分析中...</span> : "🚀 分析"}
        </button>
      )}

      {error && <div className="p-3 bg-red-50 dark:bg-red-900/20 border border-red-200 rounded-md text-sm text-red-700">{error}</div>}

      {code && solution && mode === "image" && (
        <div>
          <h3 className="text-sm font-medium mb-2 text-gray-500">识别的代码</h3>
          <pre className="p-3 bg-gray-50 dark:bg-gray-800 rounded-md text-sm font-mono overflow-x-auto border">{code}</pre>
        </div>
      )}

      {solution && (
        <div className="space-y-3">
          <h2 className="text-base font-medium flex items-center gap-2"><span>&#128221;</span> 分析结果</h2>
          <div className="p-4 bg-white dark:bg-gray-800 rounded-md border">
            <div className="prose prose-sm dark:prose-invert max-w-none">
              <Markdown remarkPlugins={[remarkGfm]}>
                {solution}
              </Markdown>
            </div>
          </div>
          <button onClick={() => navigator.clipboard.writeText(solution)}
            className="text-xs px-3 py-1.5 bg-gray-100 dark:bg-gray-700 rounded-md hover:bg-gray-200">
            📋 复制结果
          </button>
        </div>
      )}
    </div>
  );
}

export default App;
