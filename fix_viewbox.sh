#!/bin/bash
# Fix viewBox to view_box in main.rs
sed -i '' 's/viewBox:/view_box:/g' /Users/martinmaurer/Projects/mDesk_new/src/main.rs
