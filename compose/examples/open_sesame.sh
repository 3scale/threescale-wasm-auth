#!/bin/sh

PLAINTEXT="${1:-"aladdin:opensesame"}"

base64_urlencode()
{
  echo -n "${1}" | base64 | tr '/+' '_-' | tr -d '='
}

main()
{
  local plaintext="${1:-"aladdin:opensesame"}"
  local proxy_url="${2:-"http://ingress/somepath"}"
  local encoded="${BASE64}"

  if test "x${BASE64}" = "x"; then
    encoded=$(base64_urlencode "${plaintext}")
    echo "Base64url'ed: ${encoded}"
  fi

  curl -vvv -H "Authorization: Basic ${encoded}" "${proxy_url}"
}

main "${@}"
