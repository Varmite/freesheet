#!/usr/bin/env bash
# CI only: install Ubuntu packages without hanging on a bad package mirror.
#   bash scripts/ci-apt.sh <package>...     (no packages: just make apt-get update work)
# GitHub's runners use an Azure mirror. When it was unreachable, apt waited for over
# 30 minutes. Each try is time-limited, and from the second try on, apt uses Ubuntu's
# own mirror instead.
set -u
opts=(-o Acquire::Retries=3 -o Acquire::http::Timeout=30 -o DPkg::Lock::Timeout=120)
for try in 1 2 3; do
  if [ "$try" -ge 2 ]; then
    echo "Switching apt to archive.ubuntu.com"
    sudo sed -i 's|mirror+file:/etc/apt/apt-mirrors.txt|http://archive.ubuntu.com/ubuntu/|g' /etc/apt/sources.list /etc/apt/sources.list.d/*.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true
  fi
  if timeout 240 sudo apt-get "${opts[@]}" update &&
     { [ $# -eq 0 ] || timeout 480 sudo apt-get "${opts[@]}" install -y "$@"; }; then
    exit 0
  fi
  echo "apt attempt $try failed"
done
exit 1
