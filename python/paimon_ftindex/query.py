import json


class FullTextQuery:
    def to_dict(self):
        raise NotImplementedError

    def to_json(self):
        return json.dumps(self.to_dict(), separators=(",", ":"))


class MatchQuery(FullTextQuery):
    def __init__(self, terms, column="text", operator="Or", boost=1.0):
        self.terms = str(terms)
        self.column = str(column)
        self.operator = str(operator)
        self.boost = float(boost)

    def to_dict(self):
        return {
            "match": {
                "column": self.column,
                "terms": self.terms,
                "operator": self.operator,
                "boost": self.boost,
            }
        }


class PhraseQuery(FullTextQuery):
    def __init__(self, terms, column="text", slop=0):
        self.terms = str(terms)
        self.column = str(column)
        self.slop = int(slop)

    def to_dict(self):
        return {
            "match_phrase": {
                "column": self.column,
                "terms": self.terms,
                "slop": self.slop,
            }
        }
