//! Compiles the bundled tree-sitter-org C grammar (parser + external scanner)
//! directly into this crate. There is no separate grammar crate; the generated
//! `parser.c` and the pure-C `scanner.c` live under `grammar/`.

fn main() {
    let grammar_dir = std::path::Path::new("grammar");

    let mut c_config = cc::Build::new();
    c_config.include(grammar_dir);

    // On wasm32-unknown-* there is no libc. The `tree-sitter-language` crate
    // ships a mini sysroot and exposes its location via cargo `links` metadata
    // as DEP_TREE_SITTER_LANGUAGE_WASM_{HEADERS,SRC}. Pull those in so the C
    // parser/scanner can find <stdlib.h> etc. and link.
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.starts_with("wasm32-unknown") {
        // The default system compiler (notably Apple clang) often has no
        // WebAssembly backend, so cc-rs fails with "No available targets are
        // compatible with triple wasm32-unknown-unknown". wasm is LLVM-only
        // (GCC has no wasm backend), so best-effort locate an LLVM clang that
        // actually supports the target — unless the user already told us which
        // compiler to use, in which case we never override their choice.
        if let Some(clang) = wasm_clang_if_needed(&c_config) {
            c_config.compiler(&clang);
        }

        if let (Ok(headers), Ok(wasm_src)) = (
            std::env::var("DEP_TREE_SITTER_LANGUAGE_WASM_HEADERS"),
            std::env::var("DEP_TREE_SITTER_LANGUAGE_WASM_SRC"),
        ) {
            // We need the sysroot *headers* so `scanner.c`/`parser.c` can find
            // <stdlib.h> etc. We do NOT, by default, compile the sysroot
            // *sources*: the upstream `tree-sitter` runtime crate (a direct
            // dependency of this crate) already compiles the very same
            // `tree-sitter-language` wasm shim and exports `__assert_fail`,
            // `malloc`, `memcpy`, … Compiling our own copy too makes the wasm
            // linker fail with `duplicate symbol: __assert_fail` whenever both
            // rlibs are linked into one artifact (e.g. a Leptos hydrate bundle).
            //
            // The libc symbols therefore come from the tree-sitter runtime; we
            // only borrow its headers. Set
            // ORG_ELEMENT_COMPILE_WASM_SYSROOT=1 to compile the shim here
            // anyway, for the rare standalone build that links the grammar
            // without the tree-sitter runtime crate.
            c_config.include(&headers);

            // The shim's <assert.h> defines `__assert_fail` *in the header*
            // (not `static`/`inline`), so every TU that includes it mints its
            // own external copy. Our `scanner.c` includes it, which collides
            // with the identical symbol the tree-sitter runtime already
            // exports. Defining NDEBUG makes `assert()` expand to `((void)0)`
            // and emits no `__assert_fail` from our objects at all.
            c_config.define("NDEBUG", None);

            if std::env::var_os("ORG_ELEMENT_COMPILE_WASM_SYSROOT").is_some() {
                // Compile the bundled libc shim sources in a SEPARATE build with
                // warnings suppressed: it is third-party sysroot code and newer
                // clang promotes some of its warnings (e.g. incompatible-pointer-
                // types) to hard errors that `-w` alone does not downgrade.
                let src = std::path::Path::new(&wasm_src);
                let mut sysroot = cc::Build::new();
                sysroot
                    .include(&headers)
                    .warnings(false)
                    .flag_if_supported("-Wno-error=incompatible-pointer-types")
                    .flag_if_supported("-Wno-incompatible-pointer-types");
                if let Some(clang) = wasm_clang_if_needed(&sysroot) {
                    sysroot.compiler(&clang);
                }
                for f in ["stdlib.c", "stdio.c", "string.c"] {
                    sysroot.file(src.join(f));
                }
                sysroot.compile("ts_wasm_sysroot");
            }
        }
        println!("cargo:rerun-if-env-changed=ORG_ELEMENT_COMPILE_WASM_SYSROOT");
    }

    let parser_path = grammar_dir.join("parser.c");
    c_config.file(&parser_path);
    println!("cargo:rerun-if-changed={}", parser_path.display());

    let scanner_path = grammar_dir.join("scanner.c");
    c_config.file(&scanner_path);
    println!("cargo:rerun-if-changed={}", scanner_path.display());

    c_config.compile("tree_sitter_org");
}

/// Best-effort locate an LLVM `clang` that can target `wasm32-unknown-unknown`.
///
/// Returns `Some(path)` only when an override is needed AND a wasm-capable
/// clang was found. Returns `None` when:
/// - the user already set an explicit compiler (`CC_wasm32_unknown_unknown`,
///   `CC_wasm32-unknown-unknown`, or `CC`) — we never override their choice;
/// - the default compiler cc-rs would use already supports wasm;
/// - or nothing suitable was found (a `warning:` is printed in that case).
fn wasm_clang_if_needed(build: &cc::Build) -> Option<String> {
    // 0. The wasm-clang auto-detection (which shells out to `brew` and probes
    //    the filesystem) is opt-in via the `wasm` feature. Without it we never
    //    probe and never override the compiler — a user who does not need wasm
    //    and has no wasm-capable clang is completely unaffected. They get the
    //    standard tree-sitter behaviour: set CC_wasm32_unknown_unknown yourself.
    if std::env::var_os("CARGO_FEATURE_WASM").is_none() {
        return None;
    }

    // 1. Respect an explicit user override; do not second-guess it.
    for var in [
        "CC_wasm32_unknown_unknown",
        "CC_wasm32-unknown-unknown",
        "CC",
    ] {
        if std::env::var_os(var).is_some() {
            return None;
        }
    }

    // 2. If the compiler cc-rs already picked supports wasm, leave it alone.
    if let Ok(tool) = build.try_get_compiler() {
        if compiler_has_wasm(tool.path()) {
            return None;
        }
    }

    // 3. Probe likely LLVM clang locations and PATH; verify each really has
    //    the wasm32 target before accepting it.
    let mut candidates: Vec<String> = Vec::new();
    if let Some(c) = std::env::var_os("CLANG") {
        candidates.push(c.to_string_lossy().into_owned());
    }
    if let Some(prefix) = brew_llvm_prefix() {
        candidates.push(format!("{prefix}/bin/clang"));
    }
    candidates.extend(
        [
            "/opt/homebrew/opt/llvm/bin/clang", // Homebrew, Apple Silicon
            "/usr/local/opt/llvm/bin/clang",    // Homebrew, Intel mac
            "clang",                            // whatever is on PATH
        ]
        .map(String::from),
    );

    for cand in candidates {
        if compiler_has_wasm(std::path::Path::new(&cand)) {
            println!(
                "cargo:warning=org-element: using `{cand}` to compile the \
                 tree-sitter grammar for wasm32-unknown-unknown (the default \
                 compiler lacks a wasm backend). Set CC_wasm32_unknown_unknown \
                 to override."
            );
            return Some(cand);
        }
    }

    println!(
        "cargo:warning=org-element: no clang with a wasm32-unknown-unknown \
         backend was found (Apple's /usr/bin/clang has none; wasm is LLVM-only). \
         If the build fails with \"unable to create target: 'No available \
         targets are compatible with triple wasm32-unknown-unknown'\", install \
         LLVM (`brew install llvm`) and set, in your project's .cargo/config.toml \
         [env]: CC_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/clang and \
         AR_wasm32_unknown_unknown=/opt/homebrew/opt/llvm/bin/llvm-ar"
    );
    None
}

/// True if invoking `clang --print-targets` lists a `wasm32` target.
fn compiler_has_wasm(clang: &std::path::Path) -> bool {
    std::process::Command::new(clang)
        .arg("--print-targets")
        .output()
        .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).contains("wasm32"))
        .unwrap_or(false)
}

/// `brew --prefix llvm`, if Homebrew is installed and the formula is present.
fn brew_llvm_prefix() -> Option<String> {
    let out = std::process::Command::new("brew")
        .args(["--prefix", "llvm"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let p = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if p.is_empty() { None } else { Some(p) }
}
