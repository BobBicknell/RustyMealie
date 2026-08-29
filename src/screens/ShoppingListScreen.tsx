import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  dbService,
  settingsService,
  ShoppingList,
  ShoppingListItem,
  SyncSettings,
} from "../services/db";

function AddItemForm({
  listId,
  onAdd,
  disabled,
}: {
  listId: string;
  onAdd: (listId: string, note: string) => void;
  disabled: boolean;
}) {
  const [text, setText] = useState("");
  const trimmed = text.trim();
  return (
    <form
      className="flex gap-2 px-3 pb-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (!trimmed) return;
        onAdd(listId, trimmed);
        setText("");
      }}
    >
      <input
        type="text"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Add an item…"
        className="flex-1 rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
      />
      <button
        type="submit"
        disabled={disabled || !trimmed}
        className="rounded-lg bg-blue-600 text-white text-sm font-medium px-3 py-2 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
      >
        Add
      </button>
    </form>
  );
}

function ItemRow({
  item,
  onToggle,
  disabled,
}: {
  item: ShoppingListItem;
  onToggle: (item: ShoppingListItem, checked: boolean) => void;
  disabled: boolean;
}) {
  return (
    <button
      onClick={() => onToggle(item, !item.checked)}
      disabled={disabled}
      className="w-full flex items-start gap-3 text-left py-2 px-1 rounded hover:bg-gray-50 active:bg-gray-100 disabled:opacity-60"
    >
      <input
        type="checkbox"
        checked={item.checked}
        onChange={(e) => onToggle(item, e.target.checked)}
        onClick={(e) => e.stopPropagation()}
        className="mt-0.5 h-4 w-4 rounded border-gray-300 text-blue-600 focus:ring-blue-400"
      />
      <span className="min-w-0 flex-1">
        <span
          className={`block text-sm leading-snug ${
            item.checked ? "text-gray-400 line-through" : "text-gray-800"
          }`}
        >
          {item.display || "Untitled item"}
        </span>
        {item.note && (
          <span className="block text-xs text-gray-500 mt-0.5">{item.note}</span>
        )}
      </span>
    </button>
  );
}

export function ShoppingListScreen() {
  const queryClient = useQueryClient();
  const [settings, setSettings] = useState<SyncSettings | null>(null);

  useEffect(() => {
    let cancelled = false;
    settingsService.loadSettings().then((loaded) => {
      if (cancelled) return;
      setSettings(loaded);
      // Keep the list fresh from the server when this screen first opens with
      // network available. A recipe added on another screen (or an add whose
      // local write was interrupted) can otherwise leave the cached list empty
      // while the server has items. Exactly-once per mount.
      if (loaded.base_url.trim() && loaded.token.trim()) {
        refreshMutation.mutate(loaded);
      }
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const hasNetwork =
    Boolean(settings) && Boolean(settings!.base_url.trim()) && Boolean(settings!.token.trim());

  const refreshMutation = useMutation({
    mutationFn: (s: SyncSettings) => dbService.refreshShoppingLists(s.base_url, s.token),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["shoppingLists"] }),
  });

  const { data: lists, isPending, isError, error } = useQuery<ShoppingList[]>({
    queryKey: ["shoppingLists"],
    queryFn: () => dbService.getShoppingLists(),
  });

  const toggleMutation = useMutation({
    mutationFn: ({
      listId,
      item,
      checked,
    }: {
      listId: string;
      item: ShoppingListItem;
      checked: boolean;
    }) =>
      dbService.toggleShoppingItem(
        settings!.base_url,
        settings!.token,
        listId,
        item.id,
        checked
      ),
    onMutate: async ({ listId, item, checked }) => {
      await queryClient.cancelQueries({ queryKey: ["shoppingLists"] });
      const previous = queryClient.getQueryData<ShoppingList[]>(["shoppingLists"]);
      queryClient.setQueryData<ShoppingList[]>(["shoppingLists"], (old) =>
        (old ?? []).map((list) =>
          list.id === listId
            ? {
                ...list,
                items: list.items.map((i) =>
                  i.id === item.id ? { ...i, checked } : i
                ),
              }
            : list
        )
      );
      return { previous };
    },
    onError: (_err, _vars, context) => {
      if (context?.previous) {
        queryClient.setQueryData(["shoppingLists"], context.previous);
      }
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["shoppingLists"] }),
  });

  const refresh = () => {
    if (!hasNetwork || refreshMutation.isPending) return;
    refreshMutation.mutate(settings!);
  };

  const addMutation = useMutation({
    mutationFn: ({ listId, note }: { listId: string; note: string }) =>
      dbService.addShoppingListItem(
        settings!.base_url,
        settings!.token,
        listId,
        note
      ),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ["shoppingLists"] }),
    onError: (err) => console.error("Failed to add shopping item:", err),
  });

  const pendingCount = (list: ShoppingList) =>
    list.items.filter((item) => !item.checked).length;

  return (
    <div className="p-4 space-y-4">
      <div className="flex items-center justify-between">
        <h1 className="text-lg font-semibold text-gray-900">Shopping</h1>
        <button
          onClick={refresh}
          disabled={!hasNetwork || refreshMutation.isPending}
          className="flex items-center gap-1.5 rounded-lg bg-blue-600 text-white text-sm font-medium px-3 py-1.5 hover:bg-blue-700 disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {refreshMutation.isPending && (
            <span className="w-3.5 h-3.5 border-2 border-white/40 border-t-white rounded-full animate-spin" />
          )}
          {refreshMutation.isPending ? "Refreshing…" : "Refresh"}
        </button>
      </div>

      {!hasNetwork && !isPending && (
        <p className="text-center text-gray-400 text-sm py-8">
          Connect your Mealie server in Settings to sync shopping lists.
        </p>
      )}

      {refreshMutation.isError && (
        <p className="text-sm text-red-600 bg-red-50 rounded-lg p-3 break-words">
          Sync failed: {String(refreshMutation.error)}
        </p>
      )}

      {isError && (
        <p className="text-red-600 text-sm bg-red-50 rounded-lg p-3">
          Failed to load shopping lists: {String(error)}
        </p>
      )}

      {isPending && (
        <div className="flex justify-center py-12">
          <div className="w-8 h-8 border-4 border-gray-200 border-t-blue-500 rounded-full animate-spin" />
        </div>
      )}

      {lists && lists.length === 0 && hasNetwork && (
        <p className="text-center text-gray-400 text-sm py-12">
          No shopping lists yet.
        </p>
      )}

      {lists && lists.length > 0 && (
        <div className="space-y-4">
          {lists.map((list) => {
            const pending = pendingCount(list);
            const done = list.items.filter((item) => item.checked).length;
            return (
              <section
                key={list.id}
                className="bg-white rounded-lg shadow-sm border border-gray-100 overflow-hidden"
              >
                <header className="flex items-center justify-between px-3 py-2.5 border-b border-gray-100">
                  <h2 className="font-semibold text-gray-900 truncate">{list.name}</h2>
                  <span className="text-xs font-medium text-gray-500 whitespace-nowrap">
                    {list.items.length === 0
                      ? "Empty"
                      : pending === 0
                        ? `${done} done ✓`
                        : `${pending} left`}
                  </span>
                </header>
                {list.items.length === 0 ? (
                  <p className="text-sm text-gray-400 px-3 py-4 text-center">
                    No items in this list.
                  </p>
                ) : (
                  <ul className="px-3 pt-1.5">
                    {list.items.map((item) => (
                      <li key={item.id}>
                        <ItemRow
                          item={item}
                          onToggle={(i, c) =>
                            toggleMutation.mutate({
                              listId: list.id,
                              item: i,
                              checked: c,
                            })
                          }
                          disabled={!hasNetwork || toggleMutation.isPending}
                        />
                      </li>
                    ))}
                  </ul>
                )}
                <div className="border-t border-gray-100 pt-2 mt-1">
                  <AddItemForm
                    listId={list.id}
                    onAdd={(listId, note) => addMutation.mutate({ listId, note })}
                    disabled={!hasNetwork || addMutation.isPending}
                  />
                </div>
              </section>
            );
          })}
        </div>
      )}
    </div>
  );
}