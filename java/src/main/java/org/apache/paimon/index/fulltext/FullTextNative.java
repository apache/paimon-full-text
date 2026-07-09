/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

package org.apache.paimon.index.fulltext;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.nio.file.StandardCopyOption;

final class FullTextNative {

    private static final String LIB_NAME = "paimon_ftindex_jni";

    static {
        loadNativeLibrary();
    }

    private FullTextNative() {}

    private static void loadNativeLibrary() {
        String libPath = System.getenv("PAIMON_FTINDEX_JNI_LIB_PATH");
        if (libPath != null && !libPath.isEmpty()) {
            System.load(libPath);
            return;
        }

        UnsatisfiedLinkError loadLibraryError = null;
        try {
            System.loadLibrary(LIB_NAME);
            return;
        } catch (UnsatisfiedLinkError e) {
            loadLibraryError = e;
        }

        String os = normalizeOs(System.getProperty("os.name", ""));
        String arch = normalizeArch(System.getProperty("os.arch", ""));
        String libFileName = mapLibraryName(os);
        String resourcePath = "/native/" + os + "/" + arch + "/" + libFileName;

        try (InputStream in = FullTextNative.class.getResourceAsStream(resourcePath)) {
            if (in == null) {
                UnsatisfiedLinkError error =
                        new UnsatisfiedLinkError("Native library not found in JAR: " + resourcePath);
                error.addSuppressed(loadLibraryError);
                throw error;
            }
            File tempFile = File.createTempFile("paimon_ftindex_jni", libFileName);
            tempFile.deleteOnExit();
            Files.copy(in, tempFile.toPath(), StandardCopyOption.REPLACE_EXISTING);
            System.load(tempFile.getAbsolutePath());
        } catch (IOException e) {
            UnsatisfiedLinkError error =
                    new UnsatisfiedLinkError(
                            "Failed to extract native library " + resourcePath + ": " + e.getMessage());
            error.addSuppressed(loadLibraryError);
            throw error;
        }
    }

    private static String normalizeOs(String osName) {
        String lower = osName.toLowerCase();
        if (lower.contains("linux")) {
            return "linux";
        } else if (lower.contains("mac") || lower.contains("darwin")) {
            return "macos";
        } else if (lower.contains("win")) {
            return "windows";
        }
        throw new UnsatisfiedLinkError("Unsupported OS: " + osName);
    }

    private static String normalizeArch(String archName) {
        String lower = archName.toLowerCase();
        if (lower.equals("amd64") || lower.equals("x86_64")) {
            return "x86_64";
        } else if (lower.equals("aarch64") || lower.equals("arm64")) {
            return "aarch64";
        }
        throw new UnsatisfiedLinkError("Unsupported architecture: " + archName);
    }

    private static String mapLibraryName(String os) {
        switch (os) {
            case "linux":
                return "libpaimon_ftindex_jni.so";
            case "macos":
                return "libpaimon_ftindex_jni.dylib";
            case "windows":
                return "paimon_ftindex_jni.dll";
            default:
                throw new UnsatisfiedLinkError("Unsupported OS: " + os);
        }
    }

    static native long createWriter(String[] keys, String[] values);

    static native void addDocument(long writerPtr, long rowId, String text);

    static native void addDocumentFields(
            long writerPtr, long rowId, String[] fieldNames, String[] texts);

    static native void writeIndex(long writerPtr, FullTextIndexOutput output);

    static native void freeWriter(long writerPtr);

    static native long openReader(FullTextIndexInput input);

    static native FullTextSearchResult search(long readerPtr, String query, int limit);

    static native FullTextSearchResult searchWithRoaringFilter(
            long readerPtr, String query, int limit, byte[] roaringFilter);

    static native void prewarm(long readerPtr);

    static native FullTextReadMetrics readMetrics(long readerPtr);

    static native void freeReader(long readerPtr);
}
