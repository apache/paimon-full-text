package org.apache.paimon.index.fulltext;

public final class FullTextIndexReader implements AutoCloseable {

    private long nativePtr;

    public FullTextIndexReader(FullTextIndexInput input) {
        if (input == null) {
            throw new NullPointerException("input");
        }
        this.nativePtr = FullTextNative.openReader(input);
    }

    public FullTextSearchResult search(FullTextQuery query, int limit) {
        if (query == null) {
            throw new NullPointerException("query");
        }
        return FullTextNative.searchJson(requireOpen(), query.toJson(), limit);
    }

    @Override
    public void close() {
        long ptr = nativePtr;
        nativePtr = 0;
        if (ptr != 0) {
            FullTextNative.freeReader(ptr);
        }
    }

    private long requireOpen() {
        if (nativePtr == 0) {
            throw new IllegalStateException("FullTextIndexReader is closed");
        }
        return nativePtr;
    }
}
