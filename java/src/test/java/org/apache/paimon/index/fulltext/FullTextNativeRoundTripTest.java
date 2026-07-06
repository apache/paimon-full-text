package org.apache.paimon.index.fulltext;

import org.junit.Test;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.util.Collections;
import java.util.LinkedHashMap;
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
            FullTextSearchResult result =
                    reader.search("{\"match\":{\"query\":\"paimon\"}}", 10);

            assertEquals(1, result.size());
            assertEquals(20L, result.rowIds()[0]);
            assertTrue(result.scores()[0] > 0.0f);
        }
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
