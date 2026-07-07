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

# Deploy the Java release candidate artifacts to Apache Nexus staging from a
# committer/RM machine. The GitHub Actions release workflow only builds native
# libraries and packages a jar; it does not sign or deploy to Nexus.
#
# Usage:
#   cd tools
#   RELEASE_VERSION=0.1.0 RC_NUMBER=1 GITHUB_RUN_ID=<run-id> ./deploy_java_staging.sh
#
# Local dry run without Nexus deploy:
#   RELEASE_VERSION=0.1.0 RC_NUMBER=1 GITHUB_RUN_ID=<run-id> DRY_RUN=true ./deploy_java_staging.sh
#
# The script downloads native-* artifacts from the specified GitHub Actions run,
# copies them into java/src/main/resources/native, and runs mvn deploy locally.
# Instead of GITHUB_RUN_ID, set NATIVE_ARTIFACT_DIR to a directory containing:
#   native-linux-x86_64/libpaimon_ftindex_jni.so
#   native-linux-aarch64/libpaimon_ftindex_jni.so
#   native-macos-aarch64/libpaimon_ftindex_jni.dylib
#   native-windows-x86_64/paimon_ftindex_jni.dll
#
# Maven must be configured locally for the apache.releases.https server in
# ~/.m2/settings.xml. Alternatively set NEXUS_STAGE_DEPLOYER_USER and
# NEXUS_STAGE_DEPLOYER_PW and this script will create a temporary settings.xml.
# GPG signing uses the committer's local GPG setup.

set -o errexit
set -o nounset
set -o pipefail

MVN=${MVN:-mvn}
REPO=${REPO:-apache/paimon-full-text}
SKIP_TESTS=${SKIP_TESTS:-true}
CLEANUP_NATIVE_RESOURCES=${CLEANUP_NATIVE_RESOURCES:-true}
DRY_RUN=${DRY_RUN:-false}

CURR_DIR=$(pwd)
if [[ $(basename "$CURR_DIR") != "tools" ]]; then
  echo "You have to call the script from the tools/ dir" >&2
  exit 1
fi

REPO_DIR=$(cd .. && pwd)
RELEASE_VERSION=${RELEASE_VERSION:-}
RC_NUMBER=${RC_NUMBER:-}
TAG=${TAG:-}
GITHUB_RUN_ID=${GITHUB_RUN_ID:-}
NATIVE_ARTIFACT_DIR=${NATIVE_ARTIFACT_DIR:-}
STAGING_DESCRIPTION=${STAGING_DESCRIPTION:-}
MAVEN_SETTINGS=${MAVEN_SETTINGS:-}

if [[ -z "$RELEASE_VERSION" ]]; then
  echo "RELEASE_VERSION is unset" >&2
  exit 1
fi

if [[ -z "$TAG" ]]; then
  if [[ -z "$RC_NUMBER" ]]; then
    echo "RC_NUMBER is unset" >&2
    exit 1
  fi
  TAG="v${RELEASE_VERSION}-rc${RC_NUMBER}"
fi

if [[ -z "$STAGING_DESCRIPTION" ]]; then
  STAGING_DESCRIPTION="Apache Paimon Full Text, version ${RELEASE_VERSION}, release candidate ${TAG#*-rc}"
fi

if [[ -z "$NATIVE_ARTIFACT_DIR" ]]; then
  NATIVE_ARTIFACT_DIR="$CURR_DIR/release/java-native-${TAG}"
fi

POM_VERSION=$(
  sed -n 's#.*<version>\([^<]*\)</version>.*#\1#p' "$REPO_DIR/java/pom.xml" |
    sed -n '2p'
)
if [[ "$POM_VERSION" != "$RELEASE_VERSION" ]]; then
  echo "java/pom.xml version is $POM_VERSION, expected $RELEASE_VERSION" >&2
  echo "Run this script from the checked-out RC tag after bumping versions." >&2
  exit 1
fi

if ! git -C "$REPO_DIR" rev-parse -q --verify "$TAG^{commit}" >/dev/null; then
  if [[ "${ALLOW_MISSING_TAG:-false}" != "true" ]]; then
    echo "Tag $TAG does not exist locally." >&2
    echo "Run: git fetch --tags && git checkout $TAG" >&2
    echo "Or set ALLOW_MISSING_TAG=true if this is intentional." >&2
    exit 1
  fi
else
  TAG_COMMIT=$(git -C "$REPO_DIR" rev-parse "$TAG^{commit}")
  HEAD_COMMIT=$(git -C "$REPO_DIR" rev-parse HEAD)
  if [[ "$TAG_COMMIT" != "$HEAD_COMMIT" && "${ALLOW_DIFFERENT_HEAD:-false}" != "true" ]]; then
    echo "Current HEAD is not $TAG." >&2
    echo "Run: git checkout $TAG" >&2
    echo "Or set ALLOW_DIFFERENT_HEAD=true if this is intentional." >&2
    exit 1
  fi
fi

if [[ -n "$MAVEN_SETTINGS" && ! -f "$MAVEN_SETTINGS" ]]; then
  echo "MAVEN_SETTINGS does not exist: $MAVEN_SETTINGS" >&2
  exit 1
fi

download_native_artifacts() {
  if [[ -z "$GITHUB_RUN_ID" ]]; then
    return
  fi

  if ! command -v gh >/dev/null 2>&1; then
    echo "gh CLI is required when GITHUB_RUN_ID is set" >&2
    exit 1
  fi

  rm -rf "$NATIVE_ARTIFACT_DIR"
  mkdir -p "$NATIVE_ARTIFACT_DIR"

  for artifact in \
    native-linux-x86_64 \
    native-linux-aarch64 \
    native-macos-aarch64 \
    native-windows-x86_64
  do
    gh run download "$GITHUB_RUN_ID" \
      --repo "$REPO" \
      --name "$artifact" \
      --dir "$NATIVE_ARTIFACT_DIR/$artifact"
  done
}

copy_native() {
  local artifact_dir=$1
  local file_name=$2
  local target_dir=$3
  local source_file="$NATIVE_ARTIFACT_DIR/$artifact_dir/$file_name"

  if [[ ! -f "$source_file" ]]; then
    echo "Missing native artifact: $source_file" >&2
    exit 1
  fi

  mkdir -p "$target_dir"
  cp "$source_file" "$target_dir/$file_name"
}

cleanup_native_resources() {
  if [[ "$CLEANUP_NATIVE_RESOURCES" == "true" ]]; then
    rm -rf "$REPO_DIR/java/src/main/resources/native"
  fi
}
trap cleanup_native_resources EXIT

download_native_artifacts

rm -rf "$REPO_DIR/java/src/main/resources/native"
copy_native native-linux-x86_64 libpaimon_ftindex_jni.so \
  "$REPO_DIR/java/src/main/resources/native/linux/x86_64"
copy_native native-linux-aarch64 libpaimon_ftindex_jni.so \
  "$REPO_DIR/java/src/main/resources/native/linux/aarch64"
copy_native native-macos-aarch64 libpaimon_ftindex_jni.dylib \
  "$REPO_DIR/java/src/main/resources/native/macos/aarch64"
copy_native native-windows-x86_64 paimon_ftindex_jni.dll \
  "$REPO_DIR/java/src/main/resources/native/windows/x86_64"

echo "Native libraries staged for Java package:"
find "$REPO_DIR/java/src/main/resources/native" -type f | sort

TEMP_SETTINGS=
if [[ "$DRY_RUN" != "true" &&
      -z "$MAVEN_SETTINGS" &&
      ( -n "${NEXUS_STAGE_DEPLOYER_USER:-}" || -n "${NEXUS_STAGE_DEPLOYER_PW:-}" ) ]]; then
  if [[ -z "${NEXUS_STAGE_DEPLOYER_USER:-}" || -z "${NEXUS_STAGE_DEPLOYER_PW:-}" ]]; then
    echo "Both NEXUS_STAGE_DEPLOYER_USER and NEXUS_STAGE_DEPLOYER_PW are required" >&2
    exit 1
  fi

  TEMP_SETTINGS=$(mktemp)
  cat > "$TEMP_SETTINGS" <<EOF
<settings>
  <servers>
    <server>
      <id>apache.releases.https</id>
      <username>${NEXUS_STAGE_DEPLOYER_USER}</username>
      <password>${NEXUS_STAGE_DEPLOYER_PW}</password>
    </server>
  </servers>
</settings>
EOF
  MAVEN_SETTINGS="$TEMP_SETTINGS"
fi

cleanup_temp_settings() {
  if [[ -n "$TEMP_SETTINGS" ]]; then
    rm -f "$TEMP_SETTINGS"
  fi
}
trap 'cleanup_native_resources; cleanup_temp_settings' EXIT

MVN_CMD=("$MVN")
if [[ -n "$MAVEN_SETTINGS" ]]; then
  MVN_CMD+=("-s" "$MAVEN_SETTINGS")
fi

if [[ "$DRY_RUN" == "true" ]]; then
  MVN_CMD+=(clean verify -Prelease -Dgpg.skip=true)
else
  MVN_CMD+=(clean deploy -Prelease "-DstagingDescription=$STAGING_DESCRIPTION")
fi
if [[ "$SKIP_TESTS" == "true" ]]; then
  MVN_CMD+=(-DskipTests)
fi

if [[ "$DRY_RUN" == "true" ]]; then
  echo "Dry-running Java staging build. No artifacts will be deployed to Nexus."
else
  echo "Deploying Java artifacts to Apache Nexus staging."
  echo "Staging description: $STAGING_DESCRIPTION"
fi
(
  cd "$REPO_DIR/java"
  "${MVN_CMD[@]}"
)

JAR_FILE="$REPO_DIR/java/target/paimon-full-text-index-${RELEASE_VERSION}.jar"
SOURCES_JAR="$REPO_DIR/java/target/paimon-full-text-index-${RELEASE_VERSION}-sources.jar"
JAVADOC_JAR="$REPO_DIR/java/target/paimon-full-text-index-${RELEASE_VERSION}-javadoc.jar"
for artifact in "$JAR_FILE" "$SOURCES_JAR" "$JAVADOC_JAR"; do
  if [[ ! -f "$artifact" ]]; then
    echo "Expected Maven artifact is missing: $artifact" >&2
    exit 1
  fi
done

if command -v jar >/dev/null 2>&1; then
  for native_entry in \
    native/linux/x86_64/libpaimon_ftindex_jni.so \
    native/linux/aarch64/libpaimon_ftindex_jni.so \
    native/macos/aarch64/libpaimon_ftindex_jni.dylib \
    native/windows/x86_64/paimon_ftindex_jni.dll
  do
    if ! jar tf "$JAR_FILE" | grep -qx "$native_entry"; then
      echo "Packaged jar is missing native entry: $native_entry" >&2
      exit 1
    fi
  done
fi

echo ""
if [[ "$DRY_RUN" == "true" ]]; then
  echo "Java staging dry run finished successfully."
else
  echo "Java staging deploy finished."
  echo "Check the Maven output for the orgapachepaimon-XXXX staging repository id."
fi
