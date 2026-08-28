import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  dbService,
  settingsService,
  SyncReport,
  SyncStatus,
} from "../services/db";

function formatTimestamp(unixSeconds: number | null): string {
  if (!unixSeconds) return "Never";
  return new Date(unixSeconds * 1000).toLocaleString();
}

export function SyncSettingsScreen() {
  const queryClient = useQueryClient();
  const [baseUrlInput, setBaseUrlInput] = useState("");
  const [tokenInput, setTokenInput] = useState("");
  const [loaded, setLoaded] = useState(false);

  const { data: status } = useQuery<SyncStatus>({
    queryKey: ["syncStatus"],
    queryFn: () => dbService.getSyncStatus(),
  });

  const { data: lastReport } = useQuery<SyncReport | null>({
    queryKey: ["lastSyncReport"],
    queryFn: () => null,
    initialData: null,
    staleTime: Infinity,
  });

  useEffect(() => {
    let cancelled = false;
    settingsService.loadSettings().then((settings) => {
      if (cancelled) return;
      setBaseUrlInput(settings.base_url);
      setTokenInput(settings.token);
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const syncMutation = useMutation({
    mutationFn: () => dbService.triggerSync(baseUrlInput.trim(), tokenInput.trim()),
    onSuccess: async (report) => {
      queryClient.setQueryData(["lastSyncReport"], report);
      await queryClient.invalidateQueries({ queryKey: ["recipes"] });
      await queryClient.invalidateQueries({ queryKey: ["allRecipes"] });
      await queryClient.invalidateQueries({ queryKey: ["syncStatus"] });
    },
  });

  return (
    <div className="p-4 space-y-6">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!loaded || syncMutation.isPending) return;
          settingsService
            .saveSettings({ base_url: baseUrlInput.trim(), token: tokenInput.trim() })
            .then(() => syncMutation.mutate());
        }}
        className="space-y-4"
      >
        <div>
          <label htmlFor="base-url" className="block text-sm font-medium text-gray-700 mb-1">
            Mealie server URL
          </label>
          <input
            id="base-url"
            type="url"
            required
            value={baseUrlInput}
            onChange={(e) => setBaseUrlInput(e.target.value)}
            placeholder="https://mealie.example.com"
            className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
        </div>

        <div>
          <label htmlFor="api-token" className="block text-sm font-medium text-gray-700 mb-1">
            API token
          </label>
          <input
            id="api-token"
            type="password"
            required
            value={tokenInput}
            onChange={(e) => setTokenInput(e.target.value)}
            placeholder="Long-lived API token"
            className="w-full rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
          />
          <p className="mt-1 text-xs text-gray-400">
            Create one on your server at /user/profile/api-tokens
          </p>
        </div>

        <button
          type="submit"
          disabled={!loaded || syncMutation.isPending}
          className="w-full flex items-center justify-center gap-2 rounded-lg bg-blue-600 text-white font-medium py-2.5 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {syncMutation.isPending && (
            <span className="w-4 h-4 border-2 border-white/40 border-t-white rounded-full animate-spin" />
          )}
          {syncMutation.isPending ? "Syncing…" : "Save & Sync Now"}
        </button>
      </form>

      {syncMutation.isError && (
        <p className="text-sm text-red-600 bg-red-50 rounded-lg p-3 break-words">
          Sync failed: {String(syncMutation.error)}
        </p>
      )}

      {lastReport && (
        <div className="bg-green-50 border border-green-100 rounded-lg p-3 text-sm text-green-800">
          <p className="font-semibold mb-1">Sync complete</p>
          <p>{lastReport.total_recipes} recipes indexed</p>
          <p>{lastReport.details_synced} offline details pulled</p>
          <p>{lastReport.images_downloaded} thumbnails cached</p>
          {lastReport.errors > 0 && <p className="text-red-600">{lastReport.errors} errors</p>}
        </div>
      )}

      <div className="bg-gray-50 rounded-lg p-3 text-sm text-gray-600 space-y-1">
        <p className="font-semibold text-gray-700 mb-1">Status</p>
        <p>Last sync: {formatTimestamp(status?.last_sync_at ?? null)}</p>
        <p>Recipes known: {status?.last_sync_count ?? 0}</p>
        <p className="break-all">Server: {status?.server_url ?? "not configured"}</p>
      </div>
    </div>
  );
}
