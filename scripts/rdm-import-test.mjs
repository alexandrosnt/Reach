/**
 * Tests reading hosts out of a Devolutions Remote Desktop Manager export
 * (.rdm / .xml and .json) for the RDM importer, in Node.
 *
 * Node strips TypeScript types on import, so the exact module the webview
 * runs is what is under test here.
 *
 * Run: node scripts/rdm-import-test.mjs
 */

import { parseRdmExport, parseXml, splitAddress, splitGroup, findExisting, findChildFolder, sessionUser } from '../src/lib/sessions/rdm-import.ts';

let failures = 0;
const check = (name, cond, extra = '') => {
	if (cond) console.log(`  PASS  ${name}`);
	else {
		failures++;
		console.log(`  FAIL  ${name}${extra ? '\n        ' + extra : ''}`);
	}
};
const section = (s) => console.log(`\n${s}`);

const RDM_XML = `<?xml version="1.0" encoding="utf-8"?>
<!-- exported by RDM -->
<ArrayOfConnection xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance">
  <Connection>
    <ConnectionType>Group</ConnectionType>
    <Group>HQ</Group>
    <ID>11111111-1111-1111-1111-111111111111</ID>
    <Name>HQ</Name>
  </Connection>
  <Connection>
    <ConnectionType>Credential</ConnectionType>
    <ID>CCCCCCCC-0000-0000-0000-000000000000</ID>
    <Name>Domain admin</Name>
    <Credentials>
      <UserName>svc-admin</UserName>
      <Domain>CORP</Domain>
      <SafePassword>c2VjcmV0</SafePassword>
    </Credentials>
  </Connection>
  <Connection>
    <ConnectionType>RDPConfigured</ConnectionType>
    <Group>HQ\\Servers</Group>
    <ID>22222222-2222-2222-2222-222222222222</ID>
    <Name>DC01 &amp; friends</Name>
    <Url>dc01.corp.local:3390</Url>
    <RDP>
      <UserName>admin</UserName>
      <Domain>CORP</Domain>
      <SafePassword>c2VjcmV0</SafePassword>
    </RDP>
  </Connection>
  <Connection>
    <ConnectionType>RDPConfigured</ConnectionType>
    <Group>HQ</Group>
    <Name>FS01</Name>
    <Url>10.0.0.20</Url>
    <CredentialConnectionID>cccccccc-0000-0000-0000-000000000000</CredentialConnectionID>
  </Connection>
  <Connection>
    <ConnectionType>SSHShell</ConnectionType>
    <Group>Linux</Group>
    <Name>web1</Name>
    <Terminal>
      <Host>web1.example.com</Host>
      <HostPort>2222</HostPort>
      <Username>deploy</Username>
      <PrivateKeyPath>C:\\Users\\me\\.ssh\\id_ed25519</PrivateKeyPath>
      <SafePassword>c2VjcmV0</SafePassword>
    </Terminal>
  </Connection>
  <Connection>
    <ConnectionType>VNC</ConnectionType>
    <Name>Kiosk</Name>
    <VNC><Host>kiosk.local</Host><Port>0</Port></VNC>
  </Connection>
  <Connection>
    <ConnectionType>WebBrowser</ConnectionType>
    <Name>Portal</Name>
    <Url>https://portal.example.com</Url>
  </Connection>
  <Connection>
    <ConnectionType>SSHShell</ConnectionType>
    <Name>Nowhere</Name>
    <Terminal />
  </Connection>
  <Connection>
    <ConnectionType>SSHShell</ConnectionType>
    <Name>Bad port</Name>
    <Terminal><Host>x</Host><HostPort>70000</HostPort></Terminal>
  </Connection>
</ArrayOfConnection>`;

// Shaped like a real RDM 2026.1 "Export .rdm without credentials": no
// usernames anywhere, an SSH gateway kept as a VPN entry, a tunnel entry.
const RDM_2026 = `<?xml version="1.0" encoding="utf-8"?>
<RDMExport>
  <Connections>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>Group</ConnectionType>
      <Group>SSH\\Lab</Group>
      <ID>aaaaaaaa-0000-0000-0000-000000000001</ID>
      <Name>Lab</Name>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>VPN</ConnectionType>
      <Group>SSH\\Lab</Group>
      <ID>aaaaaaaa-0000-0000-0000-0000000000AA</ID>
      <Name>Lab jump gateway</Name>
      <VPN>
        <Application>SSHGateway</Application>
        <SSHGateways>
          <SSHGateway>
            <Host>gw.example.net</Host>
            <ID>aaaaaaaa-0000-0000-0000-000000000003</ID>
            <Port>1022</Port>
            <PrivateKeyType>Link</PrivateKeyType>
          </SSHGateway>
        </SSHGateways>
      </VPN>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>SSHShell</ConnectionType>
      <Group>SSH\\Lab</Group>
      <ID>aaaaaaaa-0000-0000-0000-000000000004</ID>
      <Name>router (admin)</Name>
      <Terminal>
        <Host>router.example.net</Host>
        <HostPort>22</HostPort>
        <TryAgent>true</TryAgent>
      </Terminal>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>SSHShell</ConnectionType>
      <Group>SSH\\Lab</Group>
      <ID>aaaaaaaa-0000-0000-0000-000000000005</ID>
      <Name>inside</Name>
      <Terminal>
        <Host>192.0.2.10</Host>
        <HostPort>22</HostPort>
        <LinkedSSHGateway>aaaaaaaa-0000-0000-0000-0000000000aa</LinkedSSHGateway>
        <SSHGatewayMode>Linked</SSHGatewayMode>
        <UseSSHGateway>true</UseSSHGateway>
      </Terminal>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>SSHTunnel</ConnectionType>
      <Group>SSH\\Lab</Group>
      <Name>Lab tunnel</Name>
      <Terminal><Host>gw.example.net</Host><HostPort>1022</HostPort></Terminal>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>RDPConfigured</ConnectionType>
      <Group>RDP\\Corp</Group>
      <Name>DC (AD)</Name>
      <Url>198.51.100.4</Url>
      <RDP><ScreenSizingMode>FitToWindow</ScreenSizingMode></RDP>
    </Connection>
    <Connection>
      <AppVersion>2026.1</AppVersion>
      <ConnectionType>Credential</ConnectionType>
      <Name>domain account</Name>
      <Credentials />
    </Connection>
  </Connections>
</RDMExport>`;

section('XML reader');
{
	const root = parseXml('<a x="1>2"><b>one</b><b>two</b><c/><d><![CDATA[<raw>]]></d></a>');
	const a = root.children[0];
	check('nesting and attributes with > in quotes', a.name === 'a' && a.children.length === 4);
	check('CDATA is kept as text', a.children[3].text === '<raw>');
	let threw = false;
	try {
		parseXml('<a><b></a>');
	} catch {
		threw = true;
	}
	check('mismatched tags are an error', threw);
}

section('.rdm export (XML)');
{
	const r = parseRdmExport(RDM_XML);
	check('the file is read', r.ok, JSON.stringify(r));
	const byName = Object.fromEntries((r.ok ? r.hosts : []).map((h) => [h.name, h]));
	check('groups and credentials are not hosts', r.ok && r.hosts.length === 4, r.ok ? r.hosts.map((h) => h.name).join(', ') : '');

	const dc = byName['DC01 & friends'];
	check('entities are decoded in names', !!dc);
	check('RDP host and port come from the Url', dc && dc.kind === 'rdp' && dc.host === 'dc01.corp.local' && dc.port === 3390);
	check('RDP username and domain come across', dc && dc.username === 'admin' && dc.domain === 'CORP');
	check('the group path is split on backslashes', dc && dc.folder.join('/') === 'HQ/Servers');

	const fs = byName['FS01'];
	check('RDP defaults to 3389', fs && fs.port === 3389);
	check('a linked credential lends its username and domain (ID case ignored)', fs && fs.username === 'svc-admin' && fs.domain === 'CORP');

	const web = byName['web1'];
	check('SSH reads the Terminal block', web && web.kind === 'ssh' && web.host === 'web1.example.com' && web.port === 2222 && web.username === 'deploy');
	check('SSH keeps a private key path', web && web.keyPath === 'C:\\Users\\me\\.ssh\\id_ed25519');
	check('SSH has no domain', web && web.domain === null);

	const kiosk = byName['Kiosk'];
	check('VNC port 0 means the default 5900', kiosk && kiosk.kind === 'vnc' && kiosk.port === 5900 && kiosk.folder.length === 0);

	const skipped = r.ok ? r.skipped : [];
	check('unsupported types are listed with their type', skipped.some((s) => s.label === 'Portal' && s.reason === 'unsupported' && s.type === 'WebBrowser'));
	check('an entry without a host is skipped', skipped.some((s) => s.label === 'Nowhere' && s.reason === 'no_host'));
	check('an out-of-range port is skipped', skipped.some((s) => s.label === 'Bad port' && s.reason === 'bad_port'));
	check('no password ever appears in a host', !JSON.stringify(r).includes('c2VjcmV0'));
}

section('.json export');
{
	const json = JSON.stringify({
		Connections: [
			{ ConnectionType: 'SSHShell', Name: 'db', Group: 'Prod/DB', Terminal: { Host: 'db.internal', Username: 'root', Password: 'nope' } },
			{ ConnectionType: 'RDPConfigured', Name: 'jump', Url: '[fe80::1]:3391', RDP: { UserName: 'u' } },
			{ ConnectionType: 'Putty', Name: 'telnet box', Url: 'old.local', Putty: { Protocol: 'Telnet' } },
			{ ConnectionType: 'Putty', Name: 'putty ssh', Url: 'p.local', Putty: { Protocol: 'Ssh', Port: 2200, Username: 'pu' } },
			{ ConnectionType: 1, Name: 'numeric rdp', Url: 'n.local', RDP: {} }
		]
	});
	const r = parseRdmExport(json);
	check('the file is read', r.ok, JSON.stringify(r));
	const byName = Object.fromEntries((r.ok ? r.hosts : []).map((h) => [h.name, h]));
	check('SSH from JSON, default port 22', byName.db && byName.db.port === 22 && byName.db.username === 'root' && byName.db.folder.join('/') === 'Prod/DB');
	check('a bracketed IPv6 address with a port', byName.jump && byName.jump.host === 'fe80::1' && byName.jump.port === 3391);
	check('PuTTY over SSH becomes an SSH host', byName['putty ssh'] && byName['putty ssh'].kind === 'ssh' && byName['putty ssh'].port === 2200 && byName['putty ssh'].username === 'pu');
	check('PuTTY over Telnet is skipped', r.ok && r.skipped.some((s) => s.label === 'telnet box' && s.reason === 'unsupported'));
	check('a numeric type is read from its settings block', byName['numeric rdp'] && byName['numeric rdp'].kind === 'rdp');
	check('the password is not read', !JSON.stringify(r).includes('nope'));
}

section('unreadable files');
{
	check('an encrypted blob is reported as encrypted', parseRdmExport('U2FsdGVkX1+abc/def==\nQUJDREVGR0hJSktMTU5PUA==').error === 'encrypted');
	check('XML with only an encrypted payload is reported as encrypted', parseRdmExport(`<RDMExport><EncryptedData>${'A'.repeat(100)}</EncryptedData></RDMExport>`).error === 'encrypted');
	check('broken XML is reported, with the reason', (() => {
		const r = parseRdmExport('<ArrayOfConnection><Connection>');
		return !r.ok && r.error === 'bad_file' && r.detail.length > 0;
	})());
	check('broken JSON is reported', parseRdmExport('{ nope').error === 'bad_file');
	check('JSON without entries finds nothing', parseRdmExport('{"a":1}').error === 'no_hosts');
	check('an empty file finds nothing', parseRdmExport('  ').error === 'no_hosts');
	check('a byte order mark is ignored', parseRdmExport('\uFEFF' + RDM_XML).ok);
}

section('RDM 2026.1 export without credentials');
{
	const r = parseRdmExport(RDM_2026);
	check('the file is read', r.ok, JSON.stringify(r));
	const byName = Object.fromEntries((r.ok ? r.hosts : []).map((h) => [h.name, h]));
	check('three hosts: two SSH, one RDP', r.ok && r.hosts.length === 3);
	check('hosts have no username', r.ok && r.hosts.every((h) => h.username === ''));
	check('a linked SSH gateway becomes the jump host (ID case ignored)', byName.inside && JSON.stringify(byName.inside.jump) === JSON.stringify([{ host: 'gw.example.net', port: 1022 }]));
	check('a host without a gateway has no jump', byName['router (admin)'] && byName['router (admin)'].jump.length === 0);
	check('the gateway entry itself is neither a host nor skipped', r.ok && !r.skipped.some((s) => s.label === 'Lab jump gateway'));
	check('an SSH tunnel is skipped as unsupported', r.ok && r.skipped.some((s) => s.label === 'Lab tunnel' && s.type === 'SSHTunnel'));
	check('nested groups', byName['DC (AD)'] && byName['DC (AD)'].folder.join('/') === 'RDP/Corp');
	const vpn = parseRdmExport(JSON.stringify([{ ConnectionType: 'VPN', Name: 'OpenVPN', VPN: { Application: 'OpenVPN' } }]));
	check('another VPN entry is skipped as unsupported', !vpn.ok || vpn.skipped.some((s) => s.label === 'OpenVPN' && s.reason === 'unsupported'));
}

section('credentials inherited from a group');
{
	const xml = `<RDMExport><Connections>
		<Connection><ConnectionType>Group</ConnectionType><Group>RDP\\Bank</Group><Name>Bank</Name><CredentialConnectionID>fda0</CredentialConnectionID></Connection>
		<Connection><ConnectionType>Credential</ConnectionType><Group>RDP\\Bank</Group><ID>FDA0</ID><Name>bank account</Name><Credentials><UserName>jdoe</UserName><Domain>BANK</Domain></Credentials></Connection>
		<Connection><ConnectionType>RDPConfigured</ConnectionType><Group>RDP\\Bank\\Prod</Group><Name>app</Name><Url>app.bank</Url><CredentialConnectionID>1310CF82-6FAB-4B7A-9EEA-3E2E451CA2CF</CredentialConnectionID></Connection>
		<Connection><ConnectionType>RDPConfigured</ConnectionType><Group>RDP\\Other</Group><Name>other</Name><Url>other</Url><CredentialConnectionID>1310CF82-6FAB-4B7A-9EEA-3E2E451CA2CF</CredentialConnectionID></Connection>
		<Connection><ConnectionType>RDPConfigured</ConnectionType><Group>RDP\\Bank</Group><Name>none</Name><Url>none</Url></Connection>
	</Connections></RDMExport>`;
	const r = parseRdmExport(xml);
	const byName = Object.fromEntries((r.ok ? r.hosts : []).map((h) => [h.name, h]));
	check('an unresolved link takes the nearest group credential', byName.app && byName.app.username === 'jdoe' && byName.app.domain === 'BANK');
	check('no group credential up the path leaves it empty', byName.other && byName.other.username === '');
	check('an entry with no credential link inherits nothing', byName.none && byName.none.username === '');
}

section('duplicates');
{
	const entry = (name, group, user) => ({ Name: name, ConnectionType: 'RDPConfigured', Group: group, Url: 'DC01.corp', RDP: { UserName: user } });
	const r = parseRdmExport(JSON.stringify({ Connections: [entry('dc', 'A', 'admin'), entry('dc copy', 'B', 'admin'), entry('dc other user', 'B', 'bob')] }));
	check('the same host and user twice is imported once', r.ok && r.hosts.length === 2, JSON.stringify(r.hosts?.map((h) => h.name)));
	check('the first one wins', r.ok && r.hosts[0].name === 'dc');
	check('the second is listed as a duplicate', r.ok && r.skipped.some((s) => s.label === 'dc copy' && s.reason === 'duplicate'));
	check('another user on the same host is kept', r.ok && r.hosts.some((h) => h.username === 'bob'));
}

section('helpers');
{
	check('splitAddress: scheme and path are dropped', JSON.stringify(splitAddress('rdp://h.local:3390/x')) === JSON.stringify({ host: 'h.local', port: '3390' }));
	check('splitAddress: bare IPv6 keeps every colon', splitAddress('fe80::1').host === 'fe80::1');
	check('splitGroup: empty levels are dropped', splitGroup('\\A\\\\B ').join('|') === 'A|B');

	const host = { id: '0', kind: 'rdp', name: 'x', host: 'DC01', port: 3389, username: 'a', domain: null, keyPath: null, folder: [], jump: [] };
	const sessions = [
		{ id: 's1', name: 'x', host: 'dc01', port: 3389, username: 'a', kind: 'rdp' },
		{ id: 's2', name: 'y', host: 'web', port: 22, username: 'a' }
	];
	check('findExisting matches kind, host (any case), port and user', findExisting(host, sessions)?.id === 's1');
	check('findExisting treats a session without a kind as SSH', findExisting({ ...host, kind: 'ssh', host: 'web', port: 22 }, sessions)?.id === 's2');
	check('findExisting: another user is another session', findExisting({ ...host, username: 'b' }, sessions) === undefined);
	check('findExisting: no username in the export is not another user', findExisting({ ...host, username: '' }, sessions) === undefined);
	check('findExisting does not match another kind', findExisting({ ...host, kind: 'vnc' }, sessions) === undefined);

	check("sessionUser: the export's own username wins", sessionUser({ ...host, kind: 'ssh', username: 'root' }, 'ops') === 'root');
	check('sessionUser: no username takes the typed one', sessionUser({ ...host, kind: 'ssh', username: '' }, ' ops ') === 'ops');
	check('sessionUser: VNC never gets one', sessionUser({ ...host, kind: 'vnc', username: '' }, 'ops') === '');

	const folders = [
		{ id: 'f1', name: 'HQ', parent_id: null, vault_id: null },
		{ id: 'f2', name: 'Servers', parent_id: 'f1', vault_id: null },
		{ id: 'f3', name: 'HQ', parent_id: null, vault_id: 'v' }
	];
	check('findChildFolder finds a top-level folder', findChildFolder('HQ', null, null, folders)?.id === 'f1');
	check('findChildFolder finds a nested folder', findChildFolder('Servers', 'f1', null, folders)?.id === 'f2');
	check('findChildFolder keeps vaults apart', findChildFolder('HQ', null, 'v', folders)?.id === 'f3');
}

console.log(failures === 0 ? '\nAll RDM import tests passed.' : `\n${failures} RDM import test(s) failed.`);
process.exit(failures === 0 ? 0 : 1);
