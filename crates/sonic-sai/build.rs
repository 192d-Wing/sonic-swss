//! Build script for sonic-sai crate.
//!
//! When the `generate-bindings` feature is enabled and SAI headers are available,
//! this will use bindgen to generate Rust FFI bindings for the SAI C API.

fn main() {
    // Uncomment the following when SAI headers are available:
    //
    // #[cfg(feature = "generate-bindings")]
    // {
    //     use std::env;
    //     use std::path::PathBuf;
    //
    //     let bindings = bindgen::Builder::default()
    //         .header("/usr/include/sai/sai.h")
    //         .header("/usr/include/sai/saiport.h")
    //         .header("/usr/include/sai/sairoute.h")
    //         .header("/usr/include/sai/saiacl.h")
    //         .header("/usr/include/sai/sainexthop.h")
    //         .header("/usr/include/sai/sainexthopgroup.h")
    //         .header("/usr/include/sai/saifdb.h")
    //         .header("/usr/include/sai/saineighbor.h")
    //         .header("/usr/include/sai/saivlan.h")
    //         .header("/usr/include/sai/saibridge.h")
    //         .header("/usr/include/sai/saibuffer.h")
    //         .header("/usr/include/sai/saiqueue.h")
    //         .header("/usr/include/sai/saischeduler.h")
    //         .allowlist_function("sai_.*")
    //         .allowlist_type("sai_.*")
    //         .allowlist_type("_sai_.*")
    //         .allowlist_var("SAI_.*")
    //         .derive_debug(true)
    //         .derive_default(true)
    //         .derive_eq(true)
    //         .derive_hash(true)
    //         .generate()
    //         .expect("Unable to generate SAI bindings");
    //
    //     let out_path = PathBuf::from(env::var("OUT_DIR").unwrap());
    //     bindings
    //         .write_to_file(out_path.join("sai_bindings.rs"))
    //         .expect("Couldn't write SAI bindings");
    //
    //     println!("cargo:rustc-link-lib=sai");
    //     println!("cargo:rustc-link-lib=sairedis");
    //     println!("cargo:rustc-link-lib=saimeta");
    // }

    println!("cargo:rerun-if-changed=build.rs");
}
