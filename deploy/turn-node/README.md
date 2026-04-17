# oxpulse-chat TURN Relay — Partner Node

One-command installer for a production-grade coturn relay that participates in
the oxpulse.chat TURN pool. Tested on Debian 12, Ubuntu 22.04 / 24.04,
AlmaLinux 9, Rocky Linux 9, CentOS Stream 9, RHEL 9.

## Quick start (fresh VM)

```bash
# 1. Log in as root on a freshly provisioned VM with a public IPv4.
#    Minimum: 1 vCPU, 2 GB RAM, 20 GB disk.

# 2. Run the installer (replace with your secret out-of-band):
curl -fsSL https://github.com/anatolykoptev/oxpulse-chat/releases/latest/download/turn-node-installer.sh \
  | TURN_SECRET='<shared-secret>' REGION='ru-msk' bash

# 3. Verify:
systemctl status coturn
/usr/local/sbin/oxpulse-turn-healthcheck
```

That's it. The installer is idempotent — re-running it upgrades the config
and restarts coturn safely.

## Environment variables (for install and later edits)

| Var            | Required | Default                | Notes                                                                 |
|----------------|----------|------------------------|-----------------------------------------------------------------------|
| `TURN_SECRET`  | yes      | —                      | Shared across the whole fleet. Delivered out-of-band by the operator. |
| `REGION`       | yes      | —                      | Operator-assigned tag (`ru-msk`, `de-fra`, `sg-sin`, ...).            |
| `PUBLIC_IPV4`  | no       | autodetect             | Override if autodetect picks a wrong address.                         |
| `PRIVATE_IPV4` | no       | autodetect (if behind NAT) | Cloud VMs (DO, Hetzner, Vultr) with 1:1 NAT need this.            |
| `REALM`        | no       | `oxpulse.chat`         | Rarely changed.                                                       |
| `PRIORITY`     | no       | `10`                   | Registration priority (lower = preferred).                            |

These are persisted to `/etc/default/oxpulse-turn` at install time. Editing
that file + `systemctl restart coturn` is the supported way to change values
later.

## Cloning to another region

1. Run `install.sh` on the first node (e.g. `call.rvpn.online`).
2. Verify health. Take a VM snapshot at your cloud provider.
3. Clone the snapshot into the new region.
4. SSH into the clone and run:
   ```bash
   vi /etc/default/oxpulse-turn   # update REGION, clear PUBLIC_IPV4 so it autodetects
   systemctl restart coturn
   /usr/local/sbin/oxpulse-turn-healthcheck
   ```
5. Send the operator the registration line printed by the healthcheck.

## Upgrading

Pull + verify + apply the latest release:

```bash
oxpulse-turn-upgrade           # latest
oxpulse-turn-upgrade --check   # check without applying (exit 10 if upgrade pending)
oxpulse-turn-upgrade turn-node-v1.2.3   # pin to specific version
```

Enable nightly auto-check (opt-in — disabled by default):

```bash
systemctl enable --now oxpulse-turn-upgrade.timer
```

## Uninstall

```bash
systemctl disable --now coturn oxpulse-turn-render.service
rm -f /etc/turnserver.conf /etc/default/oxpulse-turn \
      /usr/local/sbin/oxpulse-turn-render /usr/local/sbin/oxpulse-turn-healthcheck \
      /etc/systemd/system/oxpulse-turn-render.service \
      /etc/systemd/system/coturn.service.d/override.conf
systemctl daemon-reload
# coturn package left installed — remove with apt/dnf if desired.
```

## Relationship to the operator runbook

The high-level flow (why a TURN relay, credential format, drain procedure,
incident response) lives in `docs/partners/onboarding.md`. This README is
strictly the mechanical how-to for the one-command install.
