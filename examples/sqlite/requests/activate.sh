#!/bin/bash

curl -v localhost:8080/v2/policies/active -X PUT -H 'Content-Type: application/json' -d '1'
