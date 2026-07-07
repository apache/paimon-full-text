#!/usr/bin/env bash

#
# Licensed to the Apache Software Foundation (ASF) under one or more
# contributor license agreements.  See the NOTICE file distributed with
# this work for additional information regarding copyright ownership.
# The ASF licenses this file to You under the Apache License, Version 2.0
# (the "License"); you may not use this file except in compliance with
# the License.  You may obtain a copy of the License at
#
#    http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

# Local robustness checks for deploy_java_staging.sh. This script does not
# contact GitHub or Nexus. It uses synthetic native artifacts and DRY_RUN=true
# for the positive path.

set -o errexit
set -o nounset
set -o pipefail

CURR_DIR=$(pwd)
if [[ $(basename "$CURR_DIR") != "tools" ]]; then
  echo "You have to call the script from the tools/ dir" >&2
  exit 1
fi

REPO_DIR=$(cd .. && pwd)
SCRIPT="$CURR_DIR/deploy_java_staging.sh"
RELEASE_VERSION=$(
  sed -n 's#.*<version>\([^<]*\)</version>.*#\1#p' "$REPO_DIR/java/pom.xml" |
    sed -n '2p'
)
TMP_DIR=$(mktemp -d)

cleanup() {
  rm -rf "$TMP_DIR"
  rm -rf "$REPO_DIR/java/src/main/resources/native"
}
trap cleanup EXIT

make_native_artifacts() {
  local dir=$1
  rm -rf "$dir"
  mkdir -p \
    "$dir/native-linux-x86_64" \
    "$dir/native-linux-aarch64" \
    "$dir/native-macos-aarch64" \
    "$dir/native-windows-x86_64"
  printf 'test native linux x86_64\n' > "$dir/native-linux-x86_64/libpaimon_ftindex_jni.so"
  printf 'test native linux aarch64\n' > "$dir/native-linux-aarch64/libpaimon_ftindex_jni.so"
  printf 'test native macos aarch64\n' > "$dir/native-macos-aarch64/libpaimon_ftindex_jni.dylib"
  printf 'test native windows x86_64\n' > "$dir/native-windows-x86_64/paimon_ftindex_jni.dll"
}

run_expect_failure() {
  local name=$1
  local expected=$2
  shift 2

  local output
  set +o errexit
  output=$("$@" 2>&1)
  local status=$?
  set -o errexit

  if [[ $status -eq 0 ]]; then
    echo "FAIL: $name unexpectedly succeeded" >&2
    exit 1
  fi
  if ! grep -Fq "$expected" <<<"$output"; then
    echo "FAIL: $name did not contain expected text: $expected" >&2
    echo "$output" >&2
    exit 1
  fi
  echo "PASS: $name"
}

echo "Testing deploy_java_staging.sh with Java version $RELEASE_VERSION"

NATIVE_DIR="$TMP_DIR/native"
make_native_artifacts "$NATIVE_DIR"
(
  RELEASE_VERSION="$RELEASE_VERSION" \
    RC_NUMBER=1 \
    ALLOW_MISSING_TAG=true \
    NATIVE_ARTIFACT_DIR="$NATIVE_DIR" \
    DRY_RUN=true \
    "$SCRIPT"
)
if [[ -d "$REPO_DIR/java/src/main/resources/native" ]]; then
  echo "FAIL: native resources were not cleaned up after successful dry run" >&2
  exit 1
fi
echo "PASS: dry-run staging build"

run_expect_failure "wrong working directory" "tools/ dir" \
  bash -c 'cd "$1" && env RELEASE_VERSION="$2" RC_NUMBER=1 tools/deploy_java_staging.sh' \
    _ "$REPO_DIR" "$RELEASE_VERSION"

run_expect_failure "missing RELEASE_VERSION" "RELEASE_VERSION is unset" \
  "$SCRIPT"

run_expect_failure "missing RC_NUMBER or TAG" "RC_NUMBER is unset" \
  env RELEASE_VERSION="$RELEASE_VERSION" "$SCRIPT"

run_expect_failure "version mismatch" "java/pom.xml version is $RELEASE_VERSION, expected 9.9.9" \
  env RELEASE_VERSION=9.9.9 RC_NUMBER=1 ALLOW_MISSING_TAG=true "$SCRIPT"

MISSING_NATIVE_DIR="$TMP_DIR/missing-native"
make_native_artifacts "$MISSING_NATIVE_DIR"
rm "$MISSING_NATIVE_DIR/native-windows-x86_64/paimon_ftindex_jni.dll"
run_expect_failure "missing native artifact" "Missing native artifact" \
  env RELEASE_VERSION="$RELEASE_VERSION" RC_NUMBER=1 ALLOW_MISSING_TAG=true \
    NATIVE_ARTIFACT_DIR="$MISSING_NATIVE_DIR" DRY_RUN=true "$SCRIPT"

run_expect_failure "missing Maven settings file" "MAVEN_SETTINGS does not exist" \
  env RELEASE_VERSION="$RELEASE_VERSION" RC_NUMBER=1 ALLOW_MISSING_TAG=true \
    MAVEN_SETTINGS="$TMP_DIR/does-not-exist.xml" "$SCRIPT"

make_native_artifacts "$NATIVE_DIR"
run_expect_failure "partial Nexus credentials" "Both NEXUS_STAGE_DEPLOYER_USER and NEXUS_STAGE_DEPLOYER_PW are required" \
  env RELEASE_VERSION="$RELEASE_VERSION" RC_NUMBER=1 ALLOW_MISSING_TAG=true \
    NATIVE_ARTIFACT_DIR="$NATIVE_DIR" NEXUS_STAGE_DEPLOYER_USER=user "$SCRIPT"

echo "All deploy_java_staging.sh robustness checks passed."
