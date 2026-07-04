from io import BytesIO
import struct

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


def test_python_search_with_roaring_filter():
    output = BytesIO()
    allowed_id = (1 << 33) + 2
    with FullTextIndexWriter() as writer:
        writer.add_document(1, "Apache Paimon full text")
        writer.add_document(allowed_id, "Paimon filtered row")
        writer.write(output)

    filter_bytes = _roaring_treemap_bytes([allowed_id])
    with FullTextIndexReader(BytesInput(output.getvalue())) as reader:
        row_ids, scores = reader.search(
            MatchQuery("paimon"), limit=10, filter_bytes=filter_bytes
        )

    assert row_ids == [allowed_id]
    assert scores[0] > 0


def _roaring_treemap_bytes(ids):
    bitmaps = {}
    for value in sorted(set(ids)):
        high = value >> 32
        low = value & 0xFFFFFFFF
        bitmaps.setdefault(high, []).append(low)

    out = bytearray(struct.pack("<Q", len(bitmaps)))
    for high, values in bitmaps.items():
        out.extend(struct.pack("<I", high))
        out.extend(_roaring_bitmap_bytes(values))
    return bytes(out)


def _roaring_bitmap_bytes(values):
    containers = {}
    for value in values:
        key = value >> 16
        low = value & 0xFFFF
        containers.setdefault(key, []).append(low)

    out = bytearray(struct.pack("<II", 12346, len(containers)))
    descriptions = bytearray()
    offsets = bytearray()
    payload = bytearray()
    offset = 8 + len(containers) * 8
    for key, lows in containers.items():
        lows = sorted(set(lows))
        if len(lows) > 4096:
            raise ValueError("test helper only supports array containers")
        descriptions.extend(struct.pack("<HH", key, len(lows) - 1))
        offsets.extend(struct.pack("<I", offset))
        values_bytes = struct.pack("<" + "H" * len(lows), *lows)
        payload.extend(values_bytes)
        offset += len(values_bytes)
    return bytes(out + descriptions + offsets + payload)
