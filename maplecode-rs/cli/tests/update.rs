use anyhow::Result;
use predicates::str::contains;
use std::path::Path;
use tempfile::TempDir;

fn maplecode_command(maplecode_home: &Path) -> Result<assert_cmd::Command> {
    let mut cmd = assert_cmd::Command::new(maplecode_utils_cargo_bin::cargo_bin("maplecode")?);
    cmd.env("MAPLECODE_HOME", maplecode_home);
    Ok(cmd)
}

#[cfg(debug_assertions)]
#[tokio::test]
async fn update_does_not_start_interactive_prompt() -> Result<()> {
    let maplecode_home = TempDir::new()?;

    maplecode_command(maplecode_home.path())?
        .arg("update")
        .assert()
        .failure()
        .stderr(contains(
            "`maplecode update` is not available in debug builds",
        ));

    Ok(())
}
