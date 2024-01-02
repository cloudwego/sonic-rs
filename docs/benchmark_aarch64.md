## Benchmark in Apple M1 Pro

### Deserialize Struct

```
twitter/sonic_rs::from_slice_unchecked
                        time:   [436.54 µs 437.34 µs 438.22 µs]
twitter/sonic_rs::from_slice
                        time:   [457.72 µs 459.11 µs 460.80 µs]
twitter/simd_json::from_slice
                        time:   [424.34 µs 425.05 µs 425.92 µs]
twitter/serde_json::from_slice
                        time:   [831.10 µs 832.50 µs 834.16 µs]
twitter/serde_json::from_str
                        time:   [524.50 µs 525.55 µs 526.74 µs]

citm_catalog/sonic_rs::from_slice_unchecked
                        time:   [854.49 µs 855.71 µs 857.15 µs]
citm_catalog/sonic_rs::from_slice
                        time:   [892.97 µs 898.45 µs 904.43 µs]
citm_catalog/simd_json::from_slice
                        time:   [831.27 µs 837.38 µs 843.78 µs]
citm_catalog/serde_json::from_slice
                        time:   [1.3759 ms 1.3815 ms 1.3876 ms]
citm_catalog/serde_json::from_str
                        time:   [1.1859 ms 1.1875 ms 1.1894 ms]

canada/sonic_rs::from_slice_unchecked
                        time:   [3.1438 ms 3.1660 ms 3.1886 ms]
canada/sonic_rs::from_slice
                        time:   [3.1151 ms 3.1357 ms 3.1566 ms]
canada/simd_json::from_slice
                        time:   [3.2259 ms 3.2330 ms 3.2407 ms]
canada/serde_json::from_slice
                        time:   [4.9878 ms 5.0213 ms 5.0568 ms]
canada/serde_json::from_str
                        time:   [5.3256 ms 5.3714 ms 5.4191 ms]
```

### Deserialize Untyped

`cargo bench --bench deserialize_value  -- --quiet  "twitter|canada|citm_catalog"`

```
canada/sonic_rs_dom::from_slice
                        time:   [2.4394 ms 2.4495 ms 2.4606 ms]
canada/sonic_rs_dom::from_slice_unchecked
                        time:   [2.3656 ms 2.3697 ms 2.3744 ms]
canada/serde_json::from_slice
                        time:   [6.8682 ms 6.8864 ms 6.9067 ms]
canada/serde_json::from_str
                        time:   [6.9604 ms 6.9907 ms 7.0223 ms]
canada/simd_json::slice_to_owned_value
                        time:   [5.0212 ms 5.0402 ms 5.0602 ms]
canada/simd_json::slice_to_borrowed_value
                        time:   [5.0442 ms 5.0661 ms 5.0885 ms]

citm_catalog/sonic_rs_dom::from_slice
                        time:   [825.96 µs 827.98 µs 830.61 µs]
citm_catalog/sonic_rs_dom::from_slice_unchecked
                        time:   [805.69 µs 807.07 µs 808.59 µs]
citm_catalog/serde_json::from_slice
                        time:   [2.6804 ms 2.6872 ms 2.6942 ms]
citm_catalog/serde_json::from_str
                        time:   [2.4323 ms 2.4372 ms 2.4423 ms]
citm_catalog/simd_json::slice_to_owned_value
                        time:   [1.8281 ms 1.8348 ms 1.8418 ms]
citm_catalog/simd_json::slice_to_borrowed_value
                        time:   [1.3757 ms 1.3796 ms 1.3848 ms]

twitter/sonic_rs_dom::from_slice
                        time:   [380.30 µs 381.16 µs 382.14 µs]
twitter/sonic_rs_dom::from_slice_unchecked
                        time:   [357.51 µs 358.07 µs 358.70 µs]
twitter/serde_json::from_slice
                        time:   [1.5932 ms 1.5957 ms 1.5984 ms]
twitter/serde_json::from_str
                        time:   [1.2584 ms 1.2636 ms 1.2689 ms]
twitter/simd_json::slice_to_owned_value
                        time:   [892.94 µs 896.75 µs 900.67 µs]
twitter/simd_json::slice_to_borrowed_value
                        time:   [622.22 µs 622.47 µs 622.73 µs]
```

### Serialize Struct

`cargo bench --bench serialize_struct  -- --quiet`

```
twitter/sonic_rs::to_string
                        time:   [212.16 µs 213.44 µs 215.07 µs]
twitter/simd_json::to_string
                        time:   [300.20 µs 303.13 µs 306.55 µs]
twitter/serde_json::to_string
                        time:   [341.77 µs 343.50 µs 345.85 µs]

canada/sonic_rs::to_string
                        time:   [2.3674 ms 2.3730 ms 2.3785 ms]
canada/simd_json::to_string
                        time:   [2.9695 ms 2.9778 ms 2.9865 ms]
canada/serde_json::to_string
                        time:   [2.3422 ms 2.3555 ms 2.3706 ms]

citm_catalog/sonic_rs::to_string
                        time:   [325.60 µs 326.13 µs 326.71 µs]
citm_catalog/simd_json::to_string
                        time:   [374.37 µs 374.97 µs 375.66 µs]
citm_catalog/serde_json::to_string
                        time:   [431.37 µs 432.92 µs 434.81 µs]

```

### Serialize Untyped

`cargo bench --bench serialize_value  -- --quiet`

```
twitter/sonic_rs::to_string
                        time:   [168.74 µs 168.98 µs 169.24 µs]
twitter/serde_json::to_string
                        time:   [358.03 µs 358.89 µs 359.93 µs]
twitter/simd_json::to_string
                        time:   [382.20 µs 383.01 µs 383.97 µs]

citm_catalog/sonic_rs::to_string
                        time:   [336.69 µs 337.15 µs 337.66 µs]
citm_catalog/serde_json::to_string
                        time:   [588.08 µs 594.31 µs 601.53 µs]
citm_catalog/simd_json::to_string
                        time:   [814.63 µs 815.93 µs 817.37 µs]

canada/sonic_rs::to_string
                        time:   [2.8751 ms 2.8912 ms 2.9102 ms]
canada/serde_json::to_string
                        time:   [2.8237 ms 2.8298 ms 2.8357 ms]
canada/simd_json::to_string
                        time:   [3.4206 ms 3.4268 ms 3.4335 ms]
```

### Get from JSON

`cargo bench --bench get_from -- --quiet`

The benchmark is getting a specific field from the twitter JSON. 

- sonic-rs::get_unchecked_from_str: without validate
- sonic-rs::get_from_str: with validate
- gjson::get_from_str: without validate

Sonic-rs utilize SIMD to quickly skip unnecessary fields in the unchecked case, thus enhancing the performance.

```
twitter/sonic-rs::get_unchecked_from_str
                        time:   [51.211 µs 51.285 µs 51.363 µs]
twitter/sonic-rs::get_from_str
                        time:   [374.21 µs 376.41 µs 379.08 µs]
twitter/gjson::get_from_str
                        time:   [159.11 µs 159.39 µs 159.69 µs]
```

