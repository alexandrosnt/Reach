#!/usr/bin/env bash
# @reach-recipe
# id: fail2ban-baseline
# name: Fail2ban with an sshd jail
# description: Installs fail2ban and configures a sensible sshd jail in jail.local.
# version: 1.0.0
# author: alexandrosnt
# tags: security, fail2ban, ssh
# targets: debian, ubuntu, rhel
# danger: sensitive
# param: BAN_TIME | How long a ban lasts | 1h
# param: MAX_RETRY | Failures before a ban | 5
# param: IGNORE_IP | Never ban these (space separated) | 127.0.0.1/8 ::1
# @end

set -euo pipefail

echo "== Installing =="
if command -v apt-get >/dev/null 2>&1; then
  export DEBIAN_FRONTEND=noninteractive
  apt-get update -qq
  apt-get install -y -qq fail2ban
elif command -v dnf >/dev/null 2>&1; then
  dnf install -y fail2ban
elif command -v yum >/dev/null 2>&1; then
  yum install -y fail2ban
else
  echo "FATAL: no supported package manager found." >&2
  exit 1
fi

echo
echo "== Configuring =="
# jail.local, never jail.conf: the package owns jail.conf and will overwrite
# it on the next upgrade, silently undoing everything done here.
cat > /etc/fail2ban/jail.local <<'CONF'
[DEFAULT]
backend = systemd
CONF

cat >> /etc/fail2ban/jail.local <<CONF
bantime  = ${BAN_TIME}
findtime = 10m
maxretry = ${MAX_RETRY}
ignoreip = ${IGNORE_IP}

[sshd]
enabled = true
CONF

echo "wrote /etc/fail2ban/jail.local"

echo
echo "== Starting =="
systemctl enable --now fail2ban
sleep 2

echo
echo "== Status =="
fail2ban-client status sshd || {
  echo "The sshd jail is not up. Check: journalctl -u fail2ban -n 50" >&2
  exit 1
}

echo
echo "Done. Unban an address with: fail2ban-client set sshd unbanip <IP>"
