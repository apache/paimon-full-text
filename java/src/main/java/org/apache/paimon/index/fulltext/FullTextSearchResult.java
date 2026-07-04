package org.apache.paimon.index.fulltext;

public final class FullTextSearchResult {

    private final long[] rowIds;
    private final float[] scores;

    public FullTextSearchResult(long[] rowIds, float[] scores) {
        this.rowIds = rowIds;
        this.scores = scores;
    }

    public long[] rowIds() {
        return rowIds;
    }

    public float[] scores() {
        return scores;
    }

    public int size() {
        return rowIds.length;
    }
}
