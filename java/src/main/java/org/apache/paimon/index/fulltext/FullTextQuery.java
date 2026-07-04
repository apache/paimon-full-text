package org.apache.paimon.index.fulltext;

public final class FullTextQuery {

    private final String json;

    private FullTextQuery(String json) {
        this.json = json;
    }

    public static FullTextQuery match(String terms, String column) {
        return match(terms, column, "Or");
    }

    public static FullTextQuery match(String terms, String column, String operator) {
        return new FullTextQuery(
                "{\"match\":{\"column\":\""
                        + escape(column)
                        + "\",\"terms\":\""
                        + escape(terms)
                        + "\",\"operator\":\""
                        + escape(operator)
                        + "\",\"boost\":1.0}}");
    }

    public static FullTextQuery phrase(String terms, String column) {
        return new FullTextQuery(
                "{\"match_phrase\":{\"column\":\""
                        + escape(column)
                        + "\",\"terms\":\""
                        + escape(terms)
                        + "\",\"slop\":0}}");
    }

    public static FullTextQuery json(String json) {
        if (json == null) {
            throw new NullPointerException("json");
        }
        return new FullTextQuery(json);
    }

    public String toJson() {
        return json;
    }

    private static String escape(String value) {
        if (value == null) {
            throw new NullPointerException("value");
        }
        StringBuilder builder = new StringBuilder(value.length() + 8);
        for (int i = 0; i < value.length(); i++) {
            char c = value.charAt(i);
            switch (c) {
                case '\\':
                    builder.append("\\\\");
                    break;
                case '"':
                    builder.append("\\\"");
                    break;
                case '\n':
                    builder.append("\\n");
                    break;
                case '\r':
                    builder.append("\\r");
                    break;
                case '\t':
                    builder.append("\\t");
                    break;
                default:
                    builder.append(c);
            }
        }
        return builder.toString();
    }
}
