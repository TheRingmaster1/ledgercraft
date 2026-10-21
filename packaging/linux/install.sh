#!/bin/sh
# Installs LedgerCraft for the current user (no root needed):
#   ~/.local/bin/ledgercraft, ~/.local/bin/ledgercraft-cli
#   an Applications menu entry with the LedgerCraft icon
# Run from the unpacked release folder: ./install.sh
# To remove: ./install.sh --uninstall
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
apps="$data/applications"
icons="$data/icons/hicolor/256x256/apps"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$bin/ledgercraft" "$bin/ledgercraft-cli" \
          "$apps/ledgercraft.desktop" "$icons/ledgercraft.png"
    echo "LedgerCraft removed. Your company files and preferences were not touched."
    exit 0
fi

mkdir -p "$bin" "$apps" "$icons"
cp "$here/ledgercraft" "$here/ledgercraft-cli" "$bin/"
chmod 755 "$bin/ledgercraft" "$bin/ledgercraft-cli"
cp "$here/ledgercraft.png" "$icons/ledgercraft.png"
sed "s|^Exec=ledgercraft|Exec=$bin/ledgercraft|" "$here/ledgercraft.desktop" > "$apps/ledgercraft.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$data/icons/hicolor" || true

echo "LedgerCraft installed. Find it in your Applications menu, or run: ledgercraft"
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "Note: $bin is not on your PATH; add it to run ledgercraft from a terminal." ;;
esac
