// Transitional ABI between the existing native hosts and the owned Rust engine.
#pragma once
#include <cstddef>
#include <cstdint>
extern "C" {
struct VkRustConfig {
    int32_t language, spelling, modern, restore, autoSwitch, teencode, style;
    int32_t previousRestored, request, lastSide;
};
struct VkRustHook { uint8_t code, backspaces, count, ext; uint32_t chars[32]; };
struct VkRustOutput {
    VkRustHook hook;
    int32_t request, lastSide, language, spelling, undoCount;
    uint32_t undo[32];
    int32_t restoredCount;
    uint32_t restored[32];
};
void* vk_create(const VkRustConfig*);
void vk_destroy(void*);
void vk_configure(void*, const VkRustConfig*);
const VkRustOutput* vk_output(void*);
void vk_handle(void*, int32_t, int32_t, uint32_t, uint8_t, bool);
void vk_reset(void*, int32_t);
bool vk_check_word(void*, const uint32_t*, int32_t, const uint32_t* const*, const int32_t*, int32_t);
bool vk_fix_double(void*, const uint32_t*, int32_t);
int32_t vk_word_side(void*, const uint32_t*, int32_t);
int32_t vk_word_score(const void*, int32_t, const char*, uint16_t*, int32_t*);
bool vk_is_spelling(const void*, const char*);
uint32_t vk_character(uint32_t);
uint32_t vk_key_character(uint32_t);
void vk_next_mass(int32_t, const char*, double*);
int32_t vk_completions(int32_t, const char*, char (*)[32], int32_t*, int32_t);
void vk_load_counts(void*, int32_t, const char*, float, int32_t);
void vk_smart_load(void*, const uint8_t*, size_t);
size_t vk_smart_save(const void*, uint8_t*, size_t);
int32_t vk_smart_get(void*, const char*, int32_t, bool);
void* vk_snapshot(const void*);
void vk_restore(void*, void*);
}
static_assert(sizeof(VkRustHook) == 132, "Rust hook ABI changed");
static_assert(sizeof(VkRustOutput) == 412, "Rust output ABI changed");
