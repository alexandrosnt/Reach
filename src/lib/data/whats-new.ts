/**
 * Features worth pointing at, and where to point.
 *
 * This is a curated list, not a changelog. An entry earns its place by being
 * something an existing user would not otherwise discover — a new sidebar
 * section, a new settings tab. Bug fixes and improvements to things people
 * already use do not belong here: a badge that is always lit stops meaning
 * anything, and then the one that matters gets ignored with the rest.
 *
 * **Remove entries once they are no longer new.** Roughly two releases is the
 * natural life of one. Nothing expires them automatically, because a version
 * comparison would guess wrong for anyone who skipped a release.
 *
 * A fresh install marks everything here as already seen. Someone meeting Reach
 * for the first time should not be told which parts of it are recent — all of
 * it is, and highlighting three arbitrary corners is noise.
 */

/** Where a feature lives, so the right control can carry the indicator. */
export type FeatureSurface =
	| { kind: 'sidebar'; section: string }
	| { kind: 'settings'; tab: string };

export interface NewFeature {
	/** Stable. Changing it re-announces the feature to everyone. */
	id: string;
	/** The release it arrived in. Shown in the tooltip. */
	since: string;
	surface: FeatureSurface;
}

export const NEW_FEATURES: NewFeature[] = [
	{ id: 'recipes', since: '0.5.2', surface: { kind: 'sidebar', section: 'recipes' } },
	{ id: 'mcp', since: '0.5.2', surface: { kind: 'settings', tab: 'mcp' } },
	{ id: 'community', since: '0.5.2', surface: { kind: 'settings', tab: 'general' } },
	{ id: 'sharing', since: '0.5.2', surface: { kind: 'settings', tab: 'sharing' } }
];

/** Every id, for seeding a fresh install. */
export const NEW_FEATURE_IDS: string[] = NEW_FEATURES.map((f) => f.id);
