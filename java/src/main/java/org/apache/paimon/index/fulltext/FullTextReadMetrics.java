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
