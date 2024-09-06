# Sonic-rs 优化细节

下面介绍一些sonic-rs的性能优化细节，其中代码版本是commit `631411b`. 

## 按需解析

如何实现一个性能更好的按需解析算法。按需解析的性能关键在于跳过不需要的字段，其中难点在于如何跳过 JSON container， 包括 JSON Object 和 JSON array，因为我们需要注意 JSON 字符串中的括号，例如 `"{ "key": "value {}"}`。 我们利用了 simd 指令计算字符串的bitmap，然后通过计算括号的数量来跳过整个JSON container。参考论文 [JSONSki](https://dl.acm.org/doi/10.1145/3503222.3507719).

整体算法如下：

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

主要的算法步骤如下：
1. 计算 JSON 字符串的 bitmap `instring`。

对于在字符串中的字节，我们将bitmap中对应位置的bit标记为1。 这里面需要注意 JSON 字符串中可能包含 escaped 字符 ('"', '\'). 例如:
```
JSON    text  : "\\hel{}lo\""
insting bitmap: 0111111111110 
```

这里利用了 simdjson 的无分支的 SIMD 算法，代码在 `get_escaped_branchless_u64`。

2. 如何通过匹配括号数量来跳过 Object 或 array？

我们得到 `instring` 之后，再通过于 `[]` 或 `{}` bitmap的异或操作，就可以得到真正的括号bitmap。然后以此来进行括号匹配操作。每当发现有右括号存在时，这时候有可能我们就需要进行括号匹配, 因为右括号有可能是Object或array 结束位置。
在括号匹配操作里面，我们挨个判断右括号的数量是否大于之前的左括号数量，如果超过了，说明该 Object 或 Array 已经结束。

## Skip Space using SIMD

JSON 规范中的空格字符有: ` `, `\n`, '\r', '\t`. 利用 SIMD 指令跳过空格，至少有两种实现方式。
一种方式是直接使用 compeq 向量指令得到各个空格字符的 bitmap，然后进行汇总得到空格的bitmap。还有一种方式是直接利用 shuffle SIMD 指令，这个idead来源于 simdjson。这里面有两种方式的[实现测试](https://github.com/liuq19/simdstr/blob/main/examples/shuffle/bm_shuffle.cpp).

我们发现JSON的格式有紧凑的和pretty的，空格之间相隔并不远。而且在常见的 pretty 格式下，Object 的':' 和value 中间往往只隔一个空格。例如:
```
{
  "statuses": [
    {
      "metadata": {
        "result_type": "recent",
        "iso_language_code": "ja"
      },
```
(json 片段来自 twitter.json)

因此，我们在每次跳过空格时，将计算得到的非空格字符的 bitmap保存下来，后面跳过空格时，查询这个bitmap这样能够节省后续很多不必要的 simd 计算。可以参考 `skip_space` 函数中的下列代码：
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

另外，我们还针对紧凑 JSON 和只有一个空格的情况，使用了fastpath。例如， 在 `skip_space` 函数中：
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

## 使用 SIMD 解析浮点数

浮点数解析是 JSON 解析中的一个非常耗时的操作。在很多浮点数中，往往有比较长的尾数，例如 `canada.json` 中，浮点数尾数部分是15位：
```
[[[-65.613616999999977,43.420273000000009],[-65.619720000000029,43.418052999999986],[-65.625,43.421379000000059],[-65.636123999999882,43.449714999999969],[-65.633056999999951,43.474709000000132],[-65.611389000000031,43.513054000000068],[-65.605835000000013,43.516105999999979],[-65.598343,43.515830999999935],[-65.
```


对于长度为16的数字字符串，是可以直接使用 SIMD 指令进行解析，读取 ascii 数字字符并且逐步累加的。 具体算法可以参考[simd_str2int](https://github.com/cloudwego/sonic-rs/blob/main/src/util/arch/x86_64.rs#L115)。这个算法来源于 [sonic-cpp](https://github.com/bytedance/sonic-cpp/blob/master/include/sonic/internal/arch/sse/str2int.h). 在解析浮点数时，按照 IEEE754 规范，对于64 位浮点数，我们只需要关注17位有效数字。因此，在这个函数里面使用了一个 switch table 来减少不必要的 SIMD 指令。


## 使用 SIMD 序列化 JSON string

在序列化JSON字符串时, 如果JSON字符串比较长，非常适合使用SIMD。sonic-rs 使用了 `copy and find` 的算法。

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

## 内存池分配器

我们之前在 sonic-cpp 中发现，在将 JSON 解析到 document时，document 中对每个节点内存的分配，是一个性能热点，同时，在 c++ JSON 库`rapidjson` 使用了memory pool allocator 来统一预分配 document的内存。因此，我们在sonic-rs中也使用 `bump` crate来对 整个document 进行预分配内存。Arena 机制能够减少内存分配开销，同时让缓存变得更加友好，因为 document 的各个节点的内存位置是邻近的。

这里面有一个有趣的细节是，我们发现在解析 JSON array 或object时，我们事先不知道该节点里面有多少children 节点。因此，在解析的过程中，往往需要一个vector先存储中间节点，等到array或object解析完成之后，最后才能在document上面创建该 object 或 array节点。

 为了节省这一块性能开销，我们在解析JSON前，预分配了一个长度为 JSON length/2 + 2 个节点的vector作为中间存储。因此，在后续解析过程中，我们无需对该vector 进行扩容。因为当需要的节点数量超过 vector 长度时，此时的 JSON 必定是不合法的。

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

