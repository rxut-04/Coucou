#!/bin/bash
# Install terminal hook into shell config files (idempotent)

HOOK_LINE='# Coucou terminal finish sensor'
MARKER='coucou_terminal_hook'

install_to_file() {
    local file="$1"
    if [ ! -f "$file" ]; then
        return
    fi
    
    # Check if already installed
    if grep -q "$MARKER" "$file"; then
        echo "Already installed in $file"
        return
    fi
    
    # Append hook
    echo "" >> "$file"
    echo "$HOOK_LINE" >> "$file"
    echo "# $MARKER" >> "$file"
    echo 'source ~/.local/share/coucou/scripts/terminal-hook.sh' >> "$file"
    echo "Installed to $file"
}

# Install to bashrc
install_to_file "$HOME/.bashrc"

# Install to zshrc
install_to_file "$HOME/.zshrc"

echo "Terminal hook installed. Restart your shell to activate."
