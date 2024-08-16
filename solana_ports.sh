#!/bin/bash

# Define the ports
PORTS="8001 1028 1024 1025 5009"

open_ports() {
    echo "Opening UDP ports: $PORTS"
    echo "pass in proto udp from any to any port { $PORTS }" | sudo pfctl -ef -
}

close_ports() {
    echo "Closing UDP ports"
    sudo pfctl -f /etc/pf.conf
}

check_ports() {
    echo "Checking UDP ports"
    sudo nmap -sU -p $PORTS localhost
}

case "$1" in
    open)
        open_ports
        ;;
    close)
        close_ports
        ;;
    check)
	check_ports
	;;
    *)
        echo "Usage: $0 {open|close}"
        exit 1
        ;;
esac

exit 0
