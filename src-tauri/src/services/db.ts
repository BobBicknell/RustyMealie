import { invoke } from '@tauri-apps/api/core';

export interface RecipeSummary {
    id: string;
    name: string;
    description: string | null;
    image_path: string | null;
    tags: string[];
    marked_offline: boolean;
}

export const dbService = {
    /**
     * Fetch all recipes or filter them by a search query.
     */
    async getRecipes(query?: string): Promise<RecipeSummary[]> {
        try {
            return await invoke<RecipeSummary[]>('get_recipes', { query: query || null });
        } catch (error) {
            console.error('Failed to fetch recipes from local database:', error);
            throw error;
        }
    },

    /**
     * Fetch the full raw JSON payload for a single recipe details screen.
     */
    async getRecipeDetail(id: string): Promise<any | null> {
        try {
            const rawJson = await invoke<string | null>('get_recipe_detail', { id });
            return rawJson ? JSON.parse(rawJson) : null;
        } catch (error) {
            console.error(`Failed to fetch recipe detail for ID ${id}:`, error);
            throw error;
        }
    },

    /**
     * Toggle the offline availability flag for a recipe.
     */
    async toggleOfflineRecipe(id: string, offline: boolean): Promise<void> {
        try {
            await invoke('toggle_offline_recipe', { id, offline });
        } catch (error) {
            console.error(`Failed to toggle offline state for ID ${id}:`, error);
            throw error;
        }
    }
};
