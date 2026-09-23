/**
 * The Discord server, looked up rather than written down.
 *
 * A hardcoded member count is wrong the day after it is written, and a
 * hardcoded invite is one revoked link away from being a dead end. Discord's
 * invite endpoint answers both from the invite code alone — no bot, no token,
 * no application — so the page can state what is actually true.
 *
 * Fetched while the site builds, so the numbers are right with JavaScript
 * disabled and nothing depends on the reader's browser reaching Discord. The
 * endpoint also sends `access-control-allow-origin` for this site, so the
 * component refreshes the counts client side on top — the build gives a
 * correct floor, the browser makes it current.
 *
 * Nothing here throws. With Discord unreachable the invite link still works,
 * because the link is the one thing we already know without asking.
 */

/** The permanent invite. Confirmed non-expiring by the API below. */
export const DISCORD_INVITE_CODE = 'CSbEybvDVV';
export const DISCORD_URL = `https://discord.gg/${DISCORD_INVITE_CODE}`;

export interface DiscordServer {
	name: string;
	/** Everyone who has joined. */
	members: number | null;
	/** Everyone currently online — the number that says whether it is alive. */
	online: number | null;
}

const ENDPOINT =
	`https://discord.com/api/v10/invites/${DISCORD_INVITE_CODE}` +
	'?with_counts=true&with_expiration=true';

let pending: Promise<DiscordServer> | undefined;

/** Looked up once per build, not once per component. */
export function discordServer(): Promise<DiscordServer> {
	pending ??= load();
	return pending;
}

async function load(): Promise<DiscordServer> {
	try {
		const response = await fetch(ENDPOINT, { headers: { Accept: 'application/json' } });
		if (!response.ok) {
			console.warn(`[discord] API returned ${response.status}; counts will be omitted.`);
			return { name: 'Reach', members: null, online: null };
		}

		const invite = (await response.json()) as {
			guild?: { name?: string };
			approximate_member_count?: number;
			approximate_presence_count?: number;
			expires_at?: string | null;
		};

		// Worth knowing at build time rather than from a confused reader: an
		// invite with an expiry will stop working and nothing on the page can
		// tell you it has.
		if (invite.expires_at) {
			console.warn(
				`[discord] The invite expires on ${invite.expires_at}. ` +
					'Generate a permanent one in Server Settings > Invites.',
			);
		}

		return {
			name: invite.guild?.name ?? 'Reach',
			members: invite.approximate_member_count ?? null,
			online: invite.approximate_presence_count ?? null,
		};
	} catch (error) {
		console.warn('[discord] could not reach the API:', error);
		return { name: 'Reach', members: null, online: null };
	}
}
