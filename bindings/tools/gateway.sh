#!/usr/bin/env bash
# The one reader of a gateway file in shell (docs/loom-run-settings.md, "[gateway]"); the Rust reader is
# sdc_tools::gateway::read. Sourced by the adapters, never run.
#
# The file is two lines, `base_url=<address>` and `token=<token>`, or the first line alone for a gateway without
# ingress auth, written by `runbook run` into its private directory. `sdc_gateway_read <file>` sets the shell
# variables SDC_GATEWAY_BASE_URL and SDC_GATEWAY_TOKEN (empty when the file names no token; not exported: the
# caller hands them to the one process that needs them) and returns 0, or says that the file cannot be read or
# is no gateway file and returns 1. It prints neither the path nor a part of the content.
# It takes the files the Rust reader takes and no other: the same checks of the address and the token, in the
# C locale so that a length is in bytes and a character is a byte.
sdc_gateway_address_ok() {
  local url=$1 rest authority host port inside after
  [[ -n $url && ${#url} -le 2048 && $url != *[^[:graph:]]* ]] || return 1
  case $url in
    http://*) rest=${url#http://} ;;
    https://*) rest=${url#https://} ;;
    *) return 1 ;;
  esac
  authority=${rest%%/*}
  [[ $authority != *@* && $url != *[?#]* ]] || return 1
  if [[ $authority == \[* ]]; then
    [[ $authority == *]* ]] || return 1
    inside=${authority%%]*}
    after=${authority#*]}
    [[ ${#inside} -gt 1 ]] || return 1
    [[ -z $after || ( $after == :* && ${after#:} != *[^0-9]* ) ]] || return 1
  else
    [[ $authority != *:*:* ]] || return 1
    host=${authority%%:*}
    if [[ $authority == *:* ]]; then
      port=${authority#*:}
      [[ $port != *[^0-9]* ]] || return 1
    fi
    [[ -n $host ]] || return 1
  fi
}

sdc_gateway_read() {
  local LC_ALL=C file=${1:-} line base='' token='' n=0 second=''
  SDC_GATEWAY_BASE_URL=''
  SDC_GATEWAY_TOKEN=''
  if [[ -z $file || ! -f $file || ! -r $file ]]; then
    echo "gateway.sh: the gateway file cannot be read" >&2
    return 1
  fi
  # A NUL is dropped by `read`: the file is no gateway file when removing the NULs changes it.
  tr -d '\000' < "$file" | cmp -s - "$file" || { echo "gateway.sh: the gateway file is not a gateway file" >&2; return 1; }
  while IFS= read -r line || [[ -n $line ]]; do
    n=$((n + 1))
    case $n in
      1) [[ $line == base_url=?* ]] && base=${line#base_url=} ;;
      2) second=1; [[ $line == token=?* ]] && token=${line#token=} ;;
      *) n=99 ;;
    esac
  done < "$file"
  # The address line alone (n is 1) is a gateway without a token; a second line must be a good token line.
  if [[ -z $base || ( $n -ne 1 && $n -ne 2 ) || ( -n $second && -z $token ) ]] || ! sdc_gateway_address_ok "$base" ||
     [[ -n $token && ( $token == *[^[:graph:]]* || ${#token} -lt 8 || ${#token} -gt 8192 ) ]]; then
    echo "gateway.sh: the gateway file is not a gateway file" >&2
    return 1
  fi
  SDC_GATEWAY_BASE_URL=$base
  SDC_GATEWAY_TOKEN=$token
}
