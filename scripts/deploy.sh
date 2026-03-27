#!/usr/bin/env bash
# Deploy a branch to Junior. Defaults to the current branch.
# Usage: ./scripts/deploy.sh [branch]

BRANCH="${1:-$(git rev-parse --abbrev-ref HEAD)}"

echo "==> Deploying branch '$BRANCH' to Junior..."
gh workflow run deploy.yml --ref "$BRANCH"
