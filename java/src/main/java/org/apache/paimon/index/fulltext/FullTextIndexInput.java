package org.apache.paimon.index.fulltext;

import java.io.IOException;

public interface FullTextIndexInput {

    void readAt(long position, byte[] buffer, int offset, int length) throws IOException;

    default void pread(long[] positions, byte[][] buffers) throws IOException {
        if (positions == null) {
            throw new NullPointerException("positions");
        }
        if (buffers == null) {
            throw new NullPointerException("buffers");
        }
        if (positions.length != buffers.length) {
            throw new IllegalArgumentException(
                    "positions length " + positions.length + " does not match buffers length "
                            + buffers.length);
        }
        for (int i = 0; i < positions.length; i++) {
            byte[] buffer = buffers[i];
            if (buffer == null) {
                throw new NullPointerException("buffers[" + i + "]");
            }
            readAt(positions[i], buffer, 0, buffer.length);
        }
    }
}
