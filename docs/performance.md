# Some details of sonic-rs optimization

This document will introduce some performance optimization details of sonic-rs (commit `631411b`). Here are four main sections of optimization:

## Get fields from JSON/parsing JSON on-demand

The on-demand parsing algorithm focuses on skipping unnecessary fields, and the challenge lies in skipping JSON containers, including JSON Objects and JSON Arrays. This is because we need to pay attention to the brackets in the JSON string, such as `{ "key": "value {}"}`. We utilize the SIMD instructions to calculate the bitmap of the string, and then by counting the number of brackets, we can skip the entire JSON container. Reference the paper [JSONSki](https://dl.acm.org/doi/10.1145/3503222.3507719).

The overall algorithm is as follows:

```rs
#[inline(always)]
fn skip_container_loop(
    input: &[u8; 64],
    prev_instring: &mut u64,
    prev_escaped: &mut u64,
    lbrace_num: &mut usize,
    rbrace_num: &mut usize,
    left: u8,
    right: u8,
) -> Option<NonZeroU8> {
    
    let instring = get_string_bits(input, prev_instring, prev_escaped);
    // #Safety
    // the input is 64 bytes, so the v will be always valid.
    let v = unsafe { u8x64::from_slice_unaligned_unchecked(input) };
    let last_lbrace_num = *lbrace_num;
    let mut rbrace = (v.eq(u8x64::splat(right))).bitmask() & !instring;
    let lbrace = (v.eq(u8x64::splat(left))).bitmask() & !instring;
    while rbrace != 0 {
        *rbrace_num += 1;
        *lbrace_num = last_lbrace_num + (lbrace & (rbrace - 1)).count_ones() as usize;
        let is_closed = lbrace_num < rbrace_num;
        if is_closed {
            debug_assert_eq!(*rbrace_num, *lbrace_num + 1);
            let cnt = rbrace.trailing_zeros() + 1;
            return unsafe { Some(NonZeroU8::new_unchecked(cnt as u8)) };
        }
        rbrace &= rbrace - 1;
    }
    *lbrace_num = last_lbrace_num + lbrace.count_ones() as usize;
    None
}
```

The main steps of the algorithm are:

1. Calculate the JSON string bitmap `instring`.

For the bytes inside the string, we mark the corresponding bit in the bitmap as 1. Here we need to keep in mind that there might be escaped characters ('"', '\') in JSON strings. For example:
```
JSON    text  : "\\hel{}lo\""
insting bitmap: 0111111111110 
```

This SIMD branchless algorithm is borrowed from simdjson, implemented in `get_escaped_branchless_u64`.

2. How to skip Object or Array by matching brackets?

After obtaining the `instring`, we can XOR it with the corresponding `[]` or `{}` bitmap to get the actual bracket bitmap. Then, we can perform bracket matching. When a right bracket is found, it's likely that we need to do the bracket matching, as it could represent the end of an Object or Array. In the bracket matching operation, we will check if the number of right brackets exceeds the number of left brackets. If so, it indicates that the Object or Array has ended.

## Skip Space using SIMD

JSON specification includes space characters: ` `, `\n`, '\r', '\t'. To skip spaces using SIMD instructions, there are at least two implementation methods. One way is to directly use the compeq vector instruction to obtain each space character bitmap, then sum up to get the overall space bitmap. Another way is to directly use the shuffle SIMD instruction, an idea from simdjson. These two methods are implemented and tested [here](https://github.com/liuq19/simdstr/blob/main/examples/shuffle/bm_shuffle.cpp).

We find that JSON formats have both compact and pretty styles, and spaces are not too far apart in pretty format. Also, in common pretty formats, there is often only a single space between an Object ':' and its value. For example:

```
{
  "statuses": [
    {
      "metadata": {
        "result_type": "recent",
        "iso_language_code": "ja"
      },
```
(json snippet from twitter.json)

Thus, we save the calculated non-space character bitmap every time we skip spaces, which can save a lot of unnecessary SIMD computations later. We can refer to the following code in the `skip_space` function:

```rs
      // fast path 2: reuse the bitmap for short key or numbers
        let nospace_offset = (reader.index() as isize) - self.nospace_start;
        if nospace_offset < 64 {
            let bitmap = {
                let mask = !((1 << nospace_offset) - 1);
                self.nospace_bits & mask
            };
            if bitmap != 0 {
                let cnt = bitmap.trailing_zeros() as usize;
                let ch = reader.at(self.nospace_start as usize + cnt);
                reader.set_index(self.nospace_start as usize + cnt + 1);

                return Some(ch);
            } else {
                // we can still fast skip the marked space in here.
                reader.set_index(self.nospace_start as usize + 64);
            }
        }
```

In addition, we also optimize for compact JSON and cases where there's only one space, using the fast path. For instance, in the `skip_space` function:

```rs
        // fast path 1: for nospace or single space
        // most JSON is like ` "name": "balabala" `
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return Some(ch);
            }
        }
        if let Some(ch) = reader.next() {
            if !is_whitespace(ch) {
                return Some(ch);
            }
        }
```

## Float number parsing using SIMD

Parsing floating-point numbers is one of the most time-consuming operations in JSON parsing. For 16-length number strings, we can directly use SIMD instructions for parsing, as it can read ASCII number characters and accumulate them step by step. Refer to [simd_str2int](https://github.com/cloudwego/sonic-rs/blob/main/src/util/arch/x86_64.rs#L115) for the specific algorithm. This algorithm comes from [sonic-cpp](https://github.com/bytedance/sonic-cpp/blob/master/include/sonic/internal/arch/sse/str2int.h).

When parsing floating-point numbers, we only need to consider 17 significant digit bits for 64-bit floating-point numbers according to the IEEE754 specification. Thus, in this function, we employ a switch table to decrease unnecessary SIMD instructions.

## Use SIMD to serialize JSON string

When serializing JSON strings, especially long ones, utilizing SIMD is highly recommended. sonic-rs implements the `copy and find` algorithm.

```rs
    while nb >= LANS {
        // copy from the JSON string
        let v = {
            let raw = std::slice::from_raw_parts(sptr, LANS);
            u8x32::from_slice_unaligned_unchecked(raw)
        };
        v.write_to_slice_unaligned_unchecked(std::slice::from_raw_parts_mut(dptr, LANS));
        // if find the escaped character, then deal with it
        let mask = escaped_mask(v);
        if mask == 0 {
            nb -= LANS;
            dptr = dptr.add(LANS);
            sptr = sptr.add(LANS);
        } else {
            let cn = mask.trailing_zeros() as usize;
            nb -= cn;
            dptr = dptr.add(cn);
            sptr = sptr.add(cn);
            escape_unchecked(&mut sptr, &mut nb, &mut dptr);
        }
    }
```

## Arena memory allocator

In sonic-cpp, we discovered that allocating memory for each node in a document when parsing JSON into a document was a performance hotspot. Also, the C++ JSON library `rapidjson` uses a memory pool allocator to preallocate memory for the entire document. Therefore, we also use the `bump` crate in sonic-rs to preallocate memory for the entire document. Arena allocation can reduce memory allocation overhead and make the cache more friendly since the memory locations of nodes in the document are adjacent.

An interesting detail when parsing a JSON array or object is that we don't know beforehand how many children nodes are in the node. As a result, during parsing, we often need a vector to store intermediate nodes first, and only after the array or object parsing is completed can we create that object or array node on the document.

To save this performance overhead, we create a vector with a length of JSON length / 2 + 2 nodes before parsing JSON. Thus, during subsequent parsing, we don't need to resize the vector. If the required number of nodes exceeds the vector length, the JSON must be invalid.

```rs
        // optimize: use a pre-allocated vec.
        // If json is valid, the max number of value nodes should be
        // half of the valid json length + 2. like as [1,2,3,1,2,3...]
        // if the capacity is not enough, we will return a error.
        let nodes = Vec::with_capacity((json.len() / 2) + 2);
        let parent = 0;
        let mut visitor = DocumentVisitor {
            alloc,
            nodes,
            parent,
        };
```