#!/bin/bash

# Path to source files directory
SRC_DIR="/Users/martinmaurer/Projects/mDesk_new/src"

# Output file
OUTPUT_FILE="${SRC_DIR}/main.rs"

# Create a backup of the original main.rs file
if [ -f "$OUTPUT_FILE" ]; then
  cp "$OUTPUT_FILE" "${OUTPUT_FILE}.bak"
  echo "Created backup of main.rs at ${OUTPUT_FILE}.bak"
fi

# Combine all part files in order
cat "${SRC_DIR}/main.rs" > "$OUTPUT_FILE"
if [ -f "${SRC_DIR}/main.rs.part2" ]; then
  cat "${SRC_DIR}/main.rs.part2" >> "$OUTPUT_FILE"
fi
if [ -f "${SRC_DIR}/main.rs.part3" ]; then
  cat "${SRC_DIR}/main.rs.part3" >> "$OUTPUT_FILE"
fi
if [ -f "${SRC_DIR}/main.rs.part4" ]; then
  cat "${SRC_DIR}/main.rs.part4" >> "$OUTPUT_FILE"
fi
if [ -f "${SRC_DIR}/main.rs.part5" ]; then
  cat "${SRC_DIR}/main.rs.part5" >> "$OUTPUT_FILE"
fi

echo "Combined all parts into main.rs"

# Optionally remove the part files if successful
read -p "Remove all part files? (y/n): " remove_parts
if [ "$remove_parts" == "y" ]; then
  rm -f "${SRC_DIR}/main.rs.part"*
  echo "Removed all part files"
fi

echo "Done!"
