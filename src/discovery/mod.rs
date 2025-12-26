#[cfg(not(feature = "test_no_external_deps"))]
pub mod manager;

#[cfg(feature = "test_no_external_deps")]
pub mod manager_test;

#[cfg(feature = "test_no_external_deps")]
pub use manager_test::*;

#[cfg(not(feature = "test_no_external_deps"))]
pub use manager::*;
