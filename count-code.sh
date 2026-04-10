#!/bin/bash
# @amadeus-header
# summary: Repository code counting helper for tracked source files and totals.
# layer: script
# status: active
# feature_flags:
# - full
# provides:
# - cmd: count-code.sh
# uses:
# - cmd: cloc
# invariants:
# - Script command flow remains non-interactive and order-dependent.
# side_effects:
# - Reads filesystem state.
# - Runs external commands or subprocesses.
# - Writes output to stdout or stderr.
# tests:
# - cmd: bash ./count-code.sh
# @end-amadeus-header


# --- Script Configuration & Robustness ---
# Exit immediately if a command exits with a non-zero status.
set -e
# Treat unset variables as an error.
set -u

# --- Configuration Variables ---

# Common directories to exclude (e.g., dependencies, build artifacts)
EXCLUDE_DIRS="node_modules,vendor,dist,build,.git,target,temp,tmp,out,coverage,docs,doc,.venv,target,refs"

# Languages to exclude (mostly configuration, documentation, and lock files)
# Cloc recognizes these as "languages" but they are not programming code.
EXCLUDE_LANGS="JSON,YAML,TOML,Markdown,Log,Lisp,INI,CSV,XML,Git config,SQL,PowerShell,DTrace,XAML,SVG"

# Default directory to scan (current directory)
TARGET_DIR="."

# --- Helper Functions ---

# Function to check if cloc is installed
check_dependency() {
	if ! command -v cloc &>/dev/null; then
		echo "🚨 ERROR: 'cloc' command not found." >&2
		echo "Please install it using Homebrew: 'brew install cloc'" >&2
		exit 1
	fi
}

# Function to display usage
show_usage() {
	echo "Usage: $(basename "$0") [DIRECTORY]"
	echo "Counts lines of code in the specified DIRECTORY (defaults to current directory)."
	echo ""
	echo "Excludes common directories ($EXCLUDE_DIRS) and configuration languages."
	echo ""
}

# --- Main Script Logic ---

# Check for help flag
if [[ "$#" -gt 0 && ("$1" == "-h" || "$1" == "--help") ]]; then
	show_usage
	exit 0
fi

# Assign target directory from argument, if provided
if [ "$#" -gt 0 ]; then
	TARGET_DIR="$1"
fi

# Validate target directory
if [ ! -d "$TARGET_DIR" ]; then
	echo "🚨 ERROR: Directory '$TARGET_DIR' not found." >&2
	show_usage
	exit 1
fi

check_dependency

echo "--- Code Line Count Report ---"
echo "Target Directory: $(realpath "$TARGET_DIR")"
echo "Excluded Dirs:    $EXCLUDE_DIRS"
echo "Excluded Configs: $EXCLUDE_LANGS"
echo "------------------------------"
echo ""

# The main cloc command
# --exclude-dir: Skips large, unneeded folders
# --exclude-lang: Skips configuration/doc languages
# --quiet: Suppresses scanning messages for a cleaner report
cloc "$TARGET_DIR" \
	--exclude-dir="$EXCLUDE_DIRS" \
	--exclude-lang="$EXCLUDE_LANGS" \
	--quiet

echo ""
echo "------------------------------"
echo "✅ cloc completed."
