package org.apache.paimon.index.fulltext;

import org.junit.Test;

import java.io.ByteArrayOutputStream;
import java.io.File;
import java.io.IOException;
import java.util.Collections;
import java.util.concurrent.atomic.AtomicInteger;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;
import static org.junit.Assume.assumeTrue;

public class FullTextNativeRoundTripTest {

    @Test
    public void testJavaNativeRoundTrip() throws Exception {
        assumeTrue(
                "PAIMON_FTINDEX_JNI_LIB_PATH must point to the built JNI library",
                nativeLibraryConfigured());

        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try (FullTextIndexWriter writer = FullTextIndexWriter.create(Collections.emptyMap())) {
            writer.addDocument(10L, "Apache Paimon full text search");
            writer.addDocument(11L, "Tantivy only");
            writer.writeIndex(output::write);
        }

        byte[] indexBytes = output.toByteArray();
        AtomicInteger maxBatchSize = new AtomicInteger();
        FullTextIndexInput input =
                new FullTextIndexInput() {
                    @Override
                    public void readAt(long position, byte[] buffer, int offset, int length)
                            throws IOException {
                        readAtBytes(indexBytes, position, buffer, offset, length);
                    }

                    @Override
                    public void pread(long[] positions, byte[][] buffers) throws IOException {
                        maxBatchSize.updateAndGet(current -> Math.max(current, positions.length));
                        FullTextIndexInput.super.pread(positions, buffers);
                    }
                };

        try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
            FullTextSearchResult result = reader.search(FullTextQuery.match("paimon", "text"), 10);

            assertTrue(maxBatchSize.get() > 1);
            assertEquals(1, result.size());
            assertEquals(10L, result.rowIds()[0]);
            assertTrue(result.scores()[0] > 0.0f);

            try {
                reader.search(FullTextQuery.match("paimon", "text"), 0);
                fail("Expected non-positive search limit to fail");
            } catch (IllegalArgumentException expected) {
                assertEquals("search limit must be positive", expected.getMessage());
            }
        }
    }

    private static boolean nativeLibraryConfigured() {
        String path = System.getenv("PAIMON_FTINDEX_JNI_LIB_PATH");
        return path != null && !path.isEmpty() && new File(path).isFile();
    }

    private static void readAtBytes(
            byte[] source, long position, byte[] buffer, int offset, int length) throws IOException {
        long end = position + length;
        if (position < 0 || end > source.length || end < position) {
            throw new IOException("read past end of index bytes");
        }
        System.arraycopy(source, (int) position, buffer, offset, length);
    }
}
