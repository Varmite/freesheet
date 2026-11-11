#!/usr/bin/env bash
# FreeSheet accounts:  ./users.sh list | add <name> | passwd <name> | remove <name>
cd "$(dirname "$0")" && exec node users.js "$@"
