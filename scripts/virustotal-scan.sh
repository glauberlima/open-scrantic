#!/usr/bin/env bash
set -euo pipefail

# Usage: VT_API_KEY=<key> virustotal-scan.sh <binary>...
# Output: one line per binary: name|malicious|total|sha256|gui_url
# Detections are reported, not treated as failures (unsigned binaries draw false positives).

scan_binary() {
  local path="$1"
  local name
  name=$(basename "$path")

  echo "Uploading $name to VirusTotal..." >&2
  local body http_code
  body=$(curl -sS \
    --request POST \
    --url "https://www.virustotal.com/api/v3/files" \
    --header "x-apikey: ${VT_API_KEY}" \
    --form "file=@${path}" \
    --write-out "\n%{http_code}")
  http_code=$(printf '%s' "$body" | tail -1)
  body=$(printf '%s' "$body" | head -n -1)
  if [ "$http_code" -lt 200 ] || [ "$http_code" -ge 300 ]; then
    echo "Upload failed for $name (HTTP $http_code): $body" >&2
    exit 1
  fi

  local analysis_id
  analysis_id=$(echo "$body" | jq -r '.data.id')
  if [ -z "$analysis_id" ] || [ "$analysis_id" = "null" ]; then
    echo "Failed to extract analysis_id for $name. Response: $body" >&2
    exit 1
  fi

  echo "Polling analysis $analysis_id for $name..." >&2
  sleep 60
  local attempt=0
  local max_attempts=20
  while [ $attempt -lt $max_attempts ]; do
    attempt=$((attempt + 1))
    echo "  Poll attempt $attempt/$max_attempts..." >&2

    local analysis_response status
    analysis_response=$(curl -fsS \
      --request GET \
      --url "https://www.virustotal.com/api/v3/analyses/${analysis_id}" \
      --header "x-apikey: ${VT_API_KEY}")

    status=$(echo "$analysis_response" | jq -r '.data.attributes.status')
    if [ "$status" = "completed" ]; then
      local malicious total sha256
      malicious=$(echo "$analysis_response" | jq -r '.data.attributes.stats.malicious')
      # total = sum of all stat categories
      total=$(echo "$analysis_response" | jq '[.data.attributes.stats | to_entries[].value] | add')
      sha256=$(echo "$analysis_response" | jq -r '.meta.file_info.sha256')
      echo "${name}|${malicious}|${total}|${sha256}|https://www.virustotal.com/gui/file/${sha256}"
      echo "Done: $name — $malicious/$total engines flagged" >&2
      return
    fi

    echo "  Status: $status" >&2
    sleep 15
  done

  echo "Scan timeout for $name after 6 minutes" >&2
  exit 1
}

if [ $# -lt 1 ]; then
  echo "Usage: $0 <binary>..." >&2
  exit 1
fi

for binary in "$@"; do
  scan_binary "$binary"
done
