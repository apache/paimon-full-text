#!/usr/bin/env python3

# Licensed to the Apache Software Foundation (ASF) under one
# or more contributor license agreements.  See the NOTICE file
# distributed with this work for additional information
# regarding copyright ownership.  The ASF licenses this file
# to you under the Apache License, Version 2.0 (the
# "License"); you may not use this file except in compliance
# with the License.  You may obtain a copy of the License at
#
#   http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing,
# software distributed under the License is distributed on an
# "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
# KIND, either express or implied.  See the License for the
# specific language governing permissions and limitations
# under the License.

"""Smoke-test an installed paimon-ftindex wheel and its bundled native library."""

from io import BytesIO

from paimon_ftindex import FullTextIndexReader, FullTextIndexWriter


class BytesInput:
    def __init__(self, data):
        self._data = data

    def pread(self, pos, length):
        return self._data[pos : pos + length]


def main():
    output = BytesIO()
    with FullTextIndexWriter() as writer:
        writer.add_document(1, "Apache Paimon full text")
        writer.add_document(2, "Rust Tantivy search")
        writer.write(output)

    with FullTextIndexReader(BytesInput(output.getvalue())) as reader:
        row_ids, scores = reader.search('{"match":{"query":"paimon"}}', limit=10)

    if row_ids != [1] or len(scores) != 1 or scores[0] <= 0:
        raise RuntimeError(
            f"unexpected search result: row_ids={row_ids}, scores={scores}"
        )


if __name__ == "__main__":
    main()
