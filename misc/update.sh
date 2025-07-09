#!/usr/bin/env bash

# remove all build hashes from nix flake
sed -i 's/\(^\ *hash = \)"\(sha256-.\+\)";$/\1"";/' flake.nix
echo -e "\033[42;30m updating dependencies\033[0m"
nix flake update
cargo update
pnpm update
echo -e "\033[42;30mupdating package hashes, this might take a while ...\033[0m"
NEW_HASH=$(nix build |& awk '/got/{print $NF}')
sed -i "s/\(^\ *hash = \)\"\";\$/\1\"$NEW_HASH\";/" flake.nix
echo -e "\033[42;30mvalidating build\033[0m"
nix build
echo -e "\033[42;30mstaging git changes\033[0m"
git add Cargo.lock flake.lock flake.nix package.json pnpm-lock.yaml
