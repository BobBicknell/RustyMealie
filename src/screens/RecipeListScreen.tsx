import { useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { convertFileSrc } from "@tauri-apps/api/core";
import { dbService, RecipeSummary } from "../services/db";

function RecipeCard({
  recipe,
  onOpen,
}: {
  recipe: RecipeSummary;
  onOpen: (recipe: RecipeSummary) => void;
}) {
  const imageSrc = recipe.image_path ? convertFileSrc(recipe.image_path) : null;

  return (
    <li>
      <button
        onClick={() => onOpen(recipe)}
        className="w-full text-left flex items-center gap-3 bg-white rounded-lg shadow-sm border border-gray-100 p-3 hover:bg-gray-50 active:bg-gray-100 transition-colors"
      >
        {imageSrc ? (
          <img
            src={imageSrc}
            alt={recipe.name}
            className="w-14 h-14 rounded-md object-cover flex-shrink-0"
          />
        ) : (
          <div className="w-14 h-14 rounded-md bg-gray-200 flex items-center justify-center text-gray-400 text-xl flex-shrink-0">
            🍽
          </div>
        )}
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h2 className="font-semibold text-gray-900 truncate">{recipe.name}</h2>
            {recipe.marked_offline && (
              <span className="text-[10px] font-bold uppercase tracking-wide bg-green-100 text-green-700 rounded-full px-2 py-0.5 whitespace-nowrap">
                Offline
              </span>
            )}
          </div>
          {recipe.description && (
            <p className="text-sm text-gray-500 line-clamp-1">{recipe.description}</p>
          )}
          {recipe.categories.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {recipe.categories.map((category) => (
                <span
                  key={category}
                  className="text-xs bg-amber-50 text-amber-700 rounded px-1.5 py-0.5"
                >
                  {category}
                </span>
              ))}
            </div>
          )}
          {recipe.tags.length > 0 && (
            <div className="flex flex-wrap gap-1 mt-1">
              {recipe.tags.map((tag) => (
                <span
                  key={tag}
                  className="text-xs bg-blue-50 text-blue-600 rounded px-1.5 py-0.5"
                >
                  {tag}
                </span>
              ))}
            </div>
          )}
        </div>
        <span className="text-gray-300 flex-shrink-0">›</span>
      </button>
    </li>
  );
}

function FilterChip({
  label,
  active,
  onClick,
}: {
  label: string;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`text-xs rounded-full px-3 py-1 border whitespace-nowrap transition-colors ${
        active
          ? "bg-blue-600 text-white border-blue-600"
          : "bg-white text-gray-600 border-gray-200 hover:bg-gray-50"
      }`}
    >
      {label}
    </button>
  );
}

export function RecipeListScreen({
  onOpenRecipe,
}: {
  onOpenRecipe: (recipe: RecipeSummary) => void;
}) {
  const [query, setQuery] = useState("");
  const [searchInput, setSearchInput] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [tag, setTag] = useState<string | null>(null);

  const { data: allRecipes } = useQuery<RecipeSummary[]>({
    queryKey: ["allRecipes"],
    queryFn: () => dbService.getRecipes(),
  });

  const { data: recipes, isPending, isError, error } = useQuery({
    queryKey: ["recipes", query, category, tag],
    queryFn: () => dbService.getRecipes(query, category ?? undefined, tag ?? undefined),
  });

  const { categories, tags } = useMemo(() => {
    const categorySet = new Set<string>();
    const tagSet = new Set<string>();
    for (const recipe of allRecipes ?? []) {
      for (const c of recipe.categories) categorySet.add(c);
      for (const t of recipe.tags) tagSet.add(t);
    }
    return {
      categories: [...categorySet].sort((a, b) => a.localeCompare(b)),
      tags: [...tagSet].sort((a, b) => a.localeCompare(b)),
    };
  }, [allRecipes]);

  const hasFilters = Boolean(category || tag);

  return (
    <div className="p-4 space-y-4">
      <form
        onSubmit={(e) => {
          e.preventDefault();
          setQuery(searchInput.trim());
        }}
        className="flex gap-2"
      >
        <input
          type="search"
          value={searchInput}
          onChange={(e) => setSearchInput(e.target.value)}
          placeholder="Search recipes, tags or categories…"
          className="flex-1 rounded-lg border border-gray-300 px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-blue-400"
        />
        <button
          type="submit"
          className="rounded-lg bg-blue-600 text-white text-sm font-medium px-4 hover:bg-blue-700"
        >
          Search
        </button>
      </form>

      {categories.length > 0 && (
        <div>
          <p className="text-xs font-semibold text-gray-400 uppercase mb-1.5">Category</p>
          <div className="flex gap-1.5 overflow-x-auto pb-1">
            {categories.map((c) => (
              <FilterChip
                key={c}
                label={c}
                active={category === c}
                onClick={() => setCategory(category === c ? null : c)}
              />
            ))}
          </div>
        </div>
      )}

      {tags.length > 0 && (
        <div>
          <p className="text-xs font-semibold text-gray-400 uppercase mb-1.5">Tag</p>
          <div className="flex gap-1.5 overflow-x-auto pb-1">
            {tags.map((t) => (
              <FilterChip
                key={t}
                label={t}
                active={tag === t}
                onClick={() => setTag(tag === t ? null : t)}
              />
            ))}
          </div>
        </div>
      )}

      {hasFilters && (
        <div className="flex items-center gap-2 text-sm">
          <span className="bg-gray-100 text-gray-600 rounded-full px-2.5 py-0.5">
            {recipes?.length ?? 0} result{recipes?.length === 1 ? "" : "s"}
          </span>
          <button
            onClick={() => {
              setCategory(null);
              setTag(null);
            }}
            className="text-blue-600 text-xs font-medium"
          >
            Clear filters
          </button>
        </div>
      )}

      {isPending && (
        <div className="flex justify-center py-12">
          <div className="w-8 h-8 border-4 border-gray-200 border-t-blue-500 rounded-full animate-spin" />
        </div>
      )}

      {isError && (
        <p className="text-red-600 text-sm bg-red-50 rounded-lg p-3">
          Failed to load recipes: {String(error)}
        </p>
      )}

      {recipes && recipes.length === 0 && (
        <p className="text-center text-gray-400 text-sm py-12">
          {hasFilters ? "No recipes match these filters." : "No recipes yet. Connect and sync in Settings."}
        </p>
      )}

      {recipes && recipes.length > 0 && (
        <ul className="space-y-2">
          {recipes.map((recipe) => (
            <RecipeCard key={recipe.id} recipe={recipe} onOpen={onOpenRecipe} />
          ))}
        </ul>
      )}
    </div>
  );
}