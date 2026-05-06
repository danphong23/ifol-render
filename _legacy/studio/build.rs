fn main() {
    // On Windows, the `rfd` crate needs `shlwapi` which may not ship
    // as a .a import lib with the rust-mingw toolchain.
    #[cfg(target_os = "windows")]
    {
        println!("cargo::rustc-link-lib=dylib=shlwapi");
        // Find the import lib through the Windows SDK or mingw.
        // The `windows` crate provides its own .lib import stubs via
        // link-search paths, but shlwapi is not always covered.
        // We use raw-dylib linking via extern block approach instead.
    }
}
