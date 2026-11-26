#!/bin/bash
# Lets only the worlds' UDP ports in from the internet to the containers Docker publishes.
#
# Docker publishes ports with NAT rules that UFW never sees, so a published port is open to the
# world whatever UFW says. Docker leaves its DOCKER-USER chain to the administrator, and the
# packets forwarded to a container cross it after their destination was rewritten. A rule on
# `--dport` would see the container's port (8000, 8001), so the game ports are matched by the
# destination the packet had before Docker's NAT (`--ctorigdstport`).
#
# The rules are in a chain of their own, which DOCKER-USER jumps to for what comes in on the WAN
# interface, so that running this again changes nothing. The jump catches every new connection
# forwarded from the WAN, to any container Docker runs, not only to the worlds: what is not a
# world's port is dropped. Packets that leave through the WAN, their replies, and the traffic
# between containers and the host, never match it.
#
# `rustcraft-docker-firewall.sh remove` takes the jump and the chain away again.
#
# Environment: WAN_IFACE (default eth0), WORLD_PORTS (default 25000:25039: the agent's base_port
# to base_port + 2 * slots - 1).
set -euo pipefail

wan=${WAN_IFACE:-eth0}
ports=${WORLD_PORTS:-25000:25039}
chain=RUSTCRAFT-WAN

remove() {
    local ipt=$1
    "$ipt" -n -L DOCKER-USER >/dev/null 2>&1 || return 0
    while "$ipt" -D DOCKER-USER -i "$wan" -j "$chain" 2>/dev/null; do :; done
    "$ipt" -F "$chain" 2>/dev/null && "$ipt" -X "$chain" || true
}

apply() {
    local ipt=$1
    if ! "$ipt" -n -L DOCKER-USER >/dev/null 2>&1; then
        # Without Docker's chain there is no forwarding to containers to guard. Docker may have
        # no IPv6 firewall at all, but a missing IPv4 chain means Docker is not up.
        if [ "$ipt" = ip6tables ]; then
            echo "$ipt: no DOCKER-USER chain, nothing to restrict" >&2
            return 0
        fi
        echo "$ipt: no DOCKER-USER chain: is docker.service running?" >&2
        return 1
    fi
    "$ipt" -N "$chain" 2>/dev/null || "$ipt" -F "$chain"
    "$ipt" -A "$chain" -m conntrack --ctstate ESTABLISHED,RELATED -j RETURN
    "$ipt" -A "$chain" -p udp -m conntrack --ctorigdstport "$ports" -j RETURN
    "$ipt" -A "$chain" -j DROP
    "$ipt" -C DOCKER-USER -i "$wan" -j "$chain" 2>/dev/null ||
        "$ipt" -I DOCKER-USER 1 -i "$wan" -j "$chain"
}

action=apply
if [ "${1:-}" = remove ]; then action=remove; fi
$action iptables
$action ip6tables
