#include <stdarg.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>

#define SONIC_RS_DESERIALIZE_USE_RAWNUMBER 2

#define SONIC_RS_DESERIALIZE_UTF8_LOSSY 4

#define SONIC_RS_SERIALIZE_PRETTY 1

/**
 * A string allocated in Rust, ending with `\0`. Used for serialize output and error message.
 */
typedef struct SonicCString {
  const void *buf;
  uintptr_t len;
} SonicCString;

typedef struct SonicDeserializeRet {
  const void *value;
  struct SonicCString err;
} SonicDeserializeRet;

typedef struct SonicSerializeRet {
  struct SonicCString json;
  struct SonicCString err;
} SonicSerializeRet;

/**
 * # Safety
 *
 * The caller should drop the returned `value` or `err`.
 */
struct SonicDeserializeRet sonic_rs_deserialize_value(const char *json,
                                                      uintptr_t len,
                                                      uint64_t cfg);

/**
 * # Safety
 *
 * The caller should drop the returned `json` or `err`.
 */
struct SonicSerializeRet sonic_rs_serialize_value(const void *value, uint64_t cfg);

/**
 * # Safety
 */
void sonic_rs_drop_value(void *value);

/**
 * # Safety
 */
void sonic_rs_drop_string(uint8_t *buf, uint64_t len);
