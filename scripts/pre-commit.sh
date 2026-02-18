#!/bin/sh
# Format all code
cargo fmt

# Re-add all staged rust files to include formatting changes
# If there are staged rust files, add them again to capture formatting changes
FILES=$(git diff --name-only --cached | grep '\.rs$')
if [ -n "$FILES" ]; then
    for FILE in $FILES; do
        if [ -f "$FILE" ]; then
            git add "$FILE"
        fi
    done
fi

# Ensure no clippy warnings are present
# cargo clippy -- -D warnings
