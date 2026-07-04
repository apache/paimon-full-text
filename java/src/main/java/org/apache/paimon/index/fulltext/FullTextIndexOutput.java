package org.apache.paimon.index.fulltext;

import java.io.IOException;

public interface FullTextIndexOutput {

    void write(byte[] buffer, int offset, int length) throws IOException;

    default void flush() throws IOException {}
}
