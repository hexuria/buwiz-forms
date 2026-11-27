#!/usr/bin/env bash
# Update apt's indexes without letting a third-party package source stop the job.
#
# GitHub's Ubuntu runner images ship source lists we never install from (Microsoft's, for example). apt-get update
# refreshes every one of them, so a 403 or broken InRelease answer there fails the job before the Ubuntu packages we do
# need can install. This script first sets aside every source list under /etc/apt/sources.list.d that names a URI outside
# Ubuntu's archive/security/ports hosts and the runner's own /etc/apt/apt-mirrors.txt mirror list, then retries the real
# update. Sources are moved to APT_SOURCE_SET_ASIDE_DIR (not deleted), so the step is safe to run again.
set -euo pipefail

# The URI host, lower-case. mirror+file is reported as itself: it is apt's indirection to the runner mirror list.
apt_uri_host() {
  local uri="$1" rest host
  case "$uri" in
    mirror+file:*)
      printf '%s\n' "$uri"
      return
      ;;
    *://*)
      rest="${uri#*://}"
      rest="${rest#*@}"
      host="${rest%%/*}"
      host="${host%%:*}"
      printf '%s\n' "$host" | tr '[:upper:]' '[:lower:]'
      ;;
    *)
      printf '\n'
      ;;
  esac
}

# One awk reader for both apt source syntaxes: deb/deb-src one-line .list files and deb822 .sources files. It prints
# every URI token; a deb822 URIs field may continue on indented lines, and a .list entry may carry a bracketed option
# list before its URI.
apt_source_file_uris() {
  awk '
    function emit(line,   parts, count, i) {
      count = split(line, parts, /[[:space:]]+/)
      for (i = 1; i <= count; i++) {
        if (parts[i] != "") print parts[i]
      }
    }
    /^[[:space:]]*$/ { in_uris = 0; next }
    /^[[:space:]]*#/ { next }
    /^URIs:[[:space:]]*/ {
      in_uris = 1
      emit(substr($0, index($0, ":") + 1))
      next
    }
    /^[A-Za-z0-9-]+:/ { in_uris = 0; next }
    in_uris && /^[[:space:]]/ {
      emit($0)
      next
    }
    /^[[:space:]]*deb(-src)?[[:space:]]/ {
      uri = ""
      if ($2 ~ /^\[/) {
        for (i = 2; i <= NF; i++) {
          if ($i ~ /\]/) { uri = $(i + 1); break }
        }
      } else {
        uri = $2
      }
      if (uri != "") print uri
    }
  ' "$1"
}

# True when the URI is Ubuntu's own archive, security or ports host (country/cloud archive hosts included), the
# mirror+file indirection, or a host the runner image wrote into its apt-mirrors.txt.
apt_uri_allowed() {
  local uri="$1" host mirrors_file mirror_uri mirror_host
  case "$uri" in
    mirror+file:/etc/apt/apt-mirrors.txt)
      return 0
      ;;
  esac
  host="$(apt_uri_host "$uri")"
  case "$host" in
    archive.ubuntu.com|security.ubuntu.com|ports.ubuntu.com|*.archive.ubuntu.com|*.ports.ubuntu.com)
      return 0
      ;;
  esac
  mirrors_file="${APT_MIRRORS_FILE:-/etc/apt/apt-mirrors.txt}"
  if [ -r "$mirrors_file" ]; then
    while read -r mirror_uri _; do
      case "$mirror_uri" in
        ''|\#*) continue ;;
      esac
      mirror_host="$(apt_uri_host "$mirror_uri")"
      if [ -n "$host" ] && [ "$host" = "$mirror_host" ]; then
        return 0
      fi
    done < "$mirrors_file"
  fi
  return 1
}

# Move every source list in directory $1 that names a third-party URI out of apt's sight. $2 is the dry-run flag:
# anything but 0 only prints what would move. Files with no source URIs are left alone; a file with even one
# third-party URI is set aside whole (apt reads files, not individual lines of deb822 stanzas).
set_aside_third_party_source_lists() {
  local directory="${1:?source-list directory required}" dry_run="${2:-0}"
  local file uri bad_uris backup base dest n
  [ -d "$directory" ] || return 0
  while IFS= read -r file; do
    bad_uris=""
    while IFS= read -r uri; do
      if ! apt_uri_allowed "$uri"; then
        bad_uris="${bad_uris}${bad_uris:+ }$uri"
      fi
    done < <(apt_source_file_uris "$file")
    [ -n "$bad_uris" ] || continue

    if [ "$dry_run" != "0" ]; then
      echo "would set aside third-party apt source list: $file ($bad_uris)"
      continue
    fi
    backup="${APT_SOURCE_SET_ASIDE_DIR:-/var/tmp/buwiz-apt-sources-disabled}"
    mkdir -p "$backup"
    base="${file##*/}"
    dest="$backup/$base"
    n=1
    while [ -e "$dest" ] || [ -L "$dest" ]; do
      dest="$backup/$base.$n"
      n=$((n + 1))
    done
    mv "$file" "$dest"
    echo "set aside third-party apt source list: $file -> $dest ($bad_uris)"
  done < <(find "$directory" -mindepth 1 -maxdepth 1 \( -type f -o -type l \) -print | LC_ALL=C sort)
}

apt_update_with_retries() {
  local attempt pause
  pause="${APT_UPDATE_RETRY_PAUSE:-5}"
  for attempt in 1 2 3; do
    if apt-get update; then
      return 0
    fi
    if [ "$attempt" -lt 3 ]; then
      echo "apt-get update failed (attempt $attempt of 3); retrying in ${pause}s" >&2
      sleep "$pause"
    fi
  done
  echo "apt-get update failed after 3 attempts; the remaining sources are Ubuntu's" >&2
  return 1
}

apt_update_main() {
  set_aside_third_party_source_lists /etc/apt/sources.list.d 0
  apt_update_with_retries
}

if [[ "${BASH_SOURCE[0]}" == "$0" ]]; then
  apt_update_main "$@"
fi
