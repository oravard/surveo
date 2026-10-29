#!/usr/bin/env bash
# Prototype 1 (IMPL.md, phase 2) : mesure du relais.
#   ./mesure.sh [nombre d'agents] [durée en secondes]
# Lance le relais, N agents simulés (qui trouvent le relais par son
# annonce), une console simulée, puis vérifie qu'un agent d'un autre
# établissement est refusé.
set -euo pipefail
cd "$(dirname "$0")"
N="${1:-30}"
D="${2:-30}"
BIN=target/release

cargo build --release --quiet
[ -f keys/etab.json ] || "$BIN/keygen" --out keys --agents 300

"$BIN/relay" --keys keys --port 8443 &
RELAY=$!
trap 'kill $RELAY $AGENTS 2>/dev/null || true' EXIT
sleep 1

"$BIN/agent-sim" --keys keys --count "$N" &
AGENTS=$!
sleep 3

"$BIN/console-sim" --keys keys --relay 127.0.0.1:8443 --duration "$D"

echo
echo "Agent d'un autre établissement :"
"$BIN/agent-sim" --keys keys --identities keys/autre/agents.json --count 1 --relay 127.0.0.1:8443 &
INTRUS=$!
sleep 2
kill $INTRUS 2>/dev/null || true
sleep 1
