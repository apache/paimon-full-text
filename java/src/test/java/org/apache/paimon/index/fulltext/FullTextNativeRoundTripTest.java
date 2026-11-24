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

import org.junit.Test;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.lang.ref.WeakReference;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;
import static org.junit.Assume.assumeTrue;

public class FullTextNativeRoundTripTest {

    @Test
    public void testJavaNativeRoundTrip() throws Exception {
        assumeTrue(
                "PAIMON_FTINDEX_JNI_LIB_PATH must point to the built JNI library "
                        + "or the native library must be present in classpath resources",
                nativeLibraryAvailable());

        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try (FullTextIndexWriter writer = FullTextIndexWriter.create(Collections.emptyMap())) {
            writer.addDocument(10L, "Apache Paimon full text search");
            writer.addDocument(11L, "Tantivy only");
            writer.writeIndex(output::write);
        }

        byte[] indexBytes = output.toByteArray();
        AtomicInteger bytesRead = new AtomicInteger();
        FullTextIndexInput input =
                new FullTextIndexInput() {
                    @Override
                    public void pread(long position, byte[] buffer, int offset, int length)
                            throws IOException {
                        bytesRead.addAndGet(length);
                        preadBytes(indexBytes, position, buffer, offset, length);
                    }
                };

        try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
            assertEquals("text", reader.defaultTextField());
            int bytesReadAtOpen = bytesRead.get();
            FullTextReadMetrics metrics = reader.readMetrics();
            assertTrue(metrics.preadCalls() >= 2);
            assertTrue(metrics.preadBytes() > 16);
            reader.prewarm();
            FullTextReadMetrics afterPrewarm = reader.readMetrics();
            assertTrue(afterPrewarm.preadCalls() > metrics.preadCalls());
            FullTextSearchResult result = reader.search(matchQuery("paimon", "text"), 10);
            FullTextReadMetrics afterSearch = reader.readMetrics();

            assertTrue(bytesReadAtOpen < indexBytes.length);
            assertEquals(1, result.size());
            assertEquals(10L, result.rowIds()[0]);
            assertTrue(result.scores()[0] > 0.0f);
            assertTrue(afterSearch.preadCalls() >= afterPrewarm.preadCalls());
            assertTrue(afterSearch.cacheMisses() >= metrics.cacheMisses());

            try {
                reader.search(matchQuery("paimon", "text"), 0);
                fail("Expected non-positive search limit to fail");
            } catch (IllegalArgumentException expected) {
                assertEquals("search limit must be positive", expected.getMessage());
            }
        }
    }

    @Test
    public void testJavaNativeMultiFieldRoundTrip() throws Exception {
        assumeTrue(
                "PAIMON_FTINDEX_JNI_LIB_PATH must point to the built JNI library "
                        + "or the native library must be present in classpath resources",
                nativeLibraryAvailable());

        Map<String, String> options = Collections.singletonMap("text-fields", "title,body");
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try (FullTextIndexWriter writer = FullTextIndexWriter.create(options)) {
            Map<String, String> fields = new LinkedHashMap<>();
            fields.put("title", "Apache Paimon");
            fields.put("body", "lake storage");
            writer.addDocument(20L, fields);
            writer.writeIndex(output::write);
        }

        byte[] indexBytes = output.toByteArray();
        FullTextIndexInput input =
                (position, buffer, offset, length) ->
                        preadBytes(indexBytes, position, buffer, offset, length);

        try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
            assertEquals("title", reader.defaultTextField());
            FullTextSearchResult result =
                    reader.search("{\"match\":{\"query\":\"paimon\"}}", 10);

            assertEquals(1, result.size());
            assertEquals(20L, result.rowIds()[0]);
            assertTrue(result.scores()[0] > 0.0f);
        }
    }

    @Test
    public void testDefaultTextFieldReflectsArchiveConfiguration() throws Exception {
        assumeTrue("native library must be available", nativeLibraryAvailable());
        for (Map<String, String> options :
                Arrays.asList(
                    Collections.singletonMap("text-field", "内容"),
                    Collections.singletonMap("text-fields", "body,title"),
                    conflictingFieldOptions()
                )) {
            ByteArrayOutputStream output = new ByteArrayOutputStream();
            try (FullTextIndexWriter writer = FullTextIndexWriter.create(options)) {
                writer.addDocument(3L, "apache paimon");
                writer.writeIndex(output::write);
            }
            byte[] indexBytes = output.toByteArray();
            FullTextIndexReader reader =
                    new FullTextIndexReader(
                            (position, buffer, offset, length) ->
                                    preadBytes(indexBytes, position, buffer, offset, length));
            String field;
            try {
                field = reader.defaultTextField();
                assertEquals(1, reader.search(matchQuery("paimon", field), 10).size());
                if (options.containsKey("text-field")) {
                    assertEquals("内容", field);
                } else if (!options.containsKey("columns")) {
                    assertEquals("body", field);
                } else {
                    assertTrue(field.equals("body") || field.equals("title"));
                }
            } finally {
                reader.close();
            }
            try {
                reader.defaultTextField();
                fail("Expected closed reader to fail");
            } catch (IllegalStateException expected) {
                assertEquals("FullTextIndexReader is closed", expected.getMessage());
            }
        }
    }

    private static Map<String, String> conflictingFieldOptions() {
        Map<String, String> options = new LinkedHashMap<>();
        options.put("text-fields", "body");
        options.put("columns", "title");
        return options;
    }

    @Test
    public void testCallbackBuffersAreReleasedDuringNativeCall() throws Exception {
        assumeTrue(
                "PAIMON_FTINDEX_JNI_LIB_PATH must point to the built JNI library "
                        + "or the native library must be present in classpath resources",
                nativeLibraryAvailable());

        // The native side allocates one byte[] per callback. Each must become collectable as
        // soon as its callback returns, not only when the enclosing native call returns:
        // otherwise a single write or search pins every byte it transfers on the Java heap.
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        List<WeakReference<byte[]>> written = new ArrayList<>();
        AtomicInteger writtenStillReachable = new AtomicInteger();
        Map<String, String> options = new LinkedHashMap<>();
        options.put("build.writer-threads", "2");
        options.put("build.writer-memory-size", "40mb");
        try (FullTextIndexWriter writer = FullTextIndexWriter.create(options)) {
            StringBuilder text = new StringBuilder();
            for (int i = 0; i < 20000; i++) {
                text.setLength(0);
                for (int j = 0; j < 20; j++) {
                    text.append("term").append((i * 31 + j * 7919) % 50000).append(' ');
                }
                writer.addDocument(i, text.toString());
            }
            writer.writeIndex(
                    (buffer, offset, length) -> {
                        if (written.size() == 20) {
                            writtenStillReachable.set(countReachableAfterGc(written));
                        }
                        output.write(buffer, offset, length);
                        written.add(new WeakReference<>(buffer));
                    });
        }
        assertTrue("expected more than 20 write callbacks", written.size() > 20);
        assertEquals(0, writtenStillReachable.get());

        byte[] indexBytes = output.toByteArray();
        List<WeakReference<byte[]>> read = new ArrayList<>();
        AtomicInteger maxReadStillReachable = new AtomicInteger();
        FullTextIndexInput input =
                (position, buffer, offset, length) -> {
                    if (read.size() % 8 == 7) {
                        maxReadStillReachable.accumulateAndGet(
                                countReachableAfterGc(read), Math::max);
                    }
                    preadBytes(indexBytes, position, buffer, offset, length);
                    read.add(new WeakReference<>(buffer));
                };
        try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
            read.clear();
            FullTextSearchResult result =
                    reader.search(matchQuery("term1 term2 term3 term4 term5 term6", "text"), 10);
            assertTrue(result.size() > 0);
            assertTrue("expected at least 8 reads in one search", read.size() >= 8);
            assertEquals(0, maxReadStillReachable.get());
        }
    }

    private static int countReachableAfterGc(List<WeakReference<byte[]>> references) {
        int reachable = references.size();
        for (int attempt = 0; attempt < 20 && reachable > 0; attempt++) {
            System.gc();
            System.runFinalization();
            reachable = 0;
            for (WeakReference<byte[]> reference : references) {
                if (reference.get() != null) {
                    reachable++;
                }
            }
        }
        return reachable;
    }

    private static boolean nativeLibraryAvailable() {
        String path = System.getenv("PAIMON_FTINDEX_JNI_LIB_PATH");
        if (path != null && !path.isEmpty() && new File(path).isFile()) {
            return true;
        }
        return FullTextNativeRoundTripTest.class.getResource(nativeResourcePath()) != null;
    }

    private static String nativeResourcePath() {
        String os = normalizeOs(System.getProperty("os.name", ""));
        String arch = normalizeArch(System.getProperty("os.arch", ""));
        return "/native/" + os + "/" + arch + "/" + mapLibraryName(os);
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
        return "unsupported";
    }

    private static String normalizeArch(String archName) {
        String lower = archName.toLowerCase();
        if (lower.equals("amd64") || lower.equals("x86_64")) {
            return "x86_64";
        } else if (lower.equals("aarch64") || lower.equals("arm64")) {
            return "aarch64";
        }
        return "unsupported";
    }

    private static String mapLibraryName(String os) {
        if (os.equals("linux")) {
            return "libpaimon_ftindex_jni.so";
        } else if (os.equals("macos")) {
            return "libpaimon_ftindex_jni.dylib";
        } else if (os.equals("windows")) {
            return "paimon_ftindex_jni.dll";
        }
        return "unsupported";
    }

    private static String matchQuery(String terms, String column) {
        return "{\"match\":{\"query\":\"" + terms + "\",\"column\":\"" + column + "\"}}";
    }

    private static void preadBytes(
            byte[] source, long position, byte[] buffer, int offset, int length) throws IOException {
        long end = position + length;
        if (position < 0 || end > source.length || end < position) {
            throw new IOException("read past end of index bytes");
        }
        System.arraycopy(source, (int) position, buffer, offset, length);
    }
}
