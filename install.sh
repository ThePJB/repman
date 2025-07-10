#!/bin/bash

# Simple install script for repman
# Builds the project and copies binary to ~/bin

set -e  # Exit on any error

echo "Building repman..."
cargo build --release

echo "Creating ~/bin directory if it doesn't exist..."
mkdir -p ~/bin

# Detect OS and set binary name
if [[ "$OSTYPE" == "msys" || "$OSTYPE" == "win32" || "$OS" == "Windows_NT" ]]; then
    BINARY_NAME="repman.exe"
else
    BINARY_NAME="repman"
fi

echo "Installing $BINARY_NAME to ~/bin..."
cp "target/release/$BINARY_NAME" "~/bin/$BINARY_NAME"

echo "Making executable..."
chmod +x "~/bin/$BINARY_NAME"

echo ""
echo "✓ repman installed successfully!"
echo ""
echo "Make sure ~/bin is in your PATH:"
echo "  export PATH=\"\$HOME/bin:\$PATH\""
echo ""
echo "Then you can use: repman --help" 