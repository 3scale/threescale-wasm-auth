#!/bin/bash

remove_headers() {
	local response="${1}"

	echo "${response}" | sed -E -e '0,/^[[:space:]]*$/d'
}

call_token_endpoint() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3}"
	local grant_type="${4}"
	local payload="${5}"

	curl -k -sSf -i -X POST \
		-H "Content-Type: application/x-www-form-urlencoded" \
		-d "grant_type=${grant_type}" \
		-d "client_id=${client_id}" \
		-d "${payload}" \
		"${url}/auth/realms/${realm}/protocol/openid-connect/token"
}

call_with_token() {
	local method="${1}"
	local url="${2}"
	local token="${3}"
	local payload="${4}"

	curl -k -sSf -i -X "${method}" \
		-H "Authorization: Bearer ${token}" \
		-H "Content-Type: application/json" \
		-d "${payload}" \
		"${url}"
}

call_idp() {
	local method="${1}"
	local url="${2}"
	local ctype="${3}"
	local payload="${4}"

	curl -k -sSf -i -c ./cookies -b ./cookies -X "${method}" \
		-H "Content-Type: ${ctype}" \
		-d "${payload}" \
		"${url}"
}

get_auth_form() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3:-test}"
	local scope="${4:-openid+profile+email}"

	local url="${url}/auth/realms/${realm}/protocol/openid-connect/auth?client_id=${client_id}&response_type=code&scope=${scope}&redirect_uri=http%3A%2F%2F0.0.0.0%3A8080%2Foidc"

	curl -k -sSf -i -c ./cookies -b ./cookies -X GET "${url}"
}

# returns the login form URL
parse_auth_form() {
	local body="${1}"

	echo "${body}" | sed -n -E -e 's/\&amp;/\&/g' -e 's/.*<form id="kc-form-login".* action="([^[:space:]]+)".*"[ >]/\1/p'
}

# Note: does not escape the header, so be careful
get_header() {
	local response="${1}"
	local header="${2}"

	echo "${response}" | grep -i "^${header}: .*$" | cut -d' ' -f 2-
}

# Note: does not escape the parameter, so be careful
get_query_string_parameter() {
	local url="${1}"
	local parameter="${2}"

	echo "${url}" | sed -n -E -e "s/^.*[^[:space:]]+[?&]${parameter}=([^[:space:]&]+).*$/\1/p"
}

# Generates JSON with an access token
call_auth_token_password() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3}"
	local user="${4}"
	local passwd="${5}"

	call_token_endpoint "${url}" "${realm}" "${client_id}" "password" "username=${user}&password=${passwd}"
}

# Generates a Location with the right credentials for JWT authn
call_auth_token_code() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3}"
	local client_secret="${4}"
	local code="${5}"
	local redirect_uri="${6:-"http%3A%2F%2F0.0.0.0%3A8080%2Foidc"}"

	call_token_endpoint "${url}" "${realm}" "${client_id}" "authorization_code" "code=${code}&client_secret=${client_secret}&redirect_uri=${redirect_uri}"
}

get_access_token_passwd() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3}"
	local user="${4}"
	local passwd="${5}"

	remove_headers "$(call_auth_token_password "${url}" "${realm}" "${client_id}" "${user}" "${passwd}")" | jq -r ".access_token"
}

get_access_token_code() {
	local url="${1}"
	local realm="${2}"
	local client_id="${3}"
	local client_secret="${4}"
	local login_code="${5}"

	remove_headers "$(call_auth_token_code "${url}" "${realm}" "${client_id}" "${client_secret}" "${login_code}")" | jq -r ".access_token"
}

add_client() {
	local url="${1}"
	local realm="${2}"
	local token="${3}"
	local id="${4}"
	local name="${5}"

	local payload="{ \"id\": \"${id}\", \"name\": \"${name}\", \"redirectUris\": [\"*\"] }"
	call_with_token POST "${url}/auth/admin/realms/${realm}/clients" "${token}" "${payload}"
}

get_client_secret() {
	local url="${1}"
	local realm="${2}"
	local token="${3}"
	local id="${4}"

	remove_headers "$(call_with_token GET "${url}/auth/admin/realms/${realm}/clients/${id}/client-secret" "${token}")" | jq -r ".value"
}

main() {
	local web="${1}"
	local url="${2}"
	local realm="${3}"
	local client_id="${4}"
	local user="${5:-${KEYCLOAK_USER:-admin}}"
	local passwd="${6:-${KEYCLOAK_PASSWORD:-admin}}"

	if test "x${web}" = "x"; then
		echo >&2 "No proxy URL specified, taking default http://ingress/oidc"
		web="http://ingress/oidc"
	fi
	if test "x${url}" = "x"; then
		echo >&2 "No Keycloak URL specified, taking default https://0.0.0.0:18443"
		url="https://0.0.0.0:18443"
	fi
	if test "x${realm}" = "x"; then
		echo >&2 "No realm specified, taking default master"
		realm="master"
	fi
	if test "x${client_id}" = "x"; then
		echo >&2 "No client id specified, taking default test"
		client_id="test"
	fi

	rm -f ./cookies
	echo "-> Retrieving token for admin-cli from ${url} with user ${user}:${passwd} ..."
	admin_token=$(get_access_token_passwd "${url}" "${realm}" "admin-cli" "${user}" "${passwd}")
	echo "<- Got admin token: ${admin_token}"

	sleep 2

	echo "-> Creating new client ${client_id} via admin token... (might fail if pre-existing)"
	add_client "${url}" "${realm}" "${admin_token}" "${client_id}" "${client_id}" || true
	echo "<- Done"

	sleep 1

	echo "-> Retrieving client secret for ${client_id}..."
	client_secret=$(get_client_secret "${url}" "${realm}" "${admin_token}" "${client_id}")
	echo "<- Got client secret: ${client_secret}"

	echo "=== Setup done ==="
	sleep 3

	echo "=== Init auth flow via browser by requesting code for ${client_id} via administrative user/passwd authentication against IDP ==="

	sleep 1

	echo "-> Simulating we access the login form"
	auth_form=$(get_auth_form "${url}" "${realm}" "${client_id}" "openid+profile+email")
	login_url=$(parse_auth_form "${auth_form}")
	echo "<- Got login URL: ${login_url}"
	sleep 2
	echo "-> Calling login endpoint ${login_url}"
	login_response=$(call_idp POST "${login_url}" application/x-www-form-urlencoded "username=${user}&password=${passwd}&credentialId=")
	location=$(get_header "${login_response}" "Location")
	echo "<- Location: ${location}"
	login_code=$(get_query_string_parameter "${location}" code)
	echo "<- Got login code: ${login_code}"

	rm -f ./cookies

	sleep 5

	echo "-> Using ${client_id} client authz code along client secret to obtain an access token"
	response=$(call_auth_token_code "${url}" "${realm}" "${client_id}" "${client_secret}" "${login_code}")
	response_no_headers=$(remove_headers "${response}")
	echo "<- Response:"
	echo "${response_no_headers}" | jq
	id_token=$(echo "${response_no_headers}" | jq -r ".id_token")
	expiry=$(echo "${response_no_headers}" | jq -r ".expires_in")
	echo "<- Got id token with expiry ${expiry}: ${id_token}"
	sleep 5
	echo "-> Calling proxy with id_token..."
	curl -k -v -H "Authorization: Bearer ${id_token}" "${web}"
}

if [[ "${BASH_SOURCE[0]}" = "${0}" ]]; then
	set -eo pipefail
	shopt -s failglob

	main "${@}"
fi
