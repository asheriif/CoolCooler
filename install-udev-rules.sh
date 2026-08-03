#!/bin/bash
set -e

RULES_FILE="/etc/udev/rules.d/70-coolcooler.rules"
LEGACY_RULES_FILE="/etc/udev/rules.d/99-idcooling-lcd.rules"
RULE='SUBSYSTEM=="hidraw", ATTRS{idVendor}=="2000", ATTRS{idProduct}=="3000", TAG+="uaccess"'

if [ "$(id -u)" -ne 0 ]; then
    echo "This script must be run as root. Try: sudo $0"
    exit 1
fi

echo "$RULE" > "$RULES_FILE"
rm -f "$LEGACY_RULES_FILE"
udevadm control --reload-rules
udevadm trigger --subsystem-match=hidraw

echo "udev rule installed to $RULES_FILE"
echo "If the cooler is still not detected, reboot so the new device permissions are applied cleanly."
