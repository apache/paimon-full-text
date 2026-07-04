package org.apache.paimon.index.fulltext;

final class FullTextNative {

    static {
        String libPath = System.getenv("PAIMON_FTINDEX_JNI_LIB_PATH");
        if (libPath != null && !libPath.isEmpty()) {
            System.load(libPath);
        } else {
            System.loadLibrary("paimon_ftindex_jni");
        }
    }

    private FullTextNative() {}

    static native long createWriter(String[] keys, String[] values);

    static native void addDocument(long writerPtr, long rowId, String text);

    static native void writeIndex(long writerPtr, FullTextIndexOutput output);

    static native void freeWriter(long writerPtr);

    static native long openReader(FullTextIndexInput input);

    static native FullTextSearchResult searchJson(long readerPtr, String queryJson, int limit);

    static native void freeReader(long readerPtr);
}
