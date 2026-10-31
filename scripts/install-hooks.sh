#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Point Git at the checked-in hooks. Run once per clone.
set -eu

root=$(git rev-parse --show-toplevel)
cd "$root"

git config --local core.hooksPath .githooks
chmod +x .githooks/pre-commit
echo "Installed Git hooks from .githooks"
