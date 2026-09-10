/**
 * Recipe state.
 *
 * Installed recipes and the registry index are kept apart on purpose. They
 * answer different questions — "what can I run right now" and "what could I
 * install" — and merging them into one list made it impossible to tell, at a
 * glance, which scripts had already been vetted onto this machine.
 */

import * as ipc from '$lib/ipc/recipes';
import type { Danger, Recipe, RecipeEntry } from '$lib/ipc/recipes';
import { DANGER_ORDER } from '$lib/ipc/recipes';

let recipes = $state<Recipe[]>([]);
let registry = $state<RecipeEntry[]>([]);
let loading = $state(false);
let registryLoading = $state(false);
let error = $state<string | null>(null);
let registryError = $state<string | null>(null);

export function getRecipes(): Recipe[] {
	return recipes;
}

export function getRegistry(): RecipeEntry[] {
	return registry;
}

export function isLoading(): boolean {
	return loading;
}

export function isRegistryLoading(): boolean {
	return registryLoading;
}

export function getError(): string | null {
	return error;
}

export function getRegistryError(): string | null {
	return registryError;
}

/** Sort order for danger, so `worse(a, b)` reads the way it sounds. */
export function dangerRank(danger: Danger | null): number {
	return danger ? DANGER_ORDER.indexOf(danger) : -1;
}

export async function loadRecipes(): Promise<void> {
	loading = true;
	error = null;
	try {
		recipes = await ipc.recipeList();
	} catch (e) {
		error = String(e);
	} finally {
		loading = false;
	}
}

/**
 * Fetch the registry index.
 *
 * Errors are held rather than thrown: an unreachable registry must not empty
 * the list of recipes already installed, which are the ones someone is most
 * likely to need when the network is the thing that is broken.
 */
export async function loadRegistry(url?: string): Promise<void> {
	registryLoading = true;
	registryError = null;
	try {
		registry = await ipc.recipeFetchRegistry(url);
	} catch (e) {
		registryError = String(e);
	} finally {
		registryLoading = false;
	}
}

export async function saveRecipe(source: string): Promise<Recipe> {
	const saved = await ipc.recipeSave(source);
	await loadRecipes();
	return saved;
}

export async function deleteRecipe(id: string): Promise<void> {
	await ipc.recipeDelete(id);
	await loadRecipes();
}

export async function installRecipe(entry: RecipeEntry): Promise<Recipe> {
	const installed = await ipc.recipeInstall(entry);
	await loadRecipes();
	return installed;
}

/** Whether a registry entry is already on disk, so the button can say so. */
export function isInstalled(id: string): boolean {
	return recipes.some((r) => r.id === id);
}
