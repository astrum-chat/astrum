fn main() {
    println!("cargo::rerun-if-env-changed=MACOSX_DEPLOYMENT_TARGET");
    println!("cargo::rustc-check-cfg=cfg(HAS_LIQUID_GLASS_WINDOW)");

    let macos_version = get_macos_version();
    if macos_version >= 26 {
        println!("cargo::rustc-cfg=HAS_LIQUID_GLASS_WINDOW");
    }
}

fn get_macos_version() -> u32 {
    if let Some(v) = parse_version(&std::env::var("MACOSX_DEPLOYMENT_TARGET").ok()) {
        return v;
    }

    #[cfg(target_os = "macos")]
    if let Some(v) = infer_macos_version() {
        return v;
    }

    0
}

fn parse_version(version: &Option<String>) -> Option<u32> {
    version.as_ref()?.split('.').next()?.parse().ok()
}

#[cfg(target_os = "macos")]
fn infer_macos_version() -> Option<u32> {
    let output = std::process::Command::new("sw_vers")
        .arg("-productVersion")
        .output()
        .ok()?;
    let version = String::from_utf8(output.stdout).ok()?;
    version.trim().split('.').next()?.parse().ok()
}
