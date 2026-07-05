package org.apache.paimon.index.fulltext;

public final class FullTextReadMetrics {

    private final long preadCalls;
    private final long preadRanges;
    private final long preadBytes;
    private final long cacheHits;
    private final long cacheMisses;
    private final long cacheEvictions;
    private final long cachedBlocks;

    public FullTextReadMetrics(
            long preadCalls,
            long preadRanges,
            long preadBytes,
            long cacheHits,
            long cacheMisses,
            long cacheEvictions,
            long cachedBlocks) {
        this.preadCalls = preadCalls;
        this.preadRanges = preadRanges;
        this.preadBytes = preadBytes;
        this.cacheHits = cacheHits;
        this.cacheMisses = cacheMisses;
        this.cacheEvictions = cacheEvictions;
        this.cachedBlocks = cachedBlocks;
    }

    public long preadCalls() {
        return preadCalls;
    }

    public long preadRanges() {
        return preadRanges;
    }

    public long preadBytes() {
        return preadBytes;
    }

    public long cacheHits() {
        return cacheHits;
    }

    public long cacheMisses() {
        return cacheMisses;
    }

    public long cacheEvictions() {
        return cacheEvictions;
    }

    public long cachedBlocks() {
        return cachedBlocks;
    }
}
