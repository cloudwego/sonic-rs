

#include <simdjson.h>
#include <sonic/sonic.h>


extern "C" {

bool sonic_cpp_parse_dom(char *json, size_t len) {
    sonic_json::Document doc;
    doc.Parse(json, len);
    return !doc.HasParseError();
}

bool simdjson_cpp_parse_dom(char *json, size_t len) {
    simdjson::dom::parser parser;
    simdjson::dom::element doc;
    auto error = parser.parse(json, len).get(doc);
    return !error;
}

}

