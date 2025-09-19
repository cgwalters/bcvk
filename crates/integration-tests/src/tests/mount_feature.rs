//! Integration tests for mount features
//!
//! ⚠️  **CRITICAL INTEGRATION TEST POLICY** ⚠️
//!
//! INTEGRATION TESTS MUST NEVER "warn and continue" ON FAILURES!
//!
//! If something is not working:
//! - Use `todo!("reason why this doesn't work yet")`
//! - Use `panic!("clear error message")`
//! - Use `assert!()` and `unwrap()` to fail hard
//!
//! NEVER use patterns like:
//! - "Note: test failed - likely due to..."
//! - "This is acceptable in CI/testing environments"
//! - Warning and continuing on failures

use camino::Utf8Path;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

use crate::{get_bck_command, get_test_image, INTEGRATION_TEST_LABEL};

/// Create a systemd unit that verifies a mount exists and contains expected content
fn create_mount_verify_unit(
    unit_dir: &Utf8Path,
    mount_name: &str,
    expected_file: &str,
    expected_content: &str,
) -> std::io::Result<()> {
    let unit_content = format!(
        r#"[Unit]
Description=Verify mount {mount_name} and poweroff
RequiresMountsFor=/run/virtiofs-mnt-{mount_name}

[Service]
Type=oneshot
ExecStart=grep -qF "{expected_content}" /run/virtiofs-mnt-{mount_name}/{expected_file}
ExecStart=test -w /run/virtiofs-mnt-{mount_name}/{expected_file}
ExecStart=echo ok mount verify {mount_name}
ExecStart=systemctl poweroff
StandardOutput=journal+console
StandardError=journal+console
"#
    );

    let unit_path = unit_dir.join(format!("verify-mount-{}.service", mount_name));
    fs::write(&unit_path, unit_content)?;
    Ok(())
}

/// Create a systemd unit that tries to write to a mount to verify read-only status
fn create_ro_mount_verify_unit(
    unit_dir: &Utf8Path,
    mount_name: &str,
    expected_file: &str,
) -> std::io::Result<()> {
    let unit_content = format!(
        r#"[Unit]
Description=Verify read-only mount {mount_name} and poweroff
RequiresMountsFor=/run/virtiofs-mnt-{mount_name}

[Service]
Type=oneshot
ExecStart=test -f /run/virtiofs-mnt-{mount_name}/{expected_file}
ExecStart=test '!' -w /run/virtiofs-mnt-{mount_name}/{expected_file}
ExecStart=echo ok mount verify {mount_name}
ExecStart=systemctl poweroff
StandardOutput=journal+console
StandardError=journal+console
"#
    );

    let unit_path = unit_dir.join(format!("verify-ro-mount-{}.service", mount_name));
    fs::write(&unit_path, unit_content)?;
    Ok(())
}

pub fn test_mount_feature_bind() {
    let bck = get_bck_command().unwrap();

    // Create a temporary directory to test bind mounting
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_dir_path = Utf8Path::from_path(temp_dir.path()).expect("temp dir path is not utf8");
    let test_file_path = temp_dir_path.join("test.txt");
    let test_content = "Test content for bind mount";
    fs::write(&test_file_path, test_content).expect("Failed to write test file");

    // Create systemd units directory
    let units_dir = TempDir::new().expect("Failed to create units directory");
    let units_dir_path = Utf8Path::from_path(units_dir.path()).expect("units dir path is not utf8");
    let system_dir = units_dir_path.join("system");
    fs::create_dir(&system_dir).expect("Failed to create system directory");

    // Create verification unit
    create_mount_verify_unit(&system_dir, "testmount", "test.txt", test_content)
        .expect("Failed to create verify unit");

    println!("Testing bind mount with temp directory: {}", temp_dir_path);

    // Run with bind mount and verification unit
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--console",
            "-K",
            "--bind",
            &format!("{}:testmount", temp_dir_path),
            "--systemd-units",
            units_dir_path.as_str(),
            "--karg",
            "systemd.unit=verify-mount-testmount.service",
            "--karg",
            "systemd.journald.forward_to_console=1",
            &get_test_image(),
        ])
        .output()
        .expect("Failed to run bcvk with bind mount");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok mount verify"));

    println!("Successfully tested and verified bind mount feature");
}

pub fn test_mount_feature_ro_bind() {
    let bck = get_bck_command().unwrap();

    // Create a temporary directory to test read-only bind mounting
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let temp_dir_path = Utf8Path::from_path(temp_dir.path()).expect("temp dir path is not utf8");
    let test_file_path = temp_dir_path.join("readonly.txt");
    fs::write(&test_file_path, "Read-only content").expect("Failed to write test file");

    // Create systemd units directory
    let units_dir = TempDir::new().expect("Failed to create units directory");
    let units_dir_path = Utf8Path::from_path(units_dir.path()).expect("units dir path is not utf8");
    let system_dir = units_dir_path.join("system");
    fs::create_dir(&system_dir).expect("Failed to create system directory");

    // Create verification unit for read-only mount
    create_ro_mount_verify_unit(&system_dir, "romount", "readonly.txt")
        .expect("Failed to create verify unit");

    println!(
        "Testing read-only bind mount with temp directory: {}",
        temp_dir_path
    );

    // Run with read-only bind mount and verification unit
    let output = Command::new("timeout")
        .args([
            "60s",
            &bck,
            "ephemeral",
            "run",
            "--rm",
            "--label",
            INTEGRATION_TEST_LABEL,
            "--console",
            "-K",
            "--ro-bind",
            &format!("{}:romount", temp_dir_path),
            "--systemd-units",
            units_dir_path.as_str(),
            "--karg",
            "systemd.unit=verify-ro-mount-romount.service",
            "--karg",
            "systemd.journald.forward_to_console=1",
            &get_test_image(),
        ])
        .output()
        .expect("Failed to run bcvk with ro-bind mount");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ok mount verify"));
}
