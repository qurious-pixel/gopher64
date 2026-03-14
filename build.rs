use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo::rerun-if-changed=parallel-rdp");
    println!("cargo::rerun-if-changed=src/compat");

    let out_dir_var = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = PathBuf::from(out_dir_var);
    
    // 1. Slint Compilation
    let slint_config = slint_build::CompilerConfiguration::new().with_style("cosmic".into());
    if let Err(e) = slint_build::compile_with_config("src/ui/gui/appwindow.slint", slint_config) {
        println!("cargo:warning=Slint compilation failed: {}", e);
    }

    let mut volk_build = cc::Build::new();
    let mut rdp_build = cc::Build::new();
    let mut simd_build = cc::Build::new();

    // 2. Configure Volk
    volk_build
        .std("c17")
        .include("parallel-rdp/parallel-rdp-standalone/vulkan-headers/include")
        .file("parallel-rdp/parallel-rdp-standalone/volk/volk.c");

    // 3. Configure RDP
    rdp_build
        .cpp(true)
        .std("c++23")
        .flag("-Wno-unused-parameter")
        .flag("-Wno-missing-field-initializers")
        .file("parallel-rdp/parallel-rdp-standalone/parallel-rdp/command_ring.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/parallel-rdp/rdp_device.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/parallel-rdp/rdp_dump_write.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/parallel-rdp/rdp_renderer.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/parallel-rdp/video_interface.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/buffer.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/buffer_pool.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/command_buffer.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/command_pool.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/context.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/cookie.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/descriptor_set.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/device.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/event_manager.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/fence.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/fence_manager.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/image.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/indirect_layout.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/memory_allocator.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/pipeline_cache.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/pipeline_event.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/query_pool.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/render_pass.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/rtas.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/sampler.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/semaphore.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/semaphore_manager.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/shader.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/texture/texture_format.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/vulkan/wsi.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/arena_allocator.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/logging.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/thread_id.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/aligned_alloc.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/timer.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/timeline_trace_file.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/environment.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/thread_name.cpp")
        .file("parallel-rdp/parallel-rdp-standalone/util/slab_allocator.cpp")
        .file("parallel-rdp/interface.cpp")
        .file("parallel-rdp/wsi_platform.cpp")
        .include("parallel-rdp/parallel-rdp-standalone/parallel-rdp")
        .include("parallel-rdp/parallel-rdp-standalone/volk")
        .include("parallel-rdp/parallel-rdp-standalone/vulkan")
        .include("parallel-rdp/parallel-rdp-standalone/vulkan-headers/include")
        .include("parallel-rdp/parallel-rdp-standalone/util");

    // 4. Robust SDL3 Header Resolution
    // These variables are only exported if sdl3-sys/sdl3-ttf-sys define 'links'
    let sdl3_include = env::var("DEP_SDL3_OUT_DIR").map(|p| PathBuf::from(p).join("include"));
    let sdl3_ttf_include = env::var("DEP_SDL3_TTF_OUT_DIR").map(|p| PathBuf::from(p).join("include"));

    match (sdl3_include, sdl3_ttf_include) {
        (Ok(s1), Ok(s2)) => {
            rdp_build.include(s1).include(s2);
        }
        _ => {
            println!("cargo:warning=SDL3 or SDL3_TTF metadata not found. Using fallback includes.");
            // Fallback: You might want to add local include paths here if needed
        }
    }

    // 5. Architecture & OS Handling
    let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    
    let opt_flag = match arch.as_str() {
        "x86_64" => Some("-march=x86-64-v3"),
        "aarch64" => Some("-march=armv8.2-a"),
        _ => None,
    };

    if let Some(flag) = opt_flag {
        volk_build.flag(flag);
        rdp_build.flag(flag);
        simd_build.flag(flag);
    }

    if os == "windows" {
        volk_build.define("VK_USE_PLATFORM_WIN32_KHR", None);
        rdp_build.define("VK_USE_PLATFORM_WIN32_KHR", None);

        let _ = winresource::WindowsResource::new()
            .set_icon("data/icon/icon.ico") // Double check this path exists!
            .compile();
    }

    // 6. Finalize C++ Builds
    volk_build.flag("-flto=thin").compile("volk");
    rdp_build.flag("-flto=thin").compile("parallel-rdp");

    // 7. Robust Bindgen for Parallel RDP
    let rdp_header = "parallel-rdp/interface.hpp";
    if Path::new(rdp_header).exists() {
        bindgen::Builder::default()
            .header(rdp_header)
            .allowlist_function("rdp_.*") // simplified for robustness
            .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
            .generate()
            .map(|b| b.write_to_file(out_path.join("parallel_bindings.rs")))
            .iter().for_each(|res| if let Err(e) = res { println!("cargo:warning=Bindgen RDP failed: {}", e); });
    }
    
    // 8. Robust Bindgen for SIMD (ARM64 only)
    if arch == "aarch64" {
        let simd_header = "src/compat/sse2neon/sse2neon.h";
        if Path::new(simd_header).exists() {
            let b_res = bindgen::Builder::default()
                .header(simd_header)
                .clang_arg("-x")     // Add these two lines
                .clang_arg("c++")    // to force C++ mode
                .blocklist_type("__m128i")
                .blocklist_type("int64x2_t")
                .wrap_static_fns(true)
                .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
                .generate();
    
            match b_res {
                Ok(bindings) => {
                    let out_file = out_path.join("simd_bindings.rs");
                    if bindings.write_to_file(&out_file).is_ok() {
                        // Tell the compiler we successfully made the file
                        println!("cargo:rustc-cfg=simd_generated");
                        
                        simd_build
                            .cpp(true) // Ensure the C++ compiler is used here too
                            .std("c++17")
                            .flag("-DSSE2NEON_SUPPRESS_WARNINGS")
                            .file("src/compat/aarch64.c")
                            .file(out_path.join("bindgen/extern.c")) 
                            .include(".")
                            .compile("simd");
                    }
                }
                Err(e) => println!("cargo:warning=SIMD Bindgen failed: {}", e),
            }
        }
    }
    
    // 9. Robust Git Hash
    let git_hash = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);

    // 10. Netplay & Constants
    let netplay_id = env::var("NETPLAY_ID").unwrap_or_else(|_| "gopher64".into());
    println!("cargo:rustc-env=NETPLAY_ID={}", netplay_id);
    println!("cargo:rustc-env=N64_STACK_SIZE={}", 8 * 1024 * 1024);
}
