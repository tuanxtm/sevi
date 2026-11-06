// This file contains compatibility glue only. Typing decisions and persisted language memory live in Rust.
#ifdef SEVI_RUST_ENGINE
#include "Engine.h"
#include "RustBridge.h"
#include <algorithm>
#include <cstring>
#include <stdexcept>

int vTeencode = 1, vTelexStyle = 0, vAutoSwitchRequest = -1, vAutoSwitchLastSide = -1;
Uint32 vAutoSwitchUndoData[MAX_BUFF] = {}, vAutoSwitchRestoredKeys[MAX_BUFF] = {};
int vAutoSwitchUndoCount = 0, vAutoSwitchRestoredCount = 0;
bool vAutoSwitchPrevRestored = false;
static vKeyHookState hook = {};
static VkRustConfig config() {
    return {vLanguage, vCheckSpelling, vUseModernOrthography, vRestoreIfWrongSpelling,
            vAutoSwitchByTyping, vTeencode, vTelexStyle, vAutoSwitchPrevRestored,
            vAutoSwitchRequest, vAutoSwitchLastSide};
}
struct Context {
    void* ptr;
    Context() { auto c = config(); ptr = vk_create(&c); if (!ptr) throw std::runtime_error("Cannot initialize Sevi engine"); }
    ~Context() { vk_destroy(ptr); }
};
static void* context() { static Context c; auto settings = config(); vk_configure(c.ptr, &settings); return c.ptr; }
static void sync(void* c) {
    auto out = vk_output(c); if (!out) return;
    hook.code = out->hook.code; hook.backspaceCount = out->hook.backspaces;
    hook.newCharCount = out->hook.count; hook.extCode = out->hook.ext;
    std::copy_n(out->hook.chars, MAX_BUFF, hook.charData);
    vAutoSwitchRequest = out->request; vAutoSwitchLastSide = out->lastSide;
    vLanguage = out->language; vCheckSpelling = out->spelling;
    vAutoSwitchUndoCount = out->undoCount; std::copy_n(out->undo, MAX_BUFF, vAutoSwitchUndoData);
    vAutoSwitchRestoredCount = out->restoredCount; std::copy_n(out->restored, MAX_BUFF, vAutoSwitchRestoredKeys);
}
void* vKeyInit() { auto c=context(); vk_reset(c,3); sync(c); return &hook; }
void vKeyHandleEvent(const vKeyEvent& event,const vKeyEventState& state,const Uint16& key,const Uint8& caps,const bool& control) { auto c=context(); vk_handle(c,event,state,key,caps,control); sync(c); }
void startNewSession() { auto c=context(); vk_reset(c,0); sync(c); }
void vAutoSwitchForget() { auto c=context(); vk_reset(c,1); sync(c); }
void vSetCheckSpelling() { auto c=context(); vk_reset(c,2); sync(c); }
Uint32 getCharacterCode(const Uint32& value) { return vk_character(value); }
Uint16 keyCodeToCharacter(const Uint32& value) { return (Uint16)vk_key_character(value); }
bool vAutoSwitchCheckEnglishWord(const Uint32* keys,const int& count,const Uint32* const* previous,const int* counts,const int& n) { auto c=context(); bool yes=vk_check_word(c,keys,count,previous,counts,n);sync(c);return yes; }
bool vAutoSwitchCheckEnglishWord(const Uint32* keys,const int& count) { return vAutoSwitchCheckEnglishWord(keys,count,nullptr,nullptr,0); }
bool vAutoSwitchFixDoubledKey(const Uint32* keys,const int& count) { auto c=context();bool yes=vk_fix_double(c,keys,count);sync(c);return yes; }
int vAutoSwitchWordSide(const Uint32* keys,const int& count) { return vk_word_side(context(),keys,count); }
void vAutoSwitchSetBigrams(const char* path,const float& weight,const int& min) { vk_load_counts(context(),0,path,weight,min); }
void vAutoSwitchSetTrigrams(const char* path,const float& weight,const int& min) { vk_load_counts(context(),1,path,weight,min); }
void vAutoSwitchSetCapitalised(const char* path,const int& min) { vk_load_counts(context(),2,path,1,min); }
int vAutoSwitchWordScore(const int& vi,const char* letters,Uint16* out,int* len) { return vk_word_score(context(),vi,letters,out,len); }
bool vAutoSwitchIsVietnameseSpelling(const char* letters) { return vk_is_spelling(context(),letters); }
void vAutoSwitchNextKeyMass(const int& vi,const char* prefix,double out[27]) { vk_next_mass(vi,prefix,out); }
int vAutoSwitchCompletions(const int& vi,const char* prefix,char out[][32],int* scores,const int& max) { return vk_completions(vi,prefix,out,scores,max); }
void initSmartSwitchKey(const Byte* data,const int& size) { vk_smart_load(context(),data,size>0?(size_t)size:0); }
void getSmartSwitchKeySaveData(vector<Byte>& data) { auto c=context(); data.resize(vk_smart_save(c,nullptr,0)); vk_smart_save(c,data.data(),data.size()); }
int getAppInputMethodStatus(const string& app,const int& lang) { return vk_smart_get(context(),app.c_str(),lang,false); }
void setAppInputMethodStatus(const string& app,const int& lang) { vk_smart_get(context(),app.c_str(),lang,true); }
wstring utf8ToWideString(const string& value) { std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;return converter.from_bytes(value); }
string wideStringToUtf8(const wstring& value) { std::wstring_convert<std::codecvt_utf8_utf16<wchar_t>> converter;return converter.to_bytes(value); }
void* vTestSaveState() { return vk_snapshot(context()); }
void vTestRestoreState(void* state) { auto c=context();vk_restore(c,state);sync(c); }
void vTestFreeState(void* state) { vk_restore(nullptr,state); }
#endif
