import { useState } from "react";
import { RecipeListScreen } from "./screens/RecipeListScreen";
import { RecipeDetailScreen } from "./screens/RecipeDetailScreen";
import { SyncSettingsScreen } from "./screens/SyncSettingsScreen";
import { ShoppingListScreen } from "./screens/ShoppingListScreen";
import { RecipeSummary } from "./services/db";

type Tab = "recipes" | "shopping" | "settings";

const tabs: { id: Tab; label: string; icon: string }[] = [
  { id: "recipes", label: "Recipes", icon: "📖" },
  { id: "shopping", label: "Shopping", icon: "🛒" },
  { id: "settings", label: "Settings", icon: "⚙️" },
];

function App() {
  const [activeTab, setActiveTab] = useState<Tab>("recipes");
  const [openRecipe, setOpenRecipe] = useState<RecipeSummary | null>(null);

  return (
    <div className="min-h-screen flex flex-col bg-gray-50 text-gray-900">
      <main className="flex-1 overflow-y-auto pb-[calc(env(safe-area-inset-bottom)+4rem)]">
        {openRecipe ? (
          <RecipeDetailScreen recipe={openRecipe} onBack={() => setOpenRecipe(null)} />
        ) : activeTab === "recipes" ? (
          <RecipeListScreen onOpenRecipe={setOpenRecipe} />
        ) : activeTab === "shopping" ? (
          <ShoppingListScreen />
        ) : (
          <SyncSettingsScreen />
        )}
      </main>

      {!openRecipe && (
        <nav className="fixed bottom-0 inset-x-0 flex border-t border-gray-200 bg-white pb-[env(safe-area-inset-bottom)]">
          {tabs.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={`flex-1 py-2.5 text-xs font-medium transition-colors ${
                activeTab === tab.id ? "text-blue-600" : "text-gray-400"
              }`}
            >
              <span className="block text-lg leading-none mb-0.5">{tab.icon}</span>
              {tab.label}
            </button>
          ))}
        </nav>
      )}
    </div>
  );
}

export default App;
