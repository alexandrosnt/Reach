/**
 * Reading `user@host:port` typed into a search or quick-connect field.
 *
 * Deliberately conservative: a session called "a.b" must stay a search,
 * never a connection. It takes one unmistakable sign of an address: a user,
 * a port, an IPv4 address, or a dotted name ending in something TLD-shaped.
 */

export interface TypedAddress {
	host: string;
	username?: string;
	port?: number;
}

const IPV4 = /^\d{1,3}(\.\d{1,3}){3}$/;

export function parseAddress(raw: string): TypedAddress | null {
	const text = raw.trim();
	if (!text || /\s/.test(text)) return null;
	let username: string | undefined;
	let rest = text;
	const at = text.lastIndexOf('@');
	if (at > 0) {
		username = text.slice(0, at);
		rest = text.slice(at + 1);
	}
	let port: number | undefined;
	const withPort = rest.match(/^(.+):(\d{1,5})$/);
	if (withPort) {
		rest = withPort[1];
		port = Number(withPort[2]);
	}
	if (!rest || !/^[a-z0-9][a-z0-9.-]*$/i.test(rest)) return null;
	const unmistakable = username !== undefined || port !== undefined || IPV4.test(rest) || /\.[a-z]{2,}$/i.test(rest);
	return unmistakable ? { host: rest, username, port } : null;
}
