package org.apache.paimon.index.fulltext;

import java.io.IOException;

public interface FullTextIndexInput {

    /**
     * Reads exactly {@code length} bytes at {@code position} into {@code buffer}.
     *
     * <p>Implementations must be safe for concurrent calls. The native reader owns batching and
     * parallelism above this single-read callback.
     */
    void pread(long position, byte[] buffer, int offset, int length) throws IOException;
}
