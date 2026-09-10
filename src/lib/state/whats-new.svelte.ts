/**
 * Which new features this user has already looked at.
 *
 * The rule is simple and worth stating: an indicator disappears when the user
 * has *been shown the thing*, not when they have clicked the badge. Opening
 * the Recipes panel clears Recipes. Opening the MCP tab clears MCP. There is
 * nothing to dismiss, because a dismiss button is a second thing to do before
 * you get to the feature.
 *
 * Seen ids live in settings, so they survive a restart and travel with the
 * rest of the user's preferences.
 */

import { NEW_FEATURES, type FeatureSurface } from '$lib/data/whats-new';
import { getSettings, markFeaturesSeen } from '$lib/state/settings.svelte';

function seen(): string[] {
	return getSettings().seenFeatures;
}

function matches(surface: FeatureSurface, query: FeatureSurface): boolean {
	if (surface.kind !== query.kind) return false;
	if (surface.kind === 'sidebar' && query.kind === 'sidebar') {
		return surface.section === query.section;
	}
	if (surface.kind === 'settings' && query.kind === 'settings') {
		return surface.tab === query.tab;
	}
	return false;
}

/** Features on a surface that this user has not been shown yet. */
function unseenOn(query: FeatureSurface): typeof NEW_FEATURES {
	const already = seen();
	return NEW_FEATURES.filter((f) => matches(f.surface, query) && !already.includes(f.id));
}

/** Does this sidebar section have something new in it? */
export function sidebarHasNew(section: string): boolean {
	return unseenOn({ kind: 'sidebar', section }).length > 0;
}

/** Does this settings tab have something new in it? */
export function settingsTabHasNew(tab: string): boolean {
	return unseenOn({ kind: 'settings', tab }).length > 0;
}

/**
 * Anything new anywhere in Settings — the dot on the gear.
 *
 * The gear is the only route to those tabs, so without this the badge on the
 * MCP tab would only ever be seen by someone who already opened Settings for
 * another reason.
 */
export function settingsHaveNew(): boolean {
	const already = seen();
	return NEW_FEATURES.some((f) => f.surface.kind === 'settings' && !already.includes(f.id));
}

/** Which release a surface's newest feature arrived in, for the tooltip. */
export function newSince(query: FeatureSurface): string | null {
	const pending = unseenOn(query);
	return pending.length > 0 ? pending[0].since : null;
}

/** Mark everything on a surface as seen. Call it when the surface is shown. */
export function markSurfaceSeen(query: FeatureSurface): void {
	const ids = unseenOn(query).map((f) => f.id);
	if (ids.length > 0) markFeaturesSeen(ids);
}

export function markSidebarSeen(section: string): void {
	markSurfaceSeen({ kind: 'sidebar', section });
}

export function markSettingsTabSeen(tab: string): void {
	markSurfaceSeen({ kind: 'settings', tab });
}
