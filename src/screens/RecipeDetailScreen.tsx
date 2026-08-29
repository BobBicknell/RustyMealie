import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { dbService, RecipeSummary, settingsService, ShoppingList } from "../services/db";

interface RecipeData {
  name?: string;
  description?: string | null;
  slug?: string;
  recipeYield?: string;
  totalTime?: string;
  recipeIngredient?: unknown[];
  recipeInstructions?: unknown[];
  tags?: { name?: string }[];
  categories?: { name?: string }[];
  recipeCategory?: { name?: string }[];
}

function names(items?: { name?: string }[]): string[] {
  return (items ?? [])
    .map((item) => (item?.name ? String(item.name) : ""))
    .filter(Boolean);
}

function instructionText(step: unknown): string {
  if (typeof step === "string") return step;
  if (step && typeof step === "object") {
    const obj = step as Record<string, unknown>;
    const value = obj["text"] ?? obj["name"] ?? obj["title"];
    return typeof value === "string" ? value : "";
  }
  return "";
}

function ingredientText(item: unknown): string {
  if (typeof item === "string") return item;
  if (item && typeof item === "object") {
    const obj = item as Record<string, unknown>;
    for (const key of ["display", "originalText", "text", "note", "name"]) {
      const value = obj[key];
      if (typeof value === "string") return value;
    }
  }
  return "";
}

function listStrings(value: unknown[] | undefined, pick: (item: unknown) => string): string[] {
  return (Array.isArray(value) ? value : [])
    .map(pick)
    .map((item) => item.trim())
    .filter(Boolean);
}

function isPlaceholder(detail: RecipeData | null | undefined): boolean {
  if (!detail) return true;
  const keys = Object.keys(detail);
  return keys.length <= 1 || (!Array.isArray(detail.recipeInstructions) && !detail.name);
}

export function RecipeDetailScreen({
  recipe,
  onBack,
}: {
  recipe: RecipeSummary;
  onBack: () => void;
}) {
  const localQuery = useQuery<RecipeData | null>({
    queryKey: ["recipeDetail", recipe.id],
    queryFn: () => dbService.getRecipeDetail(recipe.id),
    staleTime: Infinity,
  });

  const serverMutation = useMutation<RecipeData, Error>({
    mutationFn: async () => {
      const settings = await settingsService.loadSettings();
      if (!settings.base_url || !settings.token) {
        throw new Error("No server configured. Connect in Settings first.");
      }
      if (!recipe.slug) {
        throw new Error("This recipe has no server slug available to fetch.");
      }
      return dbService.fetchRecipeDetail(
        settings.base_url,
        settings.token,
        recipe.id,
        recipe.slug
      );
    },
    onSuccess: async () => {
      await localQuery.refetch();
    },
  });

  useEffect(() => {
    if (
      localQuery.data !== undefined &&
      isPlaceholder(localQuery.data) &&
      !serverMutation.isPending &&
      !serverMutation.isSuccess &&
      !serverMutation.isError
    ) {
      serverMutation.mutate();
    }
  }, [localQuery.data]);

  const imageSrc = recipe.image_path ? convertFileSrc(recipe.image_path) : null;
  const isPending = localQuery.isPending || serverMutation.isPending;

  const queryClient = useQueryClient();
  const listsQuery = useQuery<ShoppingList[]>({
    queryKey: ["shoppingLists"],
    queryFn: () => dbService.getShoppingLists(),
  });
  const [choosingList, setChoosingList] = useState(false);

  const addToShopping = useMutation<ShoppingList, Error, string>({
    mutationFn: (listId) =>
      settingsService.loadSettings().then((settings) => {
        if (!settings.base_url || !settings.token) {
          throw new Error("No server configured. Connect in Settings first.");
        }
        return dbService.addRecipeToShoppingList(
          settings.base_url,
          settings.token,
          listId,
          recipe.id
        );
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["shoppingLists"] });
    },
  });

  const ensureLists = useMutation({
    mutationFn: () =>
      settingsService.loadSettings().then((settings) => {
        if (!settings.base_url || !settings.token) {
          throw new Error("No server configured. Connect in Settings first.");
        }
        return dbService.refreshShoppingLists(settings.base_url, settings.token);
      }),
    onSuccess: async () => {
      await queryClient.invalidateQueries({ queryKey: ["shoppingLists"] });
    },
  });

  // When the list picker opens with nothing cached, pull the shopping lists
  // from the server so the user actually has a list to pick from. `ensuredRef`
  // resets on each open so we fetch at most once per picker-open (even if the
  // server returns no lists).
  const ensuredRef = useRef(false);
  useEffect(() => {
    if (choosingList) {
      const lists = listsQuery.data;
      if ((!lists || lists.length === 0) && !ensuredRef.current && !ensureLists.isPending) {
        ensuredRef.current = true;
        ensureLists.mutate();
      }
    } else {
      ensuredRef.current = false;
    }
  }, [choosingList, listsQuery.data, ensureLists]);

  const detail: RecipeData | null = isPlaceholder(localQuery.data)
    ? null
    : (localQuery.data ?? null);
  const title = detail?.name || recipe.name;
  const instructions = listStrings(detail?.recipeInstructions, instructionText).flatMap(
    (text) =>
      text
        .split("\n")
        .map((line) => line.trim())
        .filter(Boolean)
  );
  const ingredients = listStrings(detail?.recipeIngredient, ingredientText);

  const detailCategories = detail
    ? [...names(detail.recipeCategory), ...names(detail.categories)]
    : [];
  const categories = detailCategories.length > 0 ? detailCategories : recipe.categories;
  const tags = detail?.tags ? names(detail.tags) : recipe.tags;

  return (
    <div className="min-h-screen bg-gray-50 text-gray-900 pb-16">
      <header className="flex items-center gap-3 p-4 bg-white border-b border-gray-200 sticky top-0 z-10">
        <button
          onClick={onBack}
          className="flex items-center justify-center w-8 h-8 rounded-lg bg-gray-100 text-gray-600 hover:bg-gray-200"
          aria-label="Back to recipes"
        >
          ←
        </button>
        <h1 className="font-semibold text-lg truncate">{title}</h1>
      </header>

      <main className="p-4 space-y-4">
        {imageSrc ? (
          <img
            src={imageSrc}
            alt={title}
            className="w-full rounded-lg object-cover max-h-72"
          />
        ) : (
          <div className="w-full h-48 rounded-lg bg-gray-200 flex items-center justify-center text-gray-400 text-5xl">
            🍽
          </div>
        )}

        {isPending && !detail && (
          <div className="flex justify-center py-12">
            <div className="w-8 h-8 border-4 border-gray-200 border-t-blue-500 rounded-full animate-spin" />
          </div>
        )}

        {serverMutation.isError && (
          <p className="text-sm text-red-600 bg-red-50 rounded-lg p-3 break-words">
            Could not load full details{recipe.image_path ? " (offline copy)" : ""}:{" "}
            {String(serverMutation.error)}
          </p>
        )}

        {detail && (
          <>
            {!imageSrc && detail.description && (
              <p className="text-gray-600">{detail.description}</p>
            )}

            {(detail.recipeYield || detail.totalTime) && (
              <div className="flex flex-wrap gap-3 text-sm text-gray-600">
                {detail.recipeYield && <span>Yield: {detail.recipeYield}</span>}
                {detail.totalTime && <span>Time: {detail.totalTime}</span>}
              </div>
            )}

            {categories.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {categories.map((category) => (
                  <span
                    key={category}
                    className="text-xs bg-amber-50 text-amber-700 rounded-full px-2.5 py-1"
                  >
                    {category}
                  </span>
                ))}
              </div>
            )}

            {tags.length > 0 && (
              <div className="flex flex-wrap gap-1.5">
                {tags.map((tag) => (
                  <span
                    key={tag}
                    className="text-xs bg-blue-50 text-blue-600 rounded-full px-2.5 py-1"
                  >
                    {tag}
                  </span>
                ))}
              </div>
            )}

            {ingredients.length > 0 && (
              <section>
                <div className="flex items-center justify-between mb-2">
                  <h2 className="font-semibold">Ingredients</h2>
                  <button
                    onClick={() => setChoosingList((v) => !v)}
                    className="flex items-center gap-1 text-xs font-medium bg-green-50 text-green-700 rounded-full px-3 py-1.5 hover:bg-green-100"
                  >
                    🛒 Add to list
                  </button>
                </div>
                <ul className="space-y-1.5 bg-white rounded-lg border border-gray-100 p-4">
                  {ingredients.map((ingredient, index) => (
                    <li key={index} className="flex gap-2 text-sm">
                      <span className="text-gray-300">•</span>
                      <span className="whitespace-pre-wrap">{ingredient}</span>
                    </li>
                  ))}
                </ul>

                {addToShopping.isPending && (
                  <p className="text-xs text-gray-500 mt-2">Adding ingredients…</p>
                )}
                {addToShopping.isSuccess && (
                  <p className="text-xs text-green-700 bg-green-50 rounded-lg p-2 mt-2">
                    Added to “{addToShopping.data?.name ?? "your list"}”.
                  </p>
                )}
                {addToShopping.isError && (
                  <p className="text-sm text-red-600 bg-red-50 rounded-lg p-2 mt-2 break-words">
                    {String(addToShopping.error)}
                  </p>
                )}

                {choosingList && (
                  <div
                    className="fixed inset-0 z-50 flex items-end justify-center bg-black/40"
                    onClick={() => setChoosingList(false)}
                  >
                    <div
                      className="w-full max-w-lg bg-white rounded-t-2xl p-4 pb-[calc(env(safe-area-inset-bottom)+1rem)] shadow-xl"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <div className="flex items-center justify-between mb-3">
                        <h3 className="font-semibold text-gray-900">Add to list</h3>
                        <button
                          onClick={() => setChoosingList(false)}
                          className="text-sm text-gray-400 px-2 py-1"
                        >
                          Cancel
                        </button>
                      </div>
                      {listsQuery.isPending && (
                        <p className="text-sm text-gray-400 py-4 text-center">Loading lists…</p>
                      )}
                      {(listsQuery.data ?? []).length === 0 &&
                        !ensureLists.isPending &&
                        !listsQuery.isPending && (
                          <p className="text-sm text-gray-500 py-4 text-center space-y-2">
                            <span className="block">No shopping lists cached.</span>
                            <button
                              onClick={() => ensureLists.mutate()}
                              disabled={ensureLists.isPending}
                              className="text-xs text-blue-600 font-medium disabled:opacity-50"
                            >
                              {ensureLists.isPending ? "Loading…" : "Fetch lists from server"}
                            </button>
                          </p>
                        )}
                      {ensureLists.isError && (
                        <p className="text-xs text-red-600 px-2 py-1 break-words">
                          {String(ensureLists.error)}
                        </p>
                      )}
                      <div className="space-y-1.5">
                        {(listsQuery.data ?? []).map((list) => (
                          <button
                            key={list.id}
                            onClick={() => {
                              setChoosingList(false);
                              addToShopping.mutate(list.id);
                            }}
                            disabled={addToShopping.isPending}
                            className="w-full text-left text-sm px-3 py-2.5 rounded-lg border border-gray-100 bg-gray-50 hover:bg-gray-100 disabled:opacity-50"
                          >
                            <span className="font-medium text-gray-800">{list.name}</span>{" "}
                            <span className="text-gray-400 text-xs">
                              ({list.items.length} item{list.items.length === 1 ? "" : "s"})
                            </span>
                          </button>
                        ))}
                      </div>
                      {addToShopping.isPending && (
                        <p className="text-xs text-gray-500 mt-3">Adding ingredients…</p>
                      )}
                    </div>
                  </div>
                )}
              </section>
            )}

            {instructions.length > 0 && (
              <section>
                <h2 className="font-semibold mb-2">Instructions</h2>
                <ol className="space-y-2">
                  {instructions.map((step, index) => (
                    <li key={index} className="flex gap-3 bg-white rounded-lg border border-gray-100 p-4 text-sm">
                      <span className="flex-shrink-0 flex items-center justify-center w-6 h-6 rounded-full bg-blue-600 text-white text-xs font-semibold">
                        {index + 1}
                      </span>
                      <span className="whitespace-pre-wrap">{step}</span>
                    </li>
                  ))}
                </ol>
              </section>
            )}

            {ingredients.length === 0 && instructions.length === 0 && (
              <p className="text-gray-400 text-sm">
                Full ingredients and instructions are only available for
                recipes marked for offline use after a sync.
              </p>
            )}
          </>
        )}

        {!detail && !isPending && !serverMutation.isError && (
          <p className="text-gray-500 text-sm">
            Loading recipe details…
          </p>
        )}
      </main>
    </div>
  );
}