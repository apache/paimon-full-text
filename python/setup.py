#
# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.
#

"""Build helper: copies the pre-built native FFI library into the package."""

import os
import platform
import shutil

from setuptools import Distribution, setup
from setuptools.command.build_py import build_py
from wheel.bdist_wheel import bdist_wheel


def _project_root():
    return os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))


def _package_dir():
    return os.path.join(os.path.dirname(os.path.abspath(__file__)), "paimon_ftindex")


def _license_dir():
    return os.path.join(_project_root(), "licenses")


def _lib_name():
    system = platform.system()
    if system == "Darwin":
        return "libpaimon_ftindex_ffi.dylib"
    if system == "Windows":
        return "paimon_ftindex_ffi.dll"
    return "libpaimon_ftindex_ffi.so"


def _find_native_lib():
    here = os.path.dirname(os.path.abspath(__file__))
    lib = _lib_name()

    env_path = os.environ.get("PAIMON_FTINDEX_LIB_PATH")
    if env_path:
        if os.path.isfile(env_path):
            return env_path
        candidate = os.path.join(env_path, lib)
        if os.path.isfile(candidate):
            return candidate

    for profile in ["release", "debug"]:
        candidate = os.path.join(here, "..", "target", profile, lib)
        if os.path.isfile(candidate):
            return candidate

    return None


def _prepare_binary_metadata():
    binary_license = os.path.join(_package_dir(), "LICENSE")
    with open(binary_license, "wb") as output:
        for license_part in [
            os.path.join(_project_root(), "LICENSE"),
            os.path.join(_license_dir(), "BINARY-LICENSE-APPENDIX"),
        ]:
            with open(license_part, "rb") as input_:
                shutil.copyfileobj(input_, output)

    for metadata_file in ["NOTICE", "DEPENDENCIES.rust.tsv"]:
        shutil.copy2(
            os.path.join(_project_root(), metadata_file),
            os.path.join(_package_dir(), metadata_file),
        )
    shutil.copy2(
        os.path.join(_license_dir(), "python", "THIRD-PARTY-LICENSES.html"),
        os.path.join(_package_dir(), "THIRD-PARTY-LICENSES.html"),
    )


class BuildPyWithNativeLib(build_py):
    def run(self):
        src = _find_native_lib()
        if src:
            dst = os.path.join(_package_dir(), _lib_name())
            shutil.copy2(src, dst)
        _prepare_binary_metadata()
        super().run()


class PlatformWheel(bdist_wheel):
    """Tag wheel as py3-none-{platform} since this is a ctypes package."""

    def finalize_options(self):
        bdist_wheel.finalize_options(self)
        self.root_is_pure = False

    def get_tag(self):
        _, _, plat = bdist_wheel.get_tag(self)
        return "py3", "none", plat


class BinaryDistribution(Distribution):
    """Force the wheel to be platform-specific."""

    def has_ext_modules(self):
        return True


_prepare_binary_metadata()

setup(
    cmdclass={"build_py": BuildPyWithNativeLib, "bdist_wheel": PlatformWheel},
    distclass=BinaryDistribution,
    license_files=[
        "paimon_ftindex/LICENSE",
        "paimon_ftindex/NOTICE",
        "paimon_ftindex/THIRD-PARTY-LICENSES.html",
    ],
)
