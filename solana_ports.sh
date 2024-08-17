#!/bin/bash

# Define the ports
PORTS="8001 8002 8003 8004 8005 8006 8007 8008 8009 8010 8011 8012 8013 8014 8015 8016 8017 8018 8019 8020"

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
