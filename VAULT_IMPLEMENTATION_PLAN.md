# Vault System Implementation Plan

## Core Concepts

### Vault Types
- **Private Vault**: User's personal sessions/credentials, no sync needed
- **Shared Vault**: Team/cluster sessions, syncs via Turso when online

### Data Flow
```
Session (with password) → Encrypted in Vault → Local SQLite → (if shared) Turso Sync
```

### Offline vs Online
- **Offline**: Full read/write to local replica. Can't share/invite.
- **Online**: Sync with Turso. Can share vaults, invite members.

---

## Identity System (SSL-style)

### Current State
- X25519 keypair stored in OS keychain
- Identity file stores: UUID, salt, public key
- Export: `exportIdentity()` → base64 secret key
- Import: `importIdentity(base64)` → restores on new machine

### New Machine Flow
1. User exports secret key from old machine (base64 string)
2. On new machine, clicks "Import Identity"
3. Pastes secret key
4. App derives same public key, creates identity file
5. User now has same identity on both machines
6. Shared vaults can sync (same public key = same member)

---

## UI Changes Needed

### Sessions Panel
```
PRIVATE VAULT
├── minipc (root@192.168.178.200) [PW saved]
├── webserver (deploy@10.0.0.5)
└── + Add Session

DEVOPS TEAM (shared, 3 members)
├── prod-db (admin@db.example.com) [PW saved]
├── staging (deploy@staging.example.com)
└── + Add Session

+ Create Vault
```

### Create Vault Dialog
- Name: "DevOps Team"
- Type: [Private] [Shared]
- Turso URL (for shared): libsql://vault-xxx.turso.io
- Auth Token: xxxxx

### Vault Settings (for shared vaults)
- Members list (UUID, public key, role)
- Invite Member button
- Turso sync status
- Manual sync button

### Import/Export Identity
- Settings → Identity → Export (shows base64 key, copy button)
- Settings → Identity → Import (paste base64 key)

---

## Backend Changes Needed

### 1. User-Visible Vaults
Currently: Internal vaults only (__sessions__, __credentials__, etc.)
Needed: User can create named vaults (private or shared)

```rust
pub async fn create_user_vault(
    name: &str,
    vault_type: VaultType,  // Private or Shared
    sync_url: Option<&str>,  // Turso URL for shared
    auth_token: Option<&str>,
) -> Result<VaultInfo, VaultError>
```

### 2. Session-to-Vault Assignment
Sessions belong to a specific vault (not just __sessions__)

```rust
pub struct SessionConfig {
    // ...existing fields...
    pub vault_id: String,  // Which vault this session is in
}
```

### 3. Vault Membership (for shared)
```sql
CREATE TABLE vault_members (
    user_uuid TEXT PRIMARY KEY,
    public_key BLOB NOT NULL,
    wrapped_master_dek BLOB NOT NULL,
    role TEXT NOT NULL,  -- owner, admin, member
    added_at INTEGER NOT NULL
);
```

### 4. Turso Sync Integration
```rust
impl VaultConnection {
    pub async fn sync(&self) -> Result<(), VaultError> {
        if let Some(url) = &self.sync_url {
            self.db.sync().await?;
        }
        Ok(())
    }
}
```

---

## Sharing Flow (Online Only)

### Inviter Side
1. Create shared vault with Turso URL
2. Add sessions to it
3. Click "Invite Member"
4. Enter invitee's UUID + public key
5. Backend:
   - Unwrap vault's master DEK with inviter's KEK
   - Re-wrap master DEK with invitee's X25519 public key
   - Store in vault_members table
   - Generate scoped Turso token for invitee
6. Share invite link/token with invitee (out of band)

### Invitee Side
1. Click "Accept Invite"
2. Enter sync URL + token
3. Backend:
   - Open local replica of shared vault
   - Find own entry in vault_members
   - Unwrap master DEK using own secret key
   - Now can decrypt all secrets in vault
4. Sync happens automatically

---

## Security Invariants

1. **Passwords encrypted at rest**: Always XChaCha20-Poly1305
2. **No plaintext in logs**: Secret<T> wrapper on all credentials
3. **Zeroize after use**: secrecy crate handles this
4. **Server never sees plaintext**: Only encrypted blobs sync
5. **Offline-first**: Local SQLite always works
6. **Sharing requires online**: Can't sync without Turso connection

---

## Implementation Order

### Phase 1: Password Storage (DONE)
- [x] Password field in SessionEditor
- [x] Password stored encrypted in session
- [x] Auto-connect with saved password

### Phase 2: User Vaults
- [ ] Create vault UI (name, type, sync URL)
- [ ] List user vaults in sidebar
- [ ] Session belongs to vault (vault_id field)
- [ ] Move/copy session between vaults

### Phase 3: Vault Sharing
- [ ] Invite member UI
- [ ] vault_members table
- [ ] Key re-wrap on invite
- [ ] Accept invite flow

### Phase 4: Turso Sync
- [ ] Turso config in settings
- [ ] Auto-sync for shared vaults
- [ ] Sync status indicator
- [ ] Conflict resolution

### Phase 5: Identity Management
- [ ] Export identity UI
- [ ] Import identity UI
- [ ] Multi-device same identity

### Phase 6: Full Backup Export/Import (DONE)
- [x] `src-tauri/src/vault/export.rs` — ExportBundle types, seal/unseal with XChaCha20-Poly1305 + Argon2id
- [x] Binary `.reachbackup` format: `[8B magic][2B version][32B salt][24B nonce][4B json_len][ciphertext]`
- [x] Export password min 8 chars, Argon2id (256MB, 4 iter, 4 parallel), domain-separated HKDF
- [x] Secrets exported as ciphertext only (never decrypted during export/import)
- [x] `VaultManager::export_full_backup()` — reads identity + all vault DBs → sealed bundle → file
- [x] `VaultManager::import_full_backup()` — file → unseal → reset → write identity → recreate DBs → unlock
- [x] `VaultManager::preview_backup()` — validate + return metadata without full import
- [x] IPC commands: `vault_export_backup`, `vault_import_backup`, `vault_preview_backup`
- [x] Frontend: `BackupTab.svelte` in Settings with Export/Import sections
- [x] "Import Backup" link on VaultPanel lock screen (fresh install)
- [x] `VaultError::InvalidExportFormat`, `VaultError::UnsupportedExportVersion(u16)`

---

## Files to Modify

### Backend (Rust)
- `src-tauri/src/vault/manager.rs` - User vault CRUD
- `src-tauri/src/vault/sync.rs` - Turso sync logic
- `src-tauri/src/ipc/vault_commands.rs` - New IPC endpoints
- `src-tauri/src/ipc/session_commands.rs` - vault_id in sessions

### Frontend (Svelte)
- `src/lib/components/sessions/SessionList.svelte` - Group by vault
- `src/lib/components/vault/VaultList.svelte` - New component
- `src/lib/components/vault/CreateVault.svelte` - New component
- `src/lib/components/vault/InviteMember.svelte` - New component
- `src/lib/components/settings/Identity.svelte` - Export/import
