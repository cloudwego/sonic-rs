# Benchmark Test Data

All JSON files are stored in **compact (minified) format** with no extra whitespace.

## Value Type Statistics

| File | Size | Source | Integers | Floats | Strings | Bools | Nulls | Total Values | Dominant Types |
|------|------|--------|----------|--------|---------|-------|-------|--------------|----------------|
| book.json | 367 B | — | 7 | 3 | 7 | 8 | 0 | 25 | bool 32%, int 28%, string 28% |
| canada.json | 2.0 MB | nativejson-benchmark | 46 | 111,080 | 4 | 0 | 0 | 111,130 | float 99% |
| citm_catalog.json | 489 KB | nativejson-benchmark | 14,392 | 0 | 735 | 0 | 1,263 | 16,390 | int 87% |
| github_events.json | 52 KB | nativejson-benchmark | 149 | 0 | 752 | 64 | 24 | 989 | string 76%, int 15% |
| golang_source.json | 1.9 MB | go-json-experiment | 51,320 | 12,710 | 12,807 | 0 | 0 | 76,837 | int 66%, string 16%, float 16% |
| gsoc-2018.json | 2.9 MB | yyjson_benchmark | 0 | 0 | 15,168 | 0 | 0 | 15,168 | string 100% |
| lottie.json | 282 KB | yyjson_benchmark | 18,300 | 7,769 | 3,953 | 291 | 0 | 30,313 | int 60%, float 25%, string 13% |
| poet.json | 3.1 MB | yyjson_benchmark | 0 | 0 | 26,802 | 0 | 0 | 26,802 | string 100% |
| string_escaped.json | 17 KB | go-json-experiment | 0 | 0 | 60 | 0 | 0 | 60 | string 100% |
| string_unicode.json | 17 KB | go-json-experiment | 0 | 0 | 60 | 0 | 0 | 60 | string 100% |
| synthea_fhir.json | 1.1 MB | go-json-experiment | 724 | 1,251 | 24,931 | 118 | 0 | 27,024 | string 92% |
| twitter.json | 456 KB | nativejson-benchmark | 2,108 | 1 | 4,754 | 2,791 | 1,946 | 11,600 | string 40%, bool 24%, int 18% |
| twitterescaped.json | 549 KB | yyjson_benchmark | 2,108 | 1 | 4,754 | 2,791 | 1,946 | 11,600 | string 40%, bool 24%, int 18% |

## File Descriptions

### book.json (367 B)
Tiny mixed-type file. Integers are 2-10 digits. Good for quick sanity checks only.

### canada.json (2.0 MB)
GeoJSON border contour of Canada. Contains 111k **floating-point coordinates** in range [-141, 83], stored as deeply nested `[lon, lat]` pairs. Almost no strings or integers. Best file for **float parsing** benchmarks. Max depth 7.

### citm_catalog.json (489 KB)
Concert/event catalog data. 14k integers dominated by **9-digit IDs** (92%) and **5-digit area codes** (6%), plus 243 **13-digit timestamps**. Strings are short (avg 22 chars) with some **Unicode** (French accented text). Best file for **medium-to-large integer** parsing. Max depth 7.

### github_events.json (52 KB)
Real GitHub API response. Mixed integer sizes from 1 to 9 digits (timestamps, IDs, counts). Strings avg 50 chars with a few very long ones (up to 4KB commit messages). Contains some **escape sequences** (81 in raw JSON). Max depth 6.

### golang_source.json (1.9 MB)
Tree representation of Go source code. **51k integers** with a bimodal distribution: 25% are **1-digit** (weights/counts) and 75% are **10-digit** (Unix timestamps). Also has 12k floats (small values, code metrics) and 12k short ASCII strings (file paths). **Deeply nested** (max depth 32) with uniform schema. Best file for **integer-heavy** benchmarks.

### gsoc-2018.json (2.9 MB)
Google Summer of Code project listings. **Pure strings** (15k), averaging 185 chars with max 2.7k chars. 10% of strings contain **escape characters**, 4% have **multibyte Unicode**. High escape sequence density (12.6k in raw JSON). Best for **long ASCII string + escape** parsing. Max depth 3.

### lottie.json (282 KB)
Lottie animation configuration. 18k integers, **87% single-digit** (animation frame indices, flags), rest are 2-4 digits. Also has 7.7k floats (coordinates, bezier control points). Strings are short ASCII (avg 11 chars), **no escapes**. Best for **small integer** parsing. Max depth 15.

### poet.json (3.1 MB)
Chinese classical poetry collection. 27k strings, **67% contain CJK multibyte characters** (avg 46 chars). No numbers, no escape sequences in raw JSON. Best for **multibyte Unicode string** parsing. Max depth 2.

### string_escaped.json (17 KB)
Unicode script samples where **every character is `\uXXXX`-escaped**. Same content as string_unicode.json but encoded as escape sequences. Best for **JSON escape decoding** performance. Max depth 1.

### string_unicode.json (17 KB)
Unicode script samples (Arabic, CJK, Hangul, etc.) stored as **raw multibyte UTF-8**. 60 strings of ~100 chars each. No escape sequences. Best for **raw UTF-8 string** validation. Max depth 1.

### synthea_fhir.json (1.1 MB)
Synthea-generated FHIR healthcare Bundle. **92% strings** (25k) with avg 25 chars, short and highly repetitive (small unique set). 724 integers are **all single-digit**. **Deeply nested** objects (max depth 11). Best for **string-dominant deeply nested** benchmarks.

### twitter.json (456 KB)
Twitter API search response. **Mixed content** with all JSON types. Integers span 1-18 digits (IDs, timestamps, counts). Strings avg 29 chars, 7% contain **escape chars**, 16% have **multibyte Unicode** (CJK tweets). ~1k escape sequences in raw JSON. Best for **realistic mixed-content** benchmarks. Max depth 10.

### twitterescaped.json (549 KB)
Same data as twitter.json but with all Unicode characters **`\uXXXX`-escaped** (32.8k escape sequences in raw JSON vs 1k in twitter.json). Best for **escape-heavy mixed-content** benchmarks. Max depth 10.

## Recommended Selection by Test Focus

| Focus | Recommended Files | Why |
|-------|-------------------|-----|
| Integer parsing | golang_source, citm_catalog, lottie | Highest integer counts (51k, 14k, 18k) with different digit-length distributions |
| Float parsing | canada | 111k floats, nearly pure numeric |
| Mixed number | golang_source, lottie | Both integers and floats in significant proportion |
| String parsing | gsoc-2018, poet, synthea_fhir | Large string-dominant files (15k-27k strings) |
| Escape decoding | string_escaped, twitterescaped, gsoc-2018 | Heavy `\uXXXX` sequences and escape chars |
| Unicode / multibyte | string_unicode, poet, twitter | Raw multibyte UTF-8 (CJK, Arabic, etc.) |
| Mixed content | twitter, github_events | Realistic API responses with all JSON types |
| Deeply nested | golang_source, synthea_fhir, lottie | Deep object nesting (depth 11-32) |
| Overall throughput | canada, golang_source, poet | Largest files (2-3 MB) for throughput measurement |
