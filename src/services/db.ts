import { invoke } from '@tauri-apps/api/core';
import { LazyStore } from '@tauri-apps/plugin-store';

export interface RecipeSummary {
    id: string;
    slug: string | null;
    name: string;
    description: string | null;
    image_path: string | null;
    tags: string[];
    categories: string[];
    marked_offline: boolean;
}

export interface SyncReport {
    total_recipes: number;
    details_synced: number;
    images_downloaded: number;
    errors: number;
    finished_at: number;
}

export interface SyncProgress {
    phase: string;
    processed: number;
    total: number;
    message: string;
}

export interface SyncStatus {
    last_sync_at: number | null;
    last_sync_count: number | null;
    server_url: string | null;
}

export interface SyncSettings {
    base_url: string;
    token: string;
}

export interface ShoppingListItem {
    id: string;
    display: string;
    note: string | null;
    checked: boolean;
    position: number;
    category: string | null;
    category_color: string | null;
}

export interface ShoppingList {
    id: string;
    name: string;
    items: ShoppingListItem[];
}

export interface ShoppingListsSyncReport {
    lists: number;
    items: number;
    errors: number;
}

const settingsStore = new LazyStore('settings.json');

export const settingsService = {
    async loadSettings(): Promise<SyncSettings> {
        const base_url = (await settingsStore.get<string>('base_url')) ?? '';
        const token = (await settingsStore.get<string>('token')) ?? '';
        const settings = { base_url, token };
        // Push into the Rust core's in-memory credentials so mutating
        // commands don't need base_url/token passed on every call.
        if (base_url.trim() && token.trim()) {
            await settingsService.setCredentials(settings);
        }
        return settings;
    },

    async saveSettings(settings: SyncSettings): Promise<void> {
        await settingsStore.set('base_url', settings.base_url);
        await settingsStore.set('token', settings.token);
        await settingsStore.save();
        await settingsService.setCredentials(settings);
    },

    async setCredentials(settings: SyncSettings): Promise<void> {
        await invoke('set_credentials', {
            baseUrl: settings.base_url,
            token: settings.token,
        });
    },

    async getAppVersion(): Promise<string> {
        return await invoke<string>('get_app_version');
    },
};

export const dbService = {
    async getRecipes(query?: string, category?: string, tag?: string): Promise<RecipeSummary[]> {
        try {
            return await invoke<RecipeSummary[]>('get_recipes', {
                query: query || null,
                category: category || null,
                tag: tag || null,
            });
        } catch (error) {
            console.error('Failed to fetch recipes from local database:', error);
            throw error;
        }
    },

    async getRecipeDetail(id: string): Promise<any | null> {
        try {
            const rawJson = await invoke<string | null>('get_recipe_detail', { id });
            return rawJson ? JSON.parse(rawJson) : null;
        } catch (error) {
            console.error(`Failed to fetch recipe detail for ID ${id}:`, error);
            throw error;
        }
    },

    async fetchRecipeDetail(id: string, slug: string): Promise<any> {
        try {
            const rawJson = await invoke<string>('fetch_recipe_detail', { id, slug });
            return JSON.parse(rawJson);
        } catch (error) {
            console.error(`Failed to fetch recipe detail for ID ${id} from server:`, error);
            throw error;
        }
    },

    async toggleOfflineRecipe(id: string, offline: boolean): Promise<void> {
        try {
            await invoke('toggle_offline_recipe', { id, offline });
        } catch (error) {
            console.error(`Failed to toggle offline state for ID ${id}:`, error);
            throw error;
        }
    },

    async triggerSync(): Promise<SyncReport> {
        return await invoke<SyncReport>('trigger_sync');
    },

    async getSyncStatus(): Promise<SyncStatus> {
        return await invoke<SyncStatus>('get_sync_status');
    },

    async getShoppingLists(): Promise<ShoppingList[]> {
        try {
            return await invoke<ShoppingList[]>('get_shopping_lists');
        } catch (error) {
            console.error('Failed to fetch shopping lists from local database:', error);
            throw error;
        }
    },

    async refreshShoppingLists(): Promise<ShoppingListsSyncReport> {
        return await invoke<ShoppingListsSyncReport>('refresh_shopping_lists');
    },

    async toggleShoppingItem(
        listId: string,
        itemId: string,
        checked: boolean
    ): Promise<ShoppingList> {
        try {
            return await invoke<ShoppingList>('toggle_shopping_item', {
                listId,
                itemId,
                checked,
            });
        } catch (error) {
            console.error(`Failed to toggle shopping item ${itemId}:`, error);
            throw error;
        }
    },

    async addShoppingListItem(listId: string, note: string): Promise<ShoppingList> {
        try {
            return await invoke<ShoppingList>('add_shopping_list_item', {
                listId,
                note,
            });
        } catch (error) {
            console.error(`Failed to add shopping item to ${listId}:`, error);
            throw error;
        }
    },

    async addRecipeToShoppingList(listId: string, recipeId: string): Promise<ShoppingList> {
        try {
            return await invoke<ShoppingList>('add_recipe_to_shopping_list', {
                listId,
                recipeId,
            });
        } catch (error) {
            console.error(`Failed to add recipe ${recipeId} to list ${listId}:`, error);
            throw error;
        }
    },

    async clearCheckedShoppingItems(listId: string): Promise<ShoppingList> {
        try {
            return await invoke<ShoppingList>('clear_checked_shopping_items', {
                listId,
            });
        } catch (error) {
            console.error(`Failed to clear checked items from ${listId}:`, error);
            throw error;
        }
    },
};
