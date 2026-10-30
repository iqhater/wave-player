fn main() {
    // Windows shows the default Dioxus icon unless the executable carries an
    // ICON resource. Dioxus CLI always injects its own icon/version resource
    // for desktop builds, so embedding a second icon group here would make
    // cvtres fail with CVT1100 (duplicate RT_ICON ids). Only plain cargo
    // builds need the resource.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_none() || built_by_dioxus_cli() {
        return;
    }

    embed_resource::compile(
        "assets/app.rc",
        embed_resource::ParamsIncludeDirs(&["assets"]),
    )
    .manifest_optional()
    .expect("failed to embed Windows icon resource");

    println!("cargo:rerun-if-changed=assets/app.rc");
    println!("cargo:rerun-if-changed=assets/favicon.ico");
}

fn built_by_dioxus_cli() -> bool {
    // `dx` wraps the compiler with itself, which is the only reliable marker
    // available to build scripts.
    std::env::var("RUSTC_WORKSPACE_WRAPPER")
        .ok()
        .and_then(|wrapper| {
            std::path::Path::new(&wrapper)
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(|stem| stem.eq_ignore_ascii_case("dx"))
        })
        .unwrap_or(false)
}
