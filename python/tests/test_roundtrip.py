from io import BytesIO

from paimon_ftindex import FullTextIndexReader, FullTextIndexWriter, MatchQuery


class BytesInput:
    def __init__(self, data):
        self._data = data

    def pread(self, pos, length):
        return self._data[pos:pos + length]


def test_python_round_trip():
    output = BytesIO()
    with FullTextIndexWriter() as writer:
        writer.add_document(1, "Apache Paimon full text")
        writer.add_document(2, "Rust Tantivy search")
        writer.write(output)

    with FullTextIndexReader(BytesInput(output.getvalue())) as reader:
        row_ids, scores = reader.search(MatchQuery("paimon"), limit=10)

    assert row_ids == [1]
    assert scores[0] > 0
