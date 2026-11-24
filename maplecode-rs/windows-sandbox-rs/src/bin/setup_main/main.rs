#[cfg(target_os = "windows")]
fn main() -> anyhow::Result<()> {
    maplecode_windows_sandbox::setup_helper_main()
}

#[cfg(not(target_os = "windows"))]
fn main() {
    panic!("maplecode-windows-sandbox-setup is Windows-only");
}
