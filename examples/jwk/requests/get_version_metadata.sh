#!/bin/bash

curl -v localhost:8080/v2/policies/1 -H "Authorization: Bearer $(cat "$(dirname $0)/../token.txt")"
