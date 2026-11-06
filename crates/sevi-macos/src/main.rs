fn main() {
    #[cfg(target_os = "macos")]
    {
        sevi_macos::sevi_macos_main(0, std::ptr::null());
    }
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("Sevi's macOS desktop requires macOS.");
        std::process::exit(1);
    }
}
