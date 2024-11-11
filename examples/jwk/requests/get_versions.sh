#!/bin/bash

curl -v localhost:8080/v2/policies -X GET -H "Authorization: Bearer $(cat "$(dirname $0)/../token.txt")"
