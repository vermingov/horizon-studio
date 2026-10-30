#!/bin/sh
# Horizon Studio's MCP server for AI agents, in one line:
#   curl -fsSL https://raw.githubusercontent.com/vermingov/horizon-studio/main/mcp/install.sh | sh
# Downloads horizon-studio-mcp for this machine from the latest release,
# checks it against the release's SHA256SUMS, puts it in ~/.local/bin and
# registers it with the agents installed here (Claude Code, omp, Codex,
# Cursor, Gemini CLI, Claude Desktop, OpenCode). Run it again to update.
set -eu

REPO="${HORIZON_STUDIO_REPO:-vermingov/horizon-studio}"
BIN_DIR="${HORIZON_STUDIO_BIN:-$HOME/.local/bin}"

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64 | Linux-amd64) target=x86_64-unknown-linux-musl ;;
  Linux-aarch64 | Linux-arm64) target=aarch64-unknown-linux-musl ;;
  Darwin-arm64) target=aarch64-apple-darwin ;;
  Darwin-x86_64) target=x86_64-apple-darwin ;;
  *)
    echo "No ready-made horizon-studio-mcp for $(uname -s) $(uname -m). With Rust installed:" >&2
    echo "  cargo install --git https://github.com/$REPO horizon-studio --bin horizon-studio-mcp && horizon-studio-mcp install" >&2
    exit 1
    ;;
esac

asset="horizon-studio-mcp-$target.tar.gz"
base="${HORIZON_STUDIO_RELEASE:-https://github.com/$REPO/releases/latest/download}"
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

fetch() {
  if command -v curl >/dev/null 2>&1; then
    curl -fsSL "$1" -o "$2"
  else
    wget -qO "$2" "$1"
  fi
}

echo "Downloading $asset ..."
fetch "$base/$asset" "$tmp/$asset"
fetch "$base/SHA256SUMS" "$tmp/SHA256SUMS"
want=$(awk -v f="$asset" '$2 == f { print $1 }' "$tmp/SHA256SUMS")
if command -v sha256sum >/dev/null 2>&1; then
  got=$(sha256sum "$tmp/$asset" | awk '{ print $1 }')
else
  got=$(shasum -a 256 "$tmp/$asset" | awk '{ print $1 }')
fi
if [ -z "$want" ] || [ "$want" != "$got" ]; then
  echo "The download doesn't match the release's checksum: not installing it." >&2
  exit 1
fi

tar -xzf "$tmp/$asset" -C "$tmp"
mkdir -p "$BIN_DIR"
cp "$tmp/horizon-studio-mcp" "$BIN_DIR/horizon-studio-mcp.new"
chmod 755 "$BIN_DIR/horizon-studio-mcp.new"
mv -f "$BIN_DIR/horizon-studio-mcp.new" "$BIN_DIR/horizon-studio-mcp"
echo "Installed $("$BIN_DIR/horizon-studio-mcp" --version) in $BIN_DIR"
echo
"$BIN_DIR/horizon-studio-mcp" install
