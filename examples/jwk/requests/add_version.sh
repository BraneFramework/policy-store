#!/bin/bash

curl -v localhost:8080/v2/policies -X POST -H "Authorization: Bearer $(cat "$(dirname $0)/../token.txt")" -H 'Content-Type: application/json' -d '{ "metadata": { "name": "foo", "description": "Hello, world!", "language": "boolean-v1" }, "contents": true }'
