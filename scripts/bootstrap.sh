#!/bin/sh
# bap bootstrap: one command installs the binary and configures the machine.
#
#   curl -fsSL https://raw.githubusercontent.com/Jaketdaniels/bap/main/scripts/bootstrap.sh | sh
#
# Optional: pass through flags to `bap install`, e.g.  ... | sh -s -- --zshrc
set -eu

REPO="Jaketdaniels/bap"
PREFIX="${BAP_PREFIX:-$HOME/.bap}"
BIN_DIR="$PREFIX/bin"

log() { printf '%s\n' "$*"; }
fail() { printf 'bootstrap: %s\n' "$*" >&2; exit 1; }

# --- download helper -------------------------------------------------------
fetch() {
    url=$1
    dest=$2
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL -o "$dest" "$url" || fail "download failed: $url"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "$dest" "$url" || fail "download failed: $url"
    else
        fail "need curl or wget to download bap"
    fi
}

# --- platform detection ----------------------------------------------------
case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) fail "unsupported platform: $(uname -s). Grab an archive from https://github.com/$REPO/releases" ;;
esac
case "$(uname -m)" in
    arm64 | aarch64)
        [ "$os" = "apple-darwin" ] && arch=aarch64 || arch=aarch64-unknown-linux-gnu
        ;;
    x86_64) arch=x86_64 ;;
    *) fail "unsupported architecture: $(uname -m)" ;;
esac

if [ "$(uname -s)" = "Darwin" ]; then
    triple="$arch-$os"
elif [ "$arch" = "aarch64-unknown-linux-gnu" ]; then
    triple="$arch"
else
    triple="$arch-$os"
fi

archive="bap-$triple.tar.xz"
base="https://github.com/$REPO/releases/latest/download"

log "bap bootstrap: $triple"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

fetch "$base/$archive" "$work/$archive"
fetch "$base/$archive.sha256" "$work/$archive.sha256"

# --- verify -----------------------------------------------------------------
want=$(sed "s/^[0-9a-f]*[[:space:]]*[*]\?//" "$work/$archive.sha256" >/dev/null 2>&1 && \
       awk '{print $1}' "$work/$archive.sha256" | head -n1)
[ -n "$want" ] || fail "could not read checksum"
have=$( (sha256sum "$work/$archive" 2>/dev/null || shasum -a 256 "$work/$archive") | awk '{print $1}')
[ "$have" = "$want" ] || fail "checksum mismatch for $archive (expected $want, got $have)"

# --- unpack + install -------------------------------------------------------
tar -xJf "$work/$archive" -C "$work"
mkdir -p "$BIN_DIR"
src=$(find "$work" -type f -name bap -perm +111 | head -n1)
[ -n "$src" ] || src=$(find "$work" -type f -name bap | head -n1)
[ -n "$src" ] || fail "archive did not contain a bap binary"
mv "$src" "$BIN_DIR/bap"
chmod +x "$BIN_DIR/bap"

log "installed $($BIN_DIR/bap --version) -> $BIN_DIR/bap"

# --- configure --------------------------------------------------------------
# Older releases may predate the setup/install commands; degrade quietly.
if "$BIN_DIR/bap" help setup >/dev/null 2>&1; then
    "$BIN_DIR/bap" setup "$@"
elif "$BIN_DIR/bap" help install >/dev/null 2>&1; then
    "$BIN_DIR/bap" install "$@"
else
    log "note: this bap version has no shell-setup command; skipped"
fi

case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        log ""
        log "one more step: add bap to your PATH"
        log "  export BAP_BIN=\"$BIN_DIR\"; export PATH=\"\$BAP_BIN:\$PATH\"   # then add it to ~/.zshrc or ~/.bashrc"
        ;;
esac
