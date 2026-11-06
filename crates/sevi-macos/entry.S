// Xcode needs a compilable input to schedule its application link phase.
// All application behavior and initialization live in Rust.
.text
.p2align 2
.globl _main
_main:
#if defined(__arm64__)
    b _sevi_macos_main
#elif defined(__x86_64__)
    jmp _sevi_macos_main
#else
#error Unsupported macOS architecture
#endif
