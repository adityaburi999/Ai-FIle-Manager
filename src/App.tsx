import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

type ExclusionMode = "ignore" | "read_only" | "manual";

type OrganizationSummary = {
  scanned: number;
  moved: number;
  skipped: number;
  duplicates: number;
  indexed: number;
  rollback_group: string;
};

type SearchResult = {
  path: string;
  relevance_score: number;
  preview_metadata: string;
};

type ActionLog = {
  timestamp: string;
  action: string;
  source: string;
  destination: string;
  reason: string;
  model_confidence: number;
  rollback_group: string;
};

type SystemStatus = {
  watched_folders: { path: string; watching: boolean }[];
  index_root: string;
  database_path: string;
};

function App() {
  const [targetPath, setTargetPath] = useState("");
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchResult[]>([]);
  const [logs, setLogs] = useState<ActionLog[]>([]);
  const [status, setStatus] = useState<SystemStatus | null>(null);
  const [summary, setSummary] = useState<OrganizationSummary | null>(null);
  const [isBusy, setIsBusy] = useState(false);
  const [continuousEnabled, setContinuousEnabled] = useState(false);
  const [excludePath, setExcludePath] = useState("");
  const [excludeMode, setExcludeMode] = useState<ExclusionMode>("ignore");
  const [message, setMessage] = useState("");

  const latestRollback = useMemo(() => logs[0]?.rollback_group ?? summary?.rollback_group ?? "", [logs, summary]);

  const refresh = async () => {
    const [nextLogs, nextStatus] = await Promise.all([
      invoke<ActionLog[]>("get_logs", { limit: 30 }),
      invoke<SystemStatus>("system_status"),
    ]);
    setLogs(nextLogs);
    setStatus(nextStatus);
  };

  useEffect(() => {
    refresh().catch((error) => setMessage(String(error)));
  }, []);

  const runOrganize = async () => {
    if (!targetPath.trim()) {
      setMessage("Target path is required.");
      return;
    }

    setIsBusy(true);
    try {
      const next = await invoke<OrganizationSummary>("organize_directory", { path: targetPath.trim() });
      setSummary(next);
      setMessage("Organization complete.");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setIsBusy(false);
    }
  };

  const runSearch = async () => {
    if (!searchQuery.trim()) {
      setSearchResults([]);
      return;
    }

    try {
      const results = await invoke<SearchResult[]>("semantic_search", { query: searchQuery.trim(), limit: 25 });
      setSearchResults(results);
      setMessage(`Found ${results.length} result(s).`);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const applyExclusion = async () => {
    if (!excludePath.trim()) {
      setMessage("Exclusion path is required.");
      return;
    }

    try {
      await invoke("set_exclusion", {
        rule: {
          path: excludePath.trim(),
          excluded: true,
          mode: excludeMode,
        },
      });
      setMessage("Exclusion rule saved.");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const toggleContinuous = async () => {
    if (!targetPath.trim()) {
      setMessage("Set target path before enabling continuous mode.");
      return;
    }

    try {
      const next = !continuousEnabled;
      await invoke("set_continuous_mode", { path: targetPath.trim(), enabled: next });
      setContinuousEnabled(next);
      setMessage(next ? "Continuous mode enabled." : "Continuous mode disabled.");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    }
  };

  const rollbackLast = async () => {
    if (!latestRollback) {
      setMessage("No rollback group available.");
      return;
    }

    try {
      const restored = await invoke<number>("rollback_group", { rollbackGroup: latestRollback });
      setMessage(`Rollback restored ${restored} file(s).`);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    }
  };

  return (
    <main className="min-h-screen bg-slate-950 text-slate-100 p-6">
      <div className="mx-auto max-w-7xl space-y-6">
        <header className="rounded-xl border border-slate-800 bg-slate-900 p-5">
          <h1 className="text-2xl font-bold">AI File Manager</h1>
          <p className="text-sm text-slate-300 mt-1">
            Autonomous file organization, indexing, semantic search, exclusions, rollback, and continuous watch mode.
          </p>
          {message && <p className="mt-3 text-emerald-300 text-sm">{message}</p>}
        </header>

        <section className="grid gap-4 lg:grid-cols-2">
          <div className="rounded-xl border border-slate-800 bg-slate-900 p-5 space-y-3">
            <h2 className="font-semibold text-lg">Organization Controls</h2>
            <input
              value={targetPath}
              onChange={(e) => setTargetPath(e.target.value)}
              className="w-full rounded border border-slate-700 bg-slate-950 p-2"
              placeholder="/absolute/path/to/organize"
            />
            <div className="flex gap-2 flex-wrap">
              <button
                onClick={runOrganize}
                disabled={isBusy}
                className="rounded bg-blue-600 hover:bg-blue-500 px-4 py-2 disabled:opacity-60"
              >
                Organize Now
              </button>
              <button
                onClick={toggleContinuous}
                className="rounded bg-purple-600 hover:bg-purple-500 px-4 py-2"
              >
                {continuousEnabled ? "Disable Continuous" : "Enable Continuous"}
              </button>
              <button onClick={rollbackLast} className="rounded bg-amber-600 hover:bg-amber-500 px-4 py-2">
                Rollback Last
              </button>
            </div>
            {summary && (
              <div className="text-sm rounded border border-slate-700 p-3 bg-slate-950">
                <p>Scanned: {summary.scanned}</p>
                <p>Moved: {summary.moved}</p>
                <p>Indexed: {summary.indexed}</p>
                <p>Skipped: {summary.skipped}</p>
                <p>Duplicates: {summary.duplicates}</p>
                <p className="truncate">Rollback group: {summary.rollback_group}</p>
              </div>
            )}
          </div>

          <div className="rounded-xl border border-slate-800 bg-slate-900 p-5 space-y-3">
            <h2 className="font-semibold text-lg">Exclusion Rules</h2>
            <input
              value={excludePath}
              onChange={(e) => setExcludePath(e.target.value)}
              className="w-full rounded border border-slate-700 bg-slate-950 p-2"
              placeholder="/absolute/path/to/exclude"
            />
            <select
              value={excludeMode}
              onChange={(e) => setExcludeMode(e.target.value as ExclusionMode)}
              className="w-full rounded border border-slate-700 bg-slate-950 p-2"
            >
              <option value="ignore">ignore (no indexing, no move)</option>
              <option value="read_only">read_only (index only)</option>
              <option value="manual">manual (only user-triggered operations)</option>
            </select>
            <button onClick={applyExclusion} className="rounded bg-cyan-600 hover:bg-cyan-500 px-4 py-2">
              Save Rule
            </button>

            <h3 className="font-medium mt-4">System Status</h3>
            <div className="text-sm rounded border border-slate-700 bg-slate-950 p-3">
              <p className="truncate">DB: {status?.database_path ?? "-"}</p>
              <p className="truncate">Index: {status?.index_root ?? "-"}</p>
              <p>Active watchers: {status?.watched_folders.length ?? 0}</p>
            </div>
          </div>
        </section>

        <section className="grid gap-4 lg:grid-cols-2">
          <div className="rounded-xl border border-slate-800 bg-slate-900 p-5 space-y-3">
            <h2 className="font-semibold text-lg">AI Semantic Search</h2>
            <div className="flex gap-2">
              <input
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                className="flex-1 rounded border border-slate-700 bg-slate-950 p-2"
                placeholder="Find me project invoices from last month"
              />
              <button onClick={runSearch} className="rounded bg-emerald-600 hover:bg-emerald-500 px-4 py-2">
                Search
              </button>
            </div>
            <ul className="space-y-2 max-h-80 overflow-auto">
              {searchResults.map((result) => (
                <li key={`${result.path}-${result.relevance_score}`} className="rounded border border-slate-700 bg-slate-950 p-3">
                  <p className="truncate text-sm font-medium">{result.path}</p>
                  <p className="text-xs text-slate-400">Score: {result.relevance_score.toFixed(3)}</p>
                  <p className="text-xs text-slate-300">{result.preview_metadata}</p>
                </li>
              ))}
            </ul>
          </div>

          <div className="rounded-xl border border-slate-800 bg-slate-900 p-5 space-y-3">
            <div className="flex items-center justify-between">
              <h2 className="font-semibold text-lg">Action Logs</h2>
              <button onClick={refresh} className="rounded bg-slate-700 hover:bg-slate-600 px-3 py-1 text-sm">
                Refresh
              </button>
            </div>
            <ul className="space-y-2 max-h-80 overflow-auto">
              {logs.map((log) => (
                <li key={`${log.timestamp}-${log.source}-${log.destination}`} className="rounded border border-slate-700 bg-slate-950 p-3">
                  <p className="text-xs text-slate-400">{new Date(log.timestamp).toLocaleString()}</p>
                  <p className="text-sm">
                    <span className="font-semibold">{log.action}</span> → {log.destination}
                  </p>
                  <p className="truncate text-xs text-slate-300">From: {log.source}</p>
                  <p className="truncate text-xs text-slate-300">Rollback: {log.rollback_group}</p>
                </li>
              ))}
            </ul>
          </div>
        </section>
      </div>
    </main>
  );
}

export default App;
