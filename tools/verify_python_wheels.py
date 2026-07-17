#!/usr/bin/env python3

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

"""Verify native wheels and their artifact-exact legal metadata."""

from __future__ import annotations

import argparse
import email
import subprocess
import sys
from pathlib import Path
from zipfile import ZipFile


NATIVE_LIBRARY = {
    "x86_64-unknown-linux-gnu": "paimon_ftindex/libpaimon_ftindex_ffi.so",
    "aarch64-unknown-linux-gnu": "paimon_ftindex/libpaimon_ftindex_ffi.so",
    "aarch64-apple-darwin": "paimon_ftindex/libpaimon_ftindex_ffi.dylib",
    "x86_64-pc-windows-msvc": "paimon_ftindex/paimon_ftindex_ffi.dll",
}

NESTED_LICENSE_MARKERS = (
    "For Zstandard software",
    "Dr Martin Porter",
    "UNICODE, INC. LICENSE AGREEMENT",
)


def repository_root() -> Path:
    return Path(
        subprocess.check_output(
            ["git", "rev-parse", "--show-toplevel"], text=True
        ).strip()
    )


def target_from_wheel_name(name: str) -> str:
    normalized = name.lower()
    if "win_amd64" in normalized:
        return "x86_64-pc-windows-msvc"
    if "macosx" in normalized and ("arm64" in normalized or "aarch64" in normalized):
        return "aarch64-apple-darwin"
    if "linux" in normalized and "aarch64" in normalized:
        return "aarch64-unknown-linux-gnu"
    if "linux" in normalized and ("x86_64" in normalized or "amd64" in normalized):
        return "x86_64-unknown-linux-gnu"
    raise ValueError(f"unsupported wheel platform tag: {name}")


def require_equal(actual: bytes, expected_path: Path, archive_path: str) -> None:
    expected = expected_path.read_bytes()
    if actual != expected:
        raise ValueError(
            f"{archive_path} does not match {expected_path.as_posix()}"
        )


def verify_wheel(wheel: Path, root: Path) -> None:
    target = target_from_wheel_name(wheel.name)
    legal_source = root / "python/licenses" / target
    expected_license_files = [
        f"licenses/{target}/LICENSE",
        f"licenses/{target}/NOTICE",
        f"licenses/{target}/THIRD-PARTY-LICENSES.html",
    ]

    with ZipFile(wheel) as archive:
        names = set(archive.namelist())
        metadata_paths = sorted(
            name for name in names if name.endswith(".dist-info/METADATA")
        )
        if len(metadata_paths) != 1:
            raise ValueError(
                f"expected one .dist-info/METADATA, found {metadata_paths}"
            )
        dist_info = metadata_paths[0].rsplit("/", 1)[0]

        package_legal = {
            "paimon_ftindex/LICENSE": legal_source / "LICENSE",
            "paimon_ftindex/NOTICE": legal_source / "NOTICE",
            "paimon_ftindex/THIRD-PARTY-LICENSES.html": (
                legal_source / "THIRD-PARTY-LICENSES.html"
            ),
        }
        standard_legal = {
            f"{dist_info}/licenses/{relative}": legal_source / Path(relative).name
            for relative in expected_license_files
        }
        required = set(package_legal) | set(standard_legal)
        missing = sorted(required - names)
        if missing:
            raise ValueError(f"missing legal files: {missing}")
        if "paimon_ftindex/DEPENDENCIES.rust.tsv" in names:
            raise ValueError(
                "wheel contains the cross-target repository dependency inventory"
            )

        for archive_path, expected_path in {**package_legal, **standard_legal}.items():
            require_equal(archive.read(archive_path), expected_path, archive_path)

        native_entries = sorted(
            name
            for name in names
            if name.startswith("paimon_ftindex/")
            and name.endswith((".so", ".dylib", ".dll"))
        )
        if native_entries != [NATIVE_LIBRARY[target]]:
            raise ValueError(f"unexpected native libraries: {native_entries}")

        legal_target_prefix = f"{dist_info}/licenses/licenses/"
        packaged_targets = {
            name[len(legal_target_prefix) :].split("/", 1)[0]
            for name in names
            if name.startswith(legal_target_prefix)
        }
        if packaged_targets != {target}:
            raise ValueError(
                f"wheel legal metadata covers {sorted(packaged_targets)}, expected {target}"
            )

        metadata = email.message_from_bytes(archive.read(metadata_paths[0]))
        if metadata.get("Metadata-Version") != "2.4":
            raise ValueError(
                f"unexpected Metadata-Version: {metadata.get('Metadata-Version')}"
            )
        if metadata.get("License-Expression") != "Apache-2.0":
            raise ValueError(
                f"unexpected License-Expression: {metadata.get('License-Expression')}"
            )
        if metadata.get_all("License-File", []) != expected_license_files:
            raise ValueError(
                "unexpected License-File fields: "
                + repr(metadata.get_all("License-File", []))
            )

        license_text = archive.read("paimon_ftindex/LICENSE").decode("utf-8")
        if "THIRD-PARTY-LICENSES.html" not in license_text:
            raise ValueError("LICENSE does not point to the third-party report")

        report_text = archive.read(
            "paimon_ftindex/THIRD-PARTY-LICENSES.html"
        ).decode("utf-8")
        if target not in report_text:
            raise ValueError(f"third-party report does not identify target {target}")
        for marker in NESTED_LICENSE_MARKERS:
            if marker not in report_text:
                raise ValueError(f"third-party report is missing {marker!r}")

        if target.endswith("linux-gnu"):
            if "linux-raw-sys" not in report_text or "windows_x86_64_" in report_text:
                raise ValueError("Linux report has an incorrect target dependency set")
        elif target == "aarch64-apple-darwin":
            if "linux-raw-sys" in report_text or "windows_x86_64_" in report_text:
                raise ValueError("macOS report has an incorrect target dependency set")
        else:
            if "windows_x86_64_msvc" not in report_text:
                raise ValueError("Windows report is missing the MSVC target package")
            if "windows_x86_64_gnu" in report_text or "linux-raw-sys" in report_text:
                raise ValueError("Windows report has an incorrect target dependency set")

        if any(name.startswith("tests/") for name in names):
            raise ValueError("wheel unexpectedly contains tests/")

    print(f"verified {wheel.name}: {target}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("wheels", nargs="+", type=Path)
    args = parser.parse_args()
    root = repository_root()

    failed = False
    for wheel in args.wheels:
        try:
            verify_wheel(wheel, root)
        except (KeyError, OSError, ValueError) as error:
            failed = True
            print(f"{wheel}: {error}", file=sys.stderr)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
