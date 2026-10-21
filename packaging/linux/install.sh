#!/bin/sh
# Installs LedgerCraft for the current user (no root needed):
#   ~/.local/bin/ledgercraft, ~/.local/bin/ledgercraft-cli
#   an Applications menu entry with the LedgerCraft icon
#   the .lcbooks file type, so double-clicking a company file opens it
# Run from the unpacked release folder: ./install.sh
# To remove: ./install.sh --uninstall
set -eu
here=$(cd "$(dirname "$0")" && pwd)
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
apps="$data/applications"
icons="$data/icons/hicolor/256x256/apps"
mime_icons="$data/icons/hicolor/256x256/mimetypes"
mime="$data/mime"

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$bin/ledgercraft" "$bin/ledgercraft-cli" \
          "$apps/ledgercraft.desktop" "$icons/ledgercraft.png" \
          "$mime_icons/application-x-ledgercraft.png" "$mime/packages/ledgercraft.xml"
    command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$mime" || true
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true
    echo "LedgerCraft removed. Your company files and preferences were not touched."
    exit 0
fi

mkdir -p "$bin" "$apps" "$icons" "$mime_icons" "$mime/packages"
cp "$here/ledgercraft" "$here/ledgercraft-cli" "$bin/"
chmod 755 "$bin/ledgercraft" "$bin/ledgercraft-cli"
cp "$here/ledgercraft.png" "$icons/ledgercraft.png"
cp "$here/ledgercraft.png" "$mime_icons/application-x-ledgercraft.png"
cp "$here/ledgercraft-mime.xml" "$mime/packages/ledgercraft.xml"
sed "s|^Exec=ledgercraft|Exec=$bin/ledgercraft|" "$here/ledgercraft.desktop" > "$apps/ledgercraft.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$apps" || true
command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$mime" || true
command -v xdg-mime >/dev/null 2>&1 && xdg-mime default ledgercraft.desktop application/x-ledgercraft || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q -t "$data/icons/hicolor" 2>/dev/null || true

echo "LedgerCraft installed. Find it in your Applications menu, or run: ledgercraft"
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "Note: $bin is not on your PATH; add it to run ledgercraft from a terminal." ;;
esac
