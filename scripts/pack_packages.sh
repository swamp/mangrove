#!/bin/bash
set -x

# --- Configuration ---
SOURCE_DIR="$HOME/.swamp-dev/packages"
TARGET_BASE_DIR="$HOME/tmp"
TARGET_DIR="$TARGET_BASE_DIR/target"
TARGET_PACKAGES_DIR="$TARGET_DIR/packages"
ARCHIVE_NAME="packages.tar.gz"
ARCHIVE_PATH="$TARGET_BASE_DIR/$ARCHIVE_NAME"

echo "Starting script to create archive..."

mkdir -p "$TARGET_BASE_DIR"

if [ -d "$TARGET_DIR" ]; then
  chmod -R a+w "$TARGET_DIR"
  echo "Removing existing target directory: $TARGET_DIR"
  rm -rf "$TARGET_DIR"
fi

echo "Creating target directory: $TARGET_DIR"
mkdir -p "$TARGET_DIR"

echo "Copying source directory: $SOURCE_DIR to $TARGET_DIR"
cp -R -L "$SOURCE_DIR" "$TARGET_DIR" # do not preserve symlinks

# Check if copy was successful
if [ $? -ne 0 ]; then
  echo "Error: Failed to copy source directory."
  exit 1
fi

echo "Setting read-only permissions in target directory: $TARGET_DIR"
chmod -R a-w "$TARGET_DIR"
if [ $? -ne 0 ]; then
  echo "Error: Failed to set read-only permissions."
  exit 1
fi

pushd "$TARGET_PACKAGES_DIR" > /dev/null

echo "Creating archive: $ARCHIVE_PATH"
tar -czvhf "$ARCHIVE_PATH" .
if [ $? -ne 0 ]; then
  echo "Error: Failed to create archive."
  popd > /dev/null
  exit 1
fi

popd > /dev/null

echo "Archive created successfully: $ARCHIVE_PATH"
echo "Script finished."

exit 0
