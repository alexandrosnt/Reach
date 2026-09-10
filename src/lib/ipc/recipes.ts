/**
 * Recipe IPC.
 *
 * A recipe is a bash script that carries its own metadata, takes parameters,
 * and runs against the session you have open. Nothing here executes anything:
 * `recipePrepare` returns the exact text that would be written, and sending it
 * is a separate, deliberate step taken by the caller.
 */

import { invoke } from '@tauri-apps/api/core';

/** Ordered: each level is worse than the one before it. */
export type Danger = 'benign' | 'mutating' | 'sensitive' | 'destructive';

export const DANGER_ORDER: Danger[] = ['benign', 'mutating', 'sensitive', 'destructive'];

export interface RecipeParam {
	/** Becomes an environment variable, so it is always [A-Z_][A-Z0-9_]*. */
	name: string;
	label: string;
	/** Empty means the parameter is required. */
	default: string;
}

/** Where a recipe came from, which is the same question as how far to trust it. */
export type Origin =
	| { kind: 'local' }
	| { kind: 'registry'; repo: string; sha256: string };

/** One line the analyser objected to. */
export interface Finding {
	/** 1-indexed against the script body, matching what the editor shows. */
	line: number;
	text: string;
	danger: Danger;
	reason: string;
}

export interface Analysis {
	/**
	 * The level to show and gate on: the worse of what the author declared
	 * and what the scan found. An author who says `sensitive` is believed
	 * even when the scan sees nothing, because a path behind a variable is
	 * invisible to it and not to them.
	 */
	danger: Danger;
	/** What the scan alone found. `benign` means nothing matched — not that it is safe. */
	found: Danger;
	findings: Finding[];
	/** What the author declared in the header, if anything. */
	declared: Danger | null;
	/** Analysis found something worse than the author admitted to. */
	understated: boolean;
	/** Lines that fetch code and run it, whose contents nothing can inspect. */
	opaque: number[];
}

export interface Recipe {
	id: string;
	name: string;
	description: string;
	version: string;
	author: string;
	tags: string[];
	targets: string[];
	declaredDanger: Danger | null;
	params: RecipeParam[];
	/** The body, without the metadata header. */
	script: string;
	/** The whole file as written. This is what the editor shows. */
	source: string;
	origin: Origin;
	analysis: Analysis;
}

/** One entry in the registry index. */
export interface RecipeEntry {
	id: string;
	name: string;
	version: string;
	description: string;
	author: string;
	repo: string;
	tags: string[];
	targets: string[];
	danger: Danger | null;
	url: string;
	sha256: string;
}

/** What a run would send, without sending it. */
export interface PreparedRun {
	/** The exact bytes to write into the session. */
	command: string;
	delimiter: string;
	/** Re-analysed at prepare time — the file may have changed since listing. */
	analysis: Analysis;
}

export async function recipeList(): Promise<Recipe[]> {
	return invoke<Recipe[]>('recipe_list');
}

export async function recipeGet(id: string): Promise<Recipe> {
	return invoke<Recipe>('recipe_get', { id });
}

/** Parse without saving, so the editor can report errors while typing. */
export async function recipePreview(source: string): Promise<Recipe> {
	return invoke<Recipe>('recipe_preview', { source });
}

export async function recipeSave(source: string): Promise<Recipe> {
	return invoke<Recipe>('recipe_save', { source });
}

export async function recipeDelete(id: string): Promise<void> {
	return invoke('recipe_delete', { id });
}

export async function recipeTemplate(id: string, name: string): Promise<string> {
	return invoke<string>('recipe_template', { id, name });
}

export async function recipePrepare(
	id: string,
	values: Record<string, string>
): Promise<PreparedRun> {
	return invoke<PreparedRun>('recipe_prepare', { id, values });
}

export async function recipeFetchRegistry(url?: string): Promise<RecipeEntry[]> {
	return invoke<RecipeEntry[]>('recipe_fetch_registry', { url: url ?? null });
}

export async function recipeInstall(entry: RecipeEntry): Promise<Recipe> {
	return invoke<Recipe>('recipe_install', { entry });
}

export async function recipeRegistryUrl(): Promise<string> {
	return invoke<string>('recipe_registry_url');
}
