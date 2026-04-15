#!/bin/bash
set -e

RULES_FILE="/etc/udev/rules.d/99-idcooling-lcd.rules"
RULE='SUBSYSTEM=="usb", ATTR{idVendor}=="2000", ATTR{idProduct}=="3000", MODE="0666", TAG+="uaccess"'

if [ "$(id -u)" -ne 0 ]; then
    echo "This script must be run as root. Try: sudo $0"
    exit 1
fi

echo "$RULE" > "$RULES_FILE"
udevadm control --reload-rules
udevadm trigger

echo "udev rules installed to $RULES_FILE"
echo "If the cooler is still not detected, reboot so the new device permissions are applied cleanly."
