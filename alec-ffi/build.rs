// ALEC - Adaptive Lazy Evolving Compression
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Build script for alec-ffi
//!
//! Generates C header file using cbindgen.

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let output_file = format!("{}/include/alec_generated.h", crate_dir);

    // Generate C bindings using builder pattern
    match cbindgen::Builder::new()
        .with_crate(&crate_dir)
        .with_language(cbindgen::Language::C)
        .with_include_guard("ALEC_GENERATED_H")
        .with_cpp_compat(true)
        .generate()
    {
        Ok(bindings) => {
            bindings.write_to_file(&output_file);
            println!("cargo:rerun-if-changed=src/lib.rs");
        }
        Err(e) => {
            eprintln!("Warning: cbindgen failed: {}", e);
            // Don't fail the build - we have a manual header file
        }
    }
}
