package org.apache.paimon.index.fulltext;

public final class FullTextIndexReader implements AutoCloseable {

    private long nativePtr;

    public FullTextIndexReader(FullTextIndexInput input) {
        if (input == null) {
            throw new NullPointerException("input");
        }
        this.nativePtr = FullTextNative.openReader(input);
    }

    public FullTextSearchResult search(String query, int limit) {
        if (query == null) {
            throw new NullPointerException("query");
        }
        if (limit <= 0) {
            throw new IllegalArgumentException("search limit must be positive");
        }
        return FullTextNative.search(requireOpen(), query, limit);
    }

    public FullTextSearchResult search(String query, int limit, byte[] roaringFilter) {
        if (query == null) {
            throw new NullPointerException("query");
        }
        if (limit <= 0) {
            throw new IllegalArgumentException("search limit must be positive");
        }
        if (roaringFilter == null) {
            throw new NullPointerException("roaringFilter");
        }
        return FullTextNative.searchWithRoaringFilter(requireOpen(), query, limit, roaringFilter);
    }

    public FullTextReadMetrics readMetrics() {
        return FullTextNative.readMetrics(requireOpen());
    }

    public void prewarm() {
        FullTextNative.prewarm(requireOpen());
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
