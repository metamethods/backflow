#!/bin/bash -x
# ../dbus/
DBUS_PROTOS_DIR="../dbus/"

for file in "$DBUS_PROTOS_DIR"*.xml; do
    if [ -f "$file" ]; then
        echo "Processing $file"
        zbus-xmlgen file $file
    else
        echo "No files found in $DBUS_PROTOS_DIR"
    fi
done
