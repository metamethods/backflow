#!/bin/bash -x
# ../dbus/
DBUS_PROTOS_DIR="../dbus/"

for file in "$DBUS_PROTOS_DIR"*.xml; do
    if [ -f "$file" ]; then
        echo "Processing $file"
        if [[ "$file" == *".Source"* ]]; then
            pushd source/ || exit
            zbus-xmlgen file "$file"
            popd || exit
        else
            zbus-xmlgen file "$file"
        fi
    else
        echo "No files found in $DBUS_PROTOS_DIR"
    fi
done
