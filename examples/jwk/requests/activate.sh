#!/bin/bash

curl -v localhost:8080/v2/policies/active -X PUT -H "Authorization: Bearer $(cat "$(dirname $0)/../token.txt")" -H 'Content-Type: application/json' -d '1'
