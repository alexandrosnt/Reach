<script lang="ts">
	import Modal from "$lib/components/shared/Modal.svelte";
	import Button from "$lib/components/shared/Button.svelte";
	import KeyPicker from "./KeyPicker.svelte";
	import Input from "$lib/components/shared/Input.svelte";
	import { isWindows } from "$lib/platform";
	import { sessionCreate, sessionList, sessionUpdate, sessionKind, wslListDistros,type SessionConfig, type SessionKind, type AuthMethod, type JumpHostConfig, type Folder, type SshOptions } from '$lib/ipc/sessions';
	import SshOptionsSection from './SshOptionsSection.svelte';
	import { t } from "$lib/state/i18n.svelte";
	import { open as openDialog } from "@tauri-apps/plugin-dialog";

	interface Props {
		open: boolean;
		editSession?: SessionConfig;
		vaultId?: string | null; // Which vault to save to (null = private)
		folders?: Folder[];
		onsave?: () => void;
	}

	let {
		open = $bindable(),
		editSession,
		vaultId = null,
		folders = [],
		onsave,
	}: Props = $props();

	/**
	 * SSH or RDP. The two share a name, a host, a port and a username; RDP has
	 * a password and a logon domain and nothing else, because keys, agents,
	 * jump hosts, proxies and login shells are all SSH ideas. Switching moves
	 * the port between the two defaults only if it still is one.
	 *
	 * VNC has less still: a password and no user name. What it has instead
	 * is the way in: VNC is not encrypted, so it can go through an SSH
	 * session saved here.
	 */
	let kind = $state<SessionKind>("ssh");
	let domain = $state("");
	/** RDP only: the protocol cannot say what the machine is (xrdp claims to be Windows), so the user does. */
	let os = $state<"windows" | "linux">("windows");
	let sharePath = $state("");
	let wslDistro = $state("");
	let availableDistros = $state<string[]>([]);
	/** VNC only: the saved SSH session to go through, or '' for direct. */
	let viaSessionId = $state("");
	let sshSessions = $state<SessionConfig[]>([]);

	$effect(() => {
		if (open && kind === "vnc") {
			sessionList()
				.then(
					(all) =>
						(sshSessions = all.filter(
							(s) => sessionKind(s) === "ssh",
						)),
				)
				.catch(() => (sshSessions = []));
		}
	});
	$effect(() => {
		if (open && kind === "wsl") {
			wslListDistros()
				.then((list) => {
					availableDistros = list;
					if (list.length > 0) {
						if (!wslDistro || !list.includes(wslDistro)) {
							wslDistro = list[0];
						}
					}
				})
				.catch(() => {
					availableDistros = [];
				});
		}
	});

	let name = $state("");
	let host = $state("");
	let portStr = $state("22");
	let username = $state("root");

	const DEFAULT_PORT: Record<SessionKind, string> = {
		ssh: "22",
		rdp: "3389",
		vnc: "5900",
		wsl: "0",
	};

	function setKind(next: SessionKind): void {
		if (next === kind) return;
		if (portStr === DEFAULT_PORT[kind] || portStr.trim() === "")
			portStr = DEFAULT_PORT[next];
		if (next !== "ssh" && username === "root") username = "";
		if (next === "ssh" && username === "") username = "root";
		kind = next;
	}
	let authType = $state<"Password" | "Key" | "Agent">("Password");
	let password = $state("");
	let keyPath = $state("");
	/** An imported key, used instead of keyPath. */
	let keyId = $state("");
	let keyPassphrase = $state("");
	/** Key sessions: also offer the agent's keys if this key is refused. Off by default: see build_auth. */
	let tryAgentKeys = $state(false);
	let shell = $state("");
	let tagsStr = $state("");
	let folderIdStr = $state("");
	let jumpEnabled = $state(false);
	let jumpHops = $state<
		Array<{
			host: string;
			port: string;
			username: string;
			authType: "Password" | "Key" | "Agent";
			password: string;
			keyPath: string;
			keyPassphrase: string;
		}>
	>([]);
	let proxyEnabled = $state(false);
	let proxyType = $state<'socks5' | 'socks4' | 'http'>('socks5');
	let proxyHost = $state('127.0.0.1');
	let proxyPort = $state('9050');
	let proxyUsername = $state('');
	let proxyPassword = $state('');
	/** ssh_config settings: imported files, lines set here, approvals. */
	let sshOptions = $state<SshOptions>({});
	let sshSection = $state<ReturnType<typeof SshOptionsSection> | undefined>();
	let saving = $state(false);
	let error = $state<string | undefined>();

	let isEditing = $derived(!!editSession);
	let canSave = $derived(
		name.trim().length > 0 &&
			(kind === "wsl"
				? wslDistro.trim().length > 0
				: host.trim().length > 0) &&
			(kind === "vnc" || kind === "wsl" || username.trim().length > 0) &&
			!saving,
	);

	// Populate fields when editing, reset when creating
	$effect(() => {
		if (editSession) {
			kind = sessionKind(editSession);
			domain = editSession.domain ?? "";
			wslDistro = editSession.wsl_distro ?? "";
			os = editSession.detected_os === "linux" ? "linux" : "windows";
			sharePath = editSession.share_path ?? "";
			viaSessionId = editSession.via_session_id ?? "";
			name = editSession.name;
			host = editSession.host;
			portStr = String(editSession.port);
			username = editSession.username;
			authType = editSession.auth_method.type;
			password = editSession.auth_method.password ?? "";
			keyPath = editSession.auth_method.path ?? "";
			keyId = editSession.auth_method.key_id ?? "";
			keyPassphrase = editSession.auth_method.passphrase ?? "";
			tryAgentKeys = editSession.try_agent_keys === true;
			shell = editSession.shell ?? '';
			sshOptions = editSession.ssh_options ?? {};
			tagsStr = editSession.tags.join(', ');
			folderIdStr = editSession.folder_id ?? '';
			if (editSession.jump_chain && editSession.jump_chain.length > 0) {
				jumpEnabled = true;
				jumpHops = editSession.jump_chain.map((j) => ({
					host: j.host,
					port: String(j.port),
					username: j.username,
					authType: j.auth_method.type,
					password:
						j.auth_method.type === "Password"
							? (j.auth_method.password ?? "")
							: "",
					keyPath:
						j.auth_method.type === "Key"
							? (j.auth_method.path ?? "")
							: "",
					keyPassphrase:
						j.auth_method.type === "Key"
							? (j.auth_method.passphrase ?? "")
							: "",
				}));
			} else {
				jumpEnabled = false;
				jumpHops = [];
			}
			if (editSession.proxy) {
				proxyEnabled = true;
				proxyType =
					(editSession.proxy.proxy_type as
						| "socks5"
						| "socks4"
						| "http") ?? "socks5";
				proxyHost = editSession.proxy.host ?? "127.0.0.1";
				proxyPort = String(editSession.proxy.port ?? 9050);
				proxyUsername = editSession.proxy.username ?? "";
				proxyPassword = editSession.proxy.password ?? "";
			} else {
				proxyEnabled = false;
			}
		} else {
			kind = "ssh";
			domain = "";
			os = "windows";
			sharePath = "";
			viaSessionId = "";
			name = "";
			host = "";
			portStr = "22";
			username = "root";
			authType = "Password";
			password = "";
			keyPath = "";
			keyId = "";
			keyPassphrase = "";
			tryAgentKeys = false;
			shell = '';
			sshOptions = {};
			tagsStr = '';
			folderIdStr = '';
			jumpEnabled = false;
			jumpHops = [];
			proxyEnabled = false;
			proxyType = "socks5";
			proxyHost = "127.0.0.1";
			wslDistro = "";
			proxyPort = "9050";
			proxyUsername = "";
			proxyPassword = "";
		}
		error = undefined;
	});

	async function handleSave(): Promise<void> {
		if (!canSave) return;
		saving = true;
		error = undefined;

		const port =
			kind === "wsl"
				? 0
				: parseInt(portStr, 10) ||
					parseInt(DEFAULT_PORT[kind], 10) ||
					22;
		const rdp = kind === "rdp";
		const vnc = kind === "vnc";
		/** Jump hosts, proxies and login shells are SSH's alone. */
		const desktop = rdp || vnc;
		const via = vnc ? viaSessionId || null : null;
		// RDP and VNC are password logons; the toggle below is SSH-only and
		// its value is ignored here rather than trusted.
		const authMethod: AuthMethod =
			desktop || authType === "Password"
				? { type: "Password", password: password || undefined }
				: authType === "Key"
					? {
							type: "Key",
							// One source or the other, never both: the picker clears
							// whichever the user did not choose.
							path: keyId ? undefined : keyPath.trim(),
							key_id: keyId || undefined,
							passphrase: keyPassphrase || undefined,
						}
					: { type: "Agent" };
		const tags = tagsStr
			.split(",")
			.map((t) => t.trim())
			.filter(Boolean);

		const jumpChain: JumpHostConfig[] | undefined =
			!desktop && jumpEnabled && jumpHops.length > 0
				? jumpHops.map((h) => {
						const hopAuth: AuthMethod =
							h.authType === "Password"
								? {
										type: "Password",
										password: h.password || undefined,
									}
								: h.authType === "Key"
									? {
											type: "Key",
											path: h.keyPath.trim(),
											passphrase:
												h.keyPassphrase || undefined,
										}
									: { type: "Agent" };
						return {
							host: h.host.trim(),
							port: parseInt(h.port, 10) || 22,
							username: h.username.trim(),
							auth_method: hopAuth,
						};
					})
				: undefined;

		// The weakenings in force are stored with the session, those written
		// here included, so the session list can flag a weaker session.
		const mine = [...new Set([...(sshOptions.my_accepted_weakenings ?? []), ...(sshSection?.acceptedNow() ?? [])])];
		const finalOptions: SshOptions = { ...sshOptions, my_accepted_weakenings: mine };
		const hasOptions = !desktop && (!!finalOptions.imported || (finalOptions.lines ?? []).length > 0);
		const sshOptionsToSave = hasOptions ? finalOptions : null;

		const proxyConfig = !desktop && proxyEnabled ? {
			proxy_type: proxyType,
			host: proxyHost.trim(),
			port: parseInt(proxyPort, 10) || 9050,
			username: proxyUsername.trim() || null,
			password: proxyPassword || null,
		} : null;

		try {
			if (isEditing && editSession) {
				await sessionUpdate({
					...editSession,
					name: name.trim(),
					host: host.trim(),
					port,
					username: vnc ? "" : username.trim(),
					auth_method: authMethod,
					folder_id: folderIdStr || null,
					tags,
					jump_chain: desktop
						? null
						: (jumpChain ?? editSession.jump_chain ?? null),
					proxy: proxyConfig,
					shell: desktop ? null : shell.trim() || null,
					kind,
					domain: rdp ? domain.trim() || null : null,
					// A session that changed protocol changed machine type too.
					detected_os: rdp
						? os
						: kind === sessionKind(editSession)
							? editSession.detected_os
							: null,
					share_path: rdp ? sharePath.trim() || null : null,
					via_session_id: via,
					try_agent_keys:
						!desktop && authType === "Key" && tryAgentKeys ? true : null,
					wsl_distro: kind === "wsl" ? wslDistro.trim() : null,
					ssh_options: sshOptionsToSave,
				});
			} else {
				await sessionCreate({
					name: name.trim(),
					host:
						kind === "wsl"
							? host.trim() || "wsl"
							: host.trim(),
					port,
					username: vnc ? "" : username.trim(),
					authMethod: authMethod,
					folderId: folderIdStr || null,
					tags,
					vaultId,
					jumpChain: jumpChain ?? null,
					proxy: proxyConfig,
					shell: desktop ? null : shell.trim() || null,
					kind,
					domain: rdp ? domain : null,
					detectedOs: rdp ? os : null,
					sharePath: rdp ? sharePath : null,
					viaSessionId: via,
					tryAgentKeys: !desktop && authType === "Key" && tryAgentKeys,
					sshOptions: sshOptionsToSave,
					wslDistro: kind === "wsl" ? wslDistro.trim() : null,
				});
			}
			onsave?.();
			open = false;
		} catch (err) {
			error = String(err);
		} finally {
			saving = false;
		}
	}

	async function browseKey(setter: (path: string) => void): Promise<void> {
		try {
			const selected = await openDialog({
				multiple: false,
				directory: false,
				title: t("session.select_key_file"),
				filters: [
					{
						name: t("session.ssh_private_key_filter"),
						extensions: [
							"pem",
							"key",
							"ppk",
							"rsa",
							"ed25519",
							"ecdsa",
							"dsa",
						],
					},
					{ name: "All Files", extensions: ["*"] },
				],
			});
			if (typeof selected === "string") setter(selected);
		} catch {
			// User cancelled the dialog
		}
	}

	async function browseFolder(): Promise<void> {
		try {
			const selected = await openDialog({
				multiple: false,
				directory: true,
				title: t("session.rdp_share_folder"),
			});
			if (typeof selected === "string") sharePath = selected;
		} catch {
			// User cancelled the dialog
		}
	}

	function addHop(): void {
		jumpHops = [
			...jumpHops,
			{
				host: "",
				port: "22",
				username: "root",
				authType: "Password",
				password: "",
				keyPath: "",
				keyPassphrase: "",
			},
		];
	}

	function removeHop(index: number): void {
		jumpHops = jumpHops.filter((_, i) => i !== index);
	}

	function handleClose(): void {
		if (!saving) {
			open = false;
		}
	}
</script>

<Modal
	{open}
	onclose={handleClose}
	title={isEditing ? t("session.edit_session") : t("session.new")}
>
	<form
		class="form"
		onsubmit={(e) => {
			e.preventDefault();
			handleSave();
		}}
	>
		<!-- Same control as the auth method below: one segmented row, and
		     everything under it follows the choice. -->
		<div class="auth-section">
			<span class="auth-label">{t("session.protocol")}</span>
			<div class="auth-toggle">
				<button
					type="button"
					class="auth-btn"
					class:active={kind === "ssh"}
					disabled={saving}
					onclick={() => setKind("ssh")}
				>
					{t("session.protocol_ssh")}
				</button>
				<button
					type="button"
					class="auth-btn"
					class:active={kind === "rdp"}
					disabled={saving}
					onclick={() => setKind("rdp")}
				>
					{t("session.protocol_rdp")}
				</button>
				<button
					type="button"
					class="auth-btn"
					class:active={kind === "vnc"}
					disabled={saving}
					onclick={() => setKind("vnc")}
				>
					{t("session.protocol_vnc")}
				</button>
				<!-- Only show WSL option on Windows -->
				{#if isWindows()}
					<button
						type="button"
						class="auth-btn"
						class:active={kind === "wsl"}
						disabled={saving}
						onclick={() => setKind("wsl")}
					>
						WSL
					</button>
				{/if}
			</div>
		</div>

		<Input
			label={t("session.name")}
			bind:value={name}
			placeholder={kind === "rdp" ? "Office PC" : "My Server"}
			disabled={saving}
		/>
		{#if kind === "wsl"}
			<div class="shell-field">
				<label class="via-label" for="wsl-distro"
					>WSL Distribution</label
				>
				{#if availableDistros.length > 0}
					<select
						id="wsl-distro"
						class="via-select"
						bind:value={wslDistro}
						disabled={saving}
					>
						{#each availableDistros as d}
							<option value={d}>{d}</option>
						{/each}
					</select>
				{:else}
					<Input
						label="WSL Distribution"
						bind:value={wslDistro}
						placeholder="Ubuntu (or Debian, Arch, etc.)"
						disabled={saving}
					/>
				{/if}
			</div>
		{:else}
			<div class="row">
				<div class="field-host">
					<Input
						label={t("session.host")}
						bind:value={host}
						placeholder="192.168.1.1"
						disabled={saving}
					/>
				</div>
				<div class="field-port">
					<Input
						label={t("session.port")}
						bind:value={portStr}
						type="number"
						placeholder={DEFAULT_PORT[kind]}
						disabled={saving}
					/>
				</div>
			</div>
		{/if}
		{#if kind !== "vnc" && kind !== "wsl"}
			<Input
				label={t("session.username")}
				bind:value={username}
				placeholder={kind === "rdp" ? "Administrator" : "root"}
				disabled={saving}
			/>
		{/if}

		{#if kind === "rdp"}
			<div class="shell-field">
				<Input
					label={t("session.password_optional")}
					bind:value={password}
					type="password"
					placeholder="Stored encrypted in vault"
					disabled={saving}
				/>
				<p class="shell-hint">{t("session.rdp_password_hint")}</p>
			</div>
			<Input
				label={t("session.rdp_domain")}
				bind:value={domain}
				placeholder="CORP"
				disabled={saving}
			/>
			<div class="auth-section">
				<span class="auth-label">{t("session.os")}</span>
				<div class="auth-toggle">
					<button
						type="button"
						class="auth-btn"
						class:active={os === "windows"}
						disabled={saving}
						onclick={() => (os = "windows")}
						>{t("session.os_windows")}</button
					>
					<button
						type="button"
						class="auth-btn"
						class:active={os === "linux"}
						disabled={saving}
						onclick={() => (os = "linux")}
						>{t("session.os_linux")}</button
					>
				</div>
			</div>
			<div class="shell-field">
				<div class="key-path-row">
					<div class="key-path-input">
						<Input
							label={t("session.rdp_share_folder")}
							bind:value={sharePath}
							placeholder="C:\Users\me\Documents"
							disabled={saving}
						/>
					</div>
					<Button
						variant="secondary"
						size="sm"
						onclick={browseFolder}
						disabled={saving}
					>
						{t("session.rdp_share_browse")}
					</Button>
				</div>
				<p class="shell-hint">{t("session.rdp_share_hint")}</p>
			</div>
		{:else if kind === "vnc"}
			<div class="shell-field">
				<Input
					label={t("session.password_optional")}
					bind:value={password}
					type="password"
					placeholder="Stored encrypted in vault"
					disabled={saving}
				/>
				<p class="shell-hint">{t("session.vnc_password_hint")}</p>
			</div>
			<div class="shell-field">
				<label class="via-label" for="vnc-via"
					>{t("session.vnc_via")}</label
				>
				<select
					id="vnc-via"
					class="via-select"
					bind:value={viaSessionId}
					disabled={saving}
				>
					<option value="">{t("session.vnc_via_none")}</option>
					{#each sshSessions as s (s.id)}
						<option value={s.id}
							>{s.name} ({s.username}@{s.host})</option
						>
					{/each}
				</select>
				<p class="shell-hint">{t("session.vnc_via_hint")}</p>
			</div>
		{:else if kind === "ssh"}
			<div class="auth-section">
				<span class="auth-label">{t("session.auth_method")}</span>
				<div class="auth-toggle">
					<button
						type="button"
						class="auth-btn"
						class:active={authType === "Password"}
						disabled={saving}
						onclick={() => (authType = "Password")}
					>
						{t("session.auth_password")}
					</button>
					<button
						type="button"
						class="auth-btn"
						class:active={authType === "Key"}
						disabled={saving}
						onclick={() => (authType = "Key")}
					>
						{t("session.auth_key")}
					</button>
					<button
						type="button"
						class="auth-btn"
						class:active={authType === "Agent"}
						disabled={saving}
						onclick={() => (authType = "Agent")}
					>
						{t("session.auth_agent")}
					</button>
				</div>
			</div>

			{#if authType === "Password"}
				<Input
					label={t("session.password_optional")}
					bind:value={password}
					type="password"
					placeholder="Stored encrypted in vault"
					disabled={saving}
				/>
			{:else if authType === "Key"}
				<KeyPicker bind:path={keyPath} bind:keyId disabled={saving} />
				{#if !keyId}
					<Input
						label={t("session.passphrase_optional")}
						bind:value={keyPassphrase}
						type="password"
						placeholder="Stored encrypted in vault"
						disabled={saving}
					/>
				{/if}
				<div class="shell-field">
					<label class="jump-toggle">
						<input
							type="checkbox"
							bind:checked={tryAgentKeys}
							disabled={saving}
						/>
						<span class="jump-toggle-text"
							>{t("session.try_agent_keys")}</span
						>
					</label>
					<p class="shell-hint">{t("session.try_agent_keys_hint")}</p>
				</div>
			{/if}

			<div class="shell-field">
				<Input
					label={t("session.login_shell_optional")}
					bind:value={shell}
					placeholder="fish -l"
					disabled={saving}
				/>
				<p class="shell-hint">{t("session.login_shell_hint")}</p>
			</div>

			<div class="jump-section">
				<label class="jump-toggle">
					<input
						type="checkbox"
						bind:checked={jumpEnabled}
						disabled={saving}
					/>
					<span class="jump-toggle-text"
						>{t("session.jump_host_enable")}</span
					>
				</label>

				{#if jumpEnabled}
					<p class="jump-hint">{t("session.jump_host_hint")}</p>

					{#each jumpHops as hop, i (i)}
						<div class="jump-hop">
							<div class="jump-hop-header">
								<span class="jump-hop-label"
									>{t("session.jump_hop", {
										n: String(i + 1),
									})}</span
								>
								<button
									type="button"
									class="jump-hop-remove"
									onclick={() => removeHop(i)}
									disabled={saving}
								>
									{t("session.jump_remove_hop")}
								</button>
							</div>
							<div class="row">
								<div class="field-host">
									<Input
										label={t("session.host")}
										bind:value={hop.host}
										placeholder="bastion.example.com"
										disabled={saving}
									/>
								</div>
								<div class="field-port">
									<Input
										label={t("session.port")}
										bind:value={hop.port}
										type="number"
										placeholder="22"
										disabled={saving}
									/>
								</div>
							</div>
							<Input
								label={t("session.username")}
								bind:value={hop.username}
								placeholder="root"
								disabled={saving}
							/>

							<div class="auth-section">
								<span class="auth-label"
									>{t("session.auth_method")}</span
								>
								<div class="auth-toggle">
									<button
										type="button"
										class="auth-btn"
										class:active={hop.authType ===
											"Password"}
										disabled={saving}
										onclick={() =>
											(hop.authType = "Password")}
									>
										{t("session.auth_password")}
									</button>
									<button
										type="button"
										class="auth-btn"
										class:active={hop.authType === "Key"}
										disabled={saving}
										onclick={() => (hop.authType = "Key")}
									>
										{t("session.auth_key")}
									</button>
									<button
										type="button"
										class="auth-btn"
										class:active={hop.authType === "Agent"}
										disabled={saving}
										onclick={() => (hop.authType = "Agent")}
									>
										{t("session.auth_agent")}
									</button>
								</div>
							</div>

							{#if hop.authType === "Password"}
								<Input
									label={t("session.password_optional")}
									bind:value={hop.password}
									type="password"
									disabled={saving}
								/>
							{:else if hop.authType === "Key"}
								<div class="key-path-row">
									<div class="key-path-input">
										<Input
											label={t("session.key_path")}
											bind:value={hop.keyPath}
											placeholder="~/.ssh/id_rsa"
											disabled={saving}
										/>
									</div>
									<Button
										variant="secondary"
										size="sm"
										onclick={() =>
											browseKey((p) => (hop.keyPath = p))}
										disabled={saving}
									>
										{t("session.browse_key")}
									</Button>
								</div>
								<Input
									label={t("session.passphrase_optional")}
									bind:value={hop.keyPassphrase}
									type="password"
									disabled={saving}
								/>
							{/if}
						</div>
					{/each}

					<button
						type="button"
						class="jump-add-btn"
						onclick={addHop}
						disabled={saving}
					>
						+ {t("session.jump_add_hop")}
					</button>
				{/if}
			</div>

			<div class="proxy-section">
				<label class="proxy-toggle">
					<input
						type="checkbox"
						bind:checked={proxyEnabled}
						disabled={saving}
					/>
					<span class="proxy-toggle-text">Connect via Proxy</span>
				</label>

			{#if proxyEnabled}
				<div class="proxy-fields">
					<div class="proxy-type-row">
						<button type="button" class="proxy-type-btn" class:active={proxyType === 'socks5'} onclick={() => (proxyType = 'socks5')} disabled={saving}>SOCKS5</button>
						<button type="button" class="proxy-type-btn" class:active={proxyType === 'socks4'} onclick={() => (proxyType = 'socks4')} disabled={saving}>SOCKS4</button>
						<button type="button" class="proxy-type-btn" class:active={proxyType === 'http'} onclick={() => (proxyType = 'http')} disabled={saving}>HTTP</button>
					</div>
					<div class="row">
						<div class="field-host">
							<Input label="Proxy Host" bind:value={proxyHost} placeholder="127.0.0.1" disabled={saving} />
						</div>
						<div class="field-port">
							<Input label="Port" bind:value={proxyPort} type="number" placeholder="9050" disabled={saving} />
						</div>
					</div>
					<div class="row">
						<div class="field-host">
							<Input label="Username (optional)" bind:value={proxyUsername} placeholder="" disabled={saving} />
						</div>
						<div class="field-host">
							<Input label="Password (optional)" bind:value={proxyPassword} type="password" disabled={saving} />
						</div>
					</div>
					<p class="proxy-hint">
						{#if proxyType === 'socks5'}
							Tor default: 127.0.0.1:9050 | Tor Browser: 127.0.0.1:9150
						{:else if proxyType === 'http'}
							HTTP CONNECT proxy for tunneling SSH through corporate proxies
						{:else}
							SOCKS4 proxy (no authentication support)
						{/if}
					</p>
				</div>
			{/if}
		</div>

		{#key editSession?.id ?? 'new'}
			<SshOptionsSection
				bind:this={sshSection}
				bind:options={sshOptions}
				{host}
				port={parseInt(portStr, 10) || 22}
				{username}
				disabled={saving}
			/>
		{/key}
		{/if}

		<Input
			label={t("session.tags")}
			bind:value={tagsStr}
			placeholder={kind === "rdp"
				? "office, windows"
				: "production, web, linux"}
			disabled={saving}
		/>

		{#if folders.length > 0}
			<div class="folder-section">
				<span class="folder-label">{t("session.folder")}</span>
				<select
					class="folder-select"
					bind:value={folderIdStr}
					disabled={saving}
				>
					<option value="">{t("session.no_folder")}</option>
					{#each folders as folder (folder.id)}
						<option value={folder.id}>{folder.name}</option>
					{/each}
				</select>
			</div>
		{/if}

		{#if error}
			<div class="error-message">{error}</div>
		{/if}
	</form>

	{#snippet actions()}
		<Button variant="secondary" onclick={handleClose} disabled={saving}>
			{t("common.cancel")}
		</Button>
		<Button variant="primary" onclick={handleSave} disabled={!canSave}>
			{#if saving}
				<span class="spinner"></span>
				{t("session.saving")}
			{:else}
				{isEditing
					? t("session.update_session")
					: t("session.save_session")}
			{/if}
		</Button>
	{/snippet}
</Modal>

<style>
	.form {
		display: flex;
		flex-direction: column;
		gap: 12px;
	}

	.row {
		display: flex;
		gap: 10px;
		align-items: flex-start;
	}

	.field-host {
		flex: 1;
		min-width: 0;
	}

	.field-port {
		width: 80px;
		flex-shrink: 0;
	}

	.key-path-row {
		display: flex;
		align-items: flex-end;
		gap: 8px;
	}

	.key-path-input {
		flex: 1;
		min-width: 0;
	}

	.folder-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.folder-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.folder-select {
		padding: 6px 10px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		color: var(--color-text-primary);
		font-family: var(--font-sans);
		font-size: 0.75rem;
		outline: none;
	}

	.folder-select:focus {
		border-color: var(--color-accent);
	}

	.auth-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.auth-label {
		font-size: 0.6875rem;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.05em;
		color: var(--color-text-secondary);
	}

	.shell-field {
		display: flex;
		flex-direction: column;
		gap: 4px;
	}

	.shell-hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.auth-toggle {
		display: flex;
		gap: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.auth-btn {
		flex: 1;
		padding: 7px 12px;
		font-family: var(--font-sans);
		font-size: 0.8125rem;
		font-weight: 500;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.auth-btn:hover:not(:disabled) {
		background-color: var(--color-surface-hover);
	}

	.auth-btn.active {
		background-color: var(--color-accent);
		color: #fff;
	}

	.auth-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.auth-btn + .auth-btn {
		border-left: 1px solid var(--color-border);
	}

	.error-message {
		padding: 8px 12px;
		font-size: 0.8125rem;
		color: var(--color-danger);
		background-color: rgba(255, 69, 58, 0.08);
		border: 1px solid rgba(255, 69, 58, 0.2);
		border-radius: var(--radius-btn);
	}

	.spinner {
		display: inline-block;
		width: 14px;
		height: 14px;
		border: 2px solid var(--color-border);
		border-top-color: #fff;
		border-radius: 50%;
		animation: spin 0.6s linear infinite;
	}

	@keyframes spin {
		to {
			transform: rotate(360deg);
		}
	}

	.proxy-section {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding-top: 4px;
	}

	.proxy-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.proxy-toggle input[type="checkbox"] {
		width: 14px;
		height: 14px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.proxy-toggle-text {
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.proxy-fields {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding: 10px;
		background: var(--color-surface-hover);
		border: 1px solid var(--color-border);
		border-radius: 8px;
	}

	.proxy-type-row {
		display: flex;
		gap: 0;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		overflow: hidden;
	}

	.proxy-type-btn {
		flex: 1;
		padding: 5px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		font-weight: 500;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color var(--duration-default) var(--ease-default),
			color var(--duration-default) var(--ease-default);
	}

	.proxy-type-btn.active {
		background-color: var(--color-accent);
		color: #fff;
	}

	.proxy-type-btn:not(.active):hover {
		background-color: var(--color-surface-hover);
	}

	.proxy-hint {
		margin: 0;
		font-size: 0.625rem;
		color: var(--color-text-secondary);
		opacity: 0.7;
	}

	.via-label {
		display: block;
		margin-bottom: 4px;
		font-size: 0.75rem;
		color: var(--color-text-secondary);
	}

	/* Dressed as the text fields around it. */
	.via-select {
		width: 100%;
		padding: 12px;
		font-family: var(--font-sans);
		font-size: 0.875rem;
		color: var(--color-text-primary);
		background-color: var(--color-bg-elevated);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		outline: none;
		box-sizing: border-box;
	}

	.via-select:focus {
		border-color: var(--color-accent);
	}

	.jump-section {
		display: flex;
		flex-direction: column;
		gap: 10px;
		padding-top: 4px;
	}

	.jump-toggle {
		display: flex;
		align-items: center;
		gap: 8px;
		cursor: pointer;
	}

	.jump-toggle input {
		width: 14px;
		height: 14px;
		accent-color: var(--color-accent);
	}

	.jump-toggle-text {
		font-size: 0.8125rem;
		font-weight: 500;
		color: var(--color-text-primary);
	}

	.jump-hint {
		margin: 0;
		font-size: 0.6875rem;
		color: var(--color-text-secondary);
	}

	.jump-hop {
		display: flex;
		flex-direction: column;
		gap: 8px;
		padding: 10px;
		border: 1px solid var(--color-border);
		border-radius: var(--radius-btn);
		background-color: var(--color-surface-hover);
	}

	.jump-hop-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.jump-hop-label {
		font-size: 0.75rem;
		font-weight: 600;
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.jump-hop-remove {
		padding: 2px 8px;
		font-family: var(--font-sans);
		font-size: 0.6875rem;
		color: var(--color-danger);
		background: transparent;
		border: 1px solid rgba(255, 69, 58, 0.3);
		border-radius: 4px;
		cursor: pointer;
	}

	.jump-hop-remove:hover:not(:disabled) {
		background-color: rgba(255, 69, 58, 0.08);
	}

	.jump-hop-remove:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.jump-add-btn {
		padding: 6px 12px;
		font-family: var(--font-sans);
		font-size: 0.75rem;
		font-weight: 500;
		color: var(--color-accent);
		background: transparent;
		border: 1px dashed var(--color-accent);
		border-radius: var(--radius-btn);
		cursor: pointer;
		transition: background-color var(--duration-default) var(--ease-default);
	}

	.jump-add-btn:hover:not(:disabled) {
		background-color: rgba(0, 122, 255, 0.08);
	}

	.jump-add-btn:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}
</style>
