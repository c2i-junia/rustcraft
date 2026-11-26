#!/bin/bash
# Asks a Rustcraft server for its status (the packet of crates/protocol/src/status.rs) and prints
# its players. For a hosted world, the status port is the game port + 1.
#   probe-status.sh play.rustcraft.dev 25001
# Fails after 3 seconds without a well-formed reply. Needs python3.
set -euo pipefail
[ $# -eq 2 ] || { echo "usage: $0 <host or address> <status port>" >&2; exit 2; }
exec python3 -I - "$1" "$2" <<'PY'
import socket, sys

host, port = sys.argv[1], int(sys.argv[2])
family, _, _, _, address = socket.getaddrinfo(host, port, type=socket.SOCK_DGRAM)[0]
sock = socket.socket(family, socket.SOCK_DGRAM)
sock.settimeout(3)
# magic, format 1, kind 0 (request), an 8-byte nonce, zero padding up to 32 bytes
sock.sendto(b"RCST\x01\x00" + b"\x07" * 8 + bytes(18), address)
try:
    reply = sock.recv(64)
except socket.timeout:
    sys.exit(f"no answer from {host} port {port}")
if len(reply) != 32 or reply[:4] != b"RCST" or reply[4] != 1 or reply[5] != 1 or reply[6:14] != b"\x07" * 8:
    sys.exit(f"not a status reply: {reply!r}")
players = int.from_bytes(reply[22:26], "little")
maximum = int.from_bytes(reply[26:30], "little")
print(f"{host} port {port}: {players} of {maximum} players")
PY
