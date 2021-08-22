#!/bin/sh

BASE64="${BASE64:-YWxhZGRpbjpvcGVuc2VzYW1l}"
curl -vvv -H "Authorization: Basic ${BASE64}" "http://ingress/lala"
