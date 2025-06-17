#!/usr/bin/env bash

set -e

REPO_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && realpath .)

node $REPO_DIR/scripts/run-yarn.mjs "$@"
