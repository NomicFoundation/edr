mod collector;
/// Types for code coverage reporting.
pub mod reporter;

use edr_primitives::{address, Address};

pub use self::{collector::CoverageHitCollector, reporter::CodeCoverageReporter};

pub const COVERAGE_ADDRESS: Address = address!("0xc0bEc0BEc0BeC0bEC0beC0bEC0bEC0beC0beC0BE");
