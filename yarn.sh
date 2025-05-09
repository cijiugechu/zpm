#!/usr/bin/env bash

set -e

REPO_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && realpath ..)

node ./scripts/run-yarn.mjs "$@"
