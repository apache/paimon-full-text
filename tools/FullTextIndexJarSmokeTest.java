/*
 * Licensed to the Apache Software Foundation (ASF) under one
 * or more contributor license agreements.  See the NOTICE file
 * distributed with this work for additional information
 * regarding copyright ownership.  The ASF licenses this file
 * to you under the Apache License, Version 2.0 (the
 * "License"); you may not use this file except in compliance
 * with the License.  You may obtain a copy of the License at
 *
 *   http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing,
 * software distributed under the License is distributed on an
 * "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
 * KIND, either express or implied.  See the License for the
 * specific language governing permissions and limitations
 * under the License.
 */

import org.apache.paimon.index.fulltext.FullTextIndexInput;
import org.apache.paimon.index.fulltext.FullTextIndexReader;
import org.apache.paimon.index.fulltext.FullTextIndexWriter;
import org.apache.paimon.index.fulltext.FullTextSearchResult;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.util.Collections;

/** Smoke-tests a packaged jar and the native library selected for the current platform. */
public class FullTextIndexJarSmokeTest {

    public static void main(String[] args) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try (FullTextIndexWriter writer =
                FullTextIndexWriter.create(Collections.emptyMap())) {
            writer.addDocument(1L, "Apache Paimon full text");
            writer.addDocument(2L, "Rust Tantivy search");
            writer.writeIndex(output::write);
        }

        byte[] indexBytes = output.toByteArray();
        FullTextIndexInput input =
                (position, buffer, offset, length) ->
                        pread(indexBytes, position, buffer, offset, length);

        try (FullTextIndexReader reader = new FullTextIndexReader(input)) {
            FullTextSearchResult result =
                    reader.search("{\"match\":{\"query\":\"paimon\"}}", 10);
            if (result.size() != 1
                    || result.rowIds()[0] != 1L
                    || result.scores()[0] <= 0.0f) {
                throw new IllegalStateException("unexpected packaged-jar search result");
            }
        }
    }

    private static void pread(
            byte[] source, long position, byte[] buffer, int offset, int length)
            throws IOException {
        long end = position + length;
        if (position < 0 || end > source.length || end < position) {
            throw new IOException("read past end of index bytes");
        }
        System.arraycopy(source, (int) position, buffer, offset, length);
    }
}
