package org.apache.paimon.index.fulltext;

import java.io.IOException;

public interface FullTextIndexInput {

    void readAt(long position, byte[] buffer, int offset, int length) throws IOException;
}
