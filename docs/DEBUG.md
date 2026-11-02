# Debug & Deploy

## SSH access

```bash
ssh luchhh@juniorpi.local
```

The Pi runs a self-hosted GitHub Actions runner and the `junior` systemd service.

---

## Agent iteration workflow (fast)

Use this when making code changes that need testing on the robot. No commits required.

**1. Sync code to Pi**
```bash
rsync -av --exclude target --exclude .git /path/to/junior-veecle/ luchhh@juniorpi.local:~/junior-veecle/
```

**2. Build on Pi**
```bash
ssh luchhh@juniorpi.local "cd ~/junior-veecle && ~/.cargo/bin/cargo build --bin junior-audio 2>&1"
```

**3. Restart the service**
```bash
ssh luchhh@juniorpi.local "sudo systemctl restart junior"
```

**4. Watch logs**
```bash
ssh luchhh@juniorpi.local "journalctl -u junior -f"
```

Or from the agent side, run step 4 with a timeout and ask the user to trigger the robot during that window.

---

## Production deploy

Push to `main` → GitHub Actions runs tests (Ubuntu), then deploys on the Pi (self-hosted arm64 runner): pulls latest, builds, restarts service.

Manual trigger available via `workflow_dispatch` on any branch from the GitHub UI.

---

## Useful service commands

```bash
sudo systemctl status junior    # check current state
sudo systemctl stop junior      # stop before running manually
journalctl -u junior -n 100     # last 100 log lines
journalctl -u junior -f         # follow live
```
