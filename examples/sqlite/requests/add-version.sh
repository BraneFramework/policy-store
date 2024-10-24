#!/bin/bash

curl -v localhost:8080/policies -X POST -H 'Content-Type: application/json' -d '{ "metadata": { "name": "foo", "description": "Hello, world!" }, "contents": true }'
