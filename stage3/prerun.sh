#!/bin/bash -e
# stage3/prerun.sh — inherit the rootfs built by the previous stage.
#
# pi-gen runs this before stage3's sub-stages. Without copy_previous the
# stage3 rootfs never exists and every sub-stage silently no-ops, so the
# image export fails with "stage3/rootfs: No such file or directory".

if [ ! -d "${ROOTFS_DIR}" ]; then
	copy_previous
fi
