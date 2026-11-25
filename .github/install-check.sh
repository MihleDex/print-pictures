#!/bin/sh
# Installs the built package on clean systems and checks that no file manager
# came with it. Ubuntu 24.04 is what Mint 22 and Zorin OS 18 are built on.
set -eu
for image in ubuntu:24.04 debian:13 ubuntu:26.04; do
    echo "== $image"
    # Noninteractive: a package pulled in along the way must not ask questions.
    docker run --rm -e DEBIAN_FRONTEND=noninteractive -v "$PWD/target/debian:/d" "$image" sh -c \
        "apt-get update -qq && apt-get install -y -qq /d/*.deb && print-pictures --version && ! dpkg -l caja nautilus nemo dolphin thunar 2>/dev/null | grep -q '^ii'"
done
