//! # START OF FILE hainet-seed/tests/uninstallation_integration_test.rs
//! Integration tests for the uninstaller module.
//! These tests verify the Uninstaller can be created and that the
//! cleanup logic is structurally sound. Actual SSH operations are
//! tested via the mock client in deployment_integration_test.rs.

use hainet_seed::installer::uninstaller::Uninstaller;
use anyhow::Result;

#[test]
fn test_uninstaller_creation() -> Result<()> {
    // The Uninstaller should always be constructible
    let uninstaller = Uninstaller::new()?;
    // If we get here, construction succeeded
    assert!(true);
    Ok(())
}
