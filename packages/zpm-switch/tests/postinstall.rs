use assert_cmd::prelude::*; // Add methods on commands
use predicates::prelude::*;
use zpm_utils::{FromFileString, Path}; // Used for writing assertions
use std::{process::{Command, Stdio}, str::FromStr}; // Run programs

struct TestEnv {
    cmd: Command,
    bin_dir: Path,
    tmp_dir: Path,
}

fn init_test_env() -> TestEnv {
    let cmd
        = Command::cargo_bin("yarn")
            .expect("Failed to get yarn command");

    let bin_dir
        = Path::from_str(cmd.get_program().to_str().unwrap())
            .ok()
            .and_then(|p| p.dirname())
            .expect("Failed to get bin dir");

    let tmp_dir
        = Path::temp_dir()
            .expect("Failed to create temp dir");
    
    TestEnv {
        cmd,
        bin_dir,
        tmp_dir,
    }
}

#[test]
fn bashrc_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = tmp_dir
        .with_join_str(".bashrc")
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n"
    ].join(""));

    Ok(())
}

#[test]
fn zshrc_empty_file() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/zsh");

    cmd.assert()
        .success();

    let bashrc_content = tmp_dir
        .with_join_str(".zshrc")
        .fs_read_text_prealloc()
        .expect("Failed to read .zshrc");

    assert_eq!(bashrc_content, vec![
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n"
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_header_only() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n# BEGIN YARN SWITCH MANAGED BLOCK\nWorld\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
        "World\n"
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_footer_only() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n# END YARN SWITCH MANAGED BLOCK\nWorld\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
        "World\n"
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_outdated() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n# BEGIN YARN SWITCH MANAGED BLOCK\nSomething something\n# END YARN SWITCH MANAGED BLOCK\nWorld\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
        "World\n"
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_end_of_file_nlnl() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_end_of_file_nl() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
    ].join(""));

    Ok(())
}

#[test]
fn bashrc_end_of_file_nonl() -> Result<(), Box<dyn std::error::Error>> {
    let TestEnv {
        mut cmd,
        bin_dir,
        tmp_dir,
    } = init_test_env();

    let bashrc_path = tmp_dir
        .with_join_str(".bashrc");

    bashrc_path
        .fs_write_text("Hello\n\n")
        .expect("Failed to write .bashrc");

    cmd.args(vec!["switch", "postinstall", "--home-dir", tmp_dir.as_str()]);
    cmd.env("SHELL", "/bin/bash");

    cmd.assert()
        .success();

    let bashrc_content = bashrc_path
        .fs_read_text_prealloc()
        .expect("Failed to read .bashrc");

    assert_eq!(bashrc_content, vec![
        "Hello\n",
        "\n",
        "# BEGIN YARN SWITCH MANAGED BLOCK\n",
        "export PATH=\"", &bin_dir.to_path_buf().to_string_lossy(), ":$PATH\"\n",
        "# END YARN SWITCH MANAGED BLOCK\n",
    ].join(""));

    Ok(())
}
