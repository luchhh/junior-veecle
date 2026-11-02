# Agent Instructions

## Git workflow

Direct pushes to `main` are blocked. Always use a branch and open a PR.

```bash
git checkout -b your-branch
git push -u origin your-branch
gh pr create
```

Merging to `main` triggers the production deploy automatically via GitHub Actions.

## Deploy & debug

See [docs/DEBUG.md](docs/DEBUG.md) for how to deploy changes to the robot and read logs.

## Memory feature

See [docs/MEMORY.md](docs/MEMORY.md) for the planned memory architecture (long-term + session context).
