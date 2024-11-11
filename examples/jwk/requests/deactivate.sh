#!/bin/bash

curl -v localhost:8080/v2/policies/active -X DELETE -H "Authorization: Bearer $(cat "$(dirname $0)/../token.txt")"
