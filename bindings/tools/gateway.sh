#!/usr/bin/env bash
# The one reader of a gateway file in shell (docs/loom-run-settings.md, "[gateway]"); the Rust reader is
# sdc_tools::gateway::read. Sourced by the adapters, never run.
#
# The file is two lines, `base_url=<address>` and `token=<token>`, written by `runbook run` into its private
# directory. `sdc_gateway_read <file>` sets the shell variables SDC_GATEWAY_BASE_URL and SDC_GATEWAY_TOKEN
# (not exported: the caller hands them to the one process that needs them) and returns 0, or says that the
# file cannot be read or is no gateway file and returns 1. It prints neither the path nor a part of the content.
sdc_gateway_read() {
  local file=${1:-} line base='' token='' n=0
  SDC_GATEWAY_BASE_URL=''
  SDC_GATEWAY_TOKEN=''
  if [[ -z $file || ! -f $file || ! -r $file ]]; then
    echo "gateway.sh: the gateway file cannot be read" >&2
    return 1
  fi
  while IFS= read -r line || [[ -n $line ]]; do
    n=$((n + 1))
    case $n in
      1) [[ $line == base_url=?* ]] && base=${line#base_url=} ;;
      2) [[ $line == token=?* ]] && token=${line#token=} ;;
      *) n=99 ;;
    esac
  done < "$file"
  if [[ $n -ne 2 || -z $base || -z $token ]] || [[ $base != http://?* && $base != https://?* ]] || [[ $base == *[[:space:][:cntrl:]]* || $token == *[[:space:][:cntrl:]]* ]]; then
    echo "gateway.sh: the gateway file is not a gateway file" >&2
    return 1
  fi
  SDC_GATEWAY_BASE_URL=$base
  SDC_GATEWAY_TOKEN=$token
}
