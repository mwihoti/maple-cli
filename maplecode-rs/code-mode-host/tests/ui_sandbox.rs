#[test]
fn plugin_arguments_are_parsed_before_startup() -> Result<(), Box<dyn std::error::Error>> {
    let program = maplecode_utils_cargo_bin::cargo_bin("maplecode-code-mode-host")?;
    for arguments in [
        ["--help", "--maplecode-ui-plugin-process"],
        ["--maplecode-ui-plugin-process", "--help"],
    ] {
        let output = maplecode_utils_process::background_command(&program)
            .args(arguments)
            .output()?;
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout)?.contains("Usage:"));
    }
    for arguments in [
        ["--unknown", "--maplecode-ui-plugin-process"],
        ["--maplecode-ui-plugin-process", "--unknown"],
        ["--listen=stdio", "--maplecode-ui-plugin-process"],
        ["--maplecode-ui-plugin-process", "--listen=stdio"],
    ] {
        let output = maplecode_utils_process::background_command(&program)
            .args(arguments)
            .output()?;
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8(output.stderr)?.contains("error:"));
    }
    Ok(())
}
