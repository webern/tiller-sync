#!/usr/bin/env bash

DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )"
echo "The script directory is: $DIR"
IGNORE_DIR="${DIR}/../.ignore"
CLIENT_SECRET="${DIR}/client-secret.fake.json"
TILLER_HOME="${IGNORE_DIR}/test_home"
mkdir -p "${IGNORE_DIR}"
rm -rf "${TILLER_HOME}"
cp "${CLIENT_SECRET}" "${IGNORE_DIR}/client-secret.json"
echo "created ${IGNORE_DIR}"
echo "moved   ${IGNORE_DIR}/client-secret.json"
