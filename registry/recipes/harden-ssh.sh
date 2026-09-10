#!/usr/bin/env bash
# @reach-recipe
# id: harden-ssh
# name: Harden the SSH daemon
# description: Disables root login and password auth, then reloads sshd only if the config parses.
# version: 1.0.0
# author: alexandrosnt
# tags: security, ssh, hardening
# targets: debian, ubuntu, rhel
# danger: sensitive
# param: ADMIN_USER | Account that must keep shell access |
# param: SSH_PORT | Port sshd should listen on | 22
# @end

# The failure mode this recipe exists to avoid is locking you out of the
# machine you are hardening. So: the admin user is verified to exist and to
# have a key BEFORE anything changes, the config is validated before reload,
# and reload is used rather than restart so the current session survives.
set -euo pipefail

CONFIG=/etc/ssh/sshd_config
BACKUP="${CONFIG}.reach-$(date +%Y%m%d%H%M%S)"

echo "== Preflight =="

if ! id "${ADMIN_USER}" >/dev/null 2>&1; then
  echo "FATAL: user '${ADMIN_USER}' does not exist. Refusing to disable password login." >&2
  exit 1
fi

# Disabling password auth without a working key is how a server becomes a
# brick with a bill attached.
ADMIN_HOME=$(getent passwd "${ADMIN_USER}" | cut -d: -f6)
if [ ! -s "${ADMIN_HOME}/.ssh/authorized_keys" ]; then
  echo "FATAL: ${ADMIN_HOME}/.ssh/authorized_keys is missing or empty." >&2
  echo "       Install a key for ${ADMIN_USER} first, or you will be locked out." >&2
  exit 1
fi
echo "ok: ${ADMIN_USER} exists and has $(grep -c . "${ADMIN_HOME}/.ssh/authorized_keys") authorized key(s)"

echo
echo "== Backup =="
cp -a "${CONFIG}" "${BACKUP}"
echo "saved ${BACKUP}"

echo
echo "== Applying =="
set_option() {
  local key="$1" value="$2"
  if grep -qE "^[#[:space:]]*${key}[[:space:]]" "${CONFIG}"; then
    sed -i -E "s|^[#[:space:]]*${key}[[:space:]].*|${key} ${value}|" "${CONFIG}"
  else
    printf '%s %s\n' "${key}" "${value}" >> "${CONFIG}"
  fi
  echo "  ${key} ${value}"
}

set_option PermitRootLogin no
set_option PasswordAuthentication no
set_option KbdInteractiveAuthentication no
set_option ChallengeResponseAuthentication no
set_option PubkeyAuthentication yes
set_option X11Forwarding no
set_option MaxAuthTries 3
set_option Port "${SSH_PORT}"
set_option AllowUsers "${ADMIN_USER}"

echo
echo "== Validating =="
# If this fails, the reload never happens and the backup is still on disk.
if ! sshd -t; then
  echo "FATAL: sshd rejected the new config. Restoring ${BACKUP}." >&2
  cp -a "${BACKUP}" "${CONFIG}"
  exit 1
fi
echo "ok: config parses"

echo
echo "== Reloading =="
# reload, not restart: existing sessions - including this one - stay up.
if command -v systemctl >/dev/null 2>&1; then
  systemctl reload sshd 2>/dev/null || systemctl reload ssh
else
  service ssh reload 2>/dev/null || service sshd reload
fi

echo
echo "Done. Open a SECOND session and confirm you can still log in before"
echo "closing this one. If anything is wrong: cp -a ${BACKUP} ${CONFIG}"
