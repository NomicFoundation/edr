mod collector;

use edr_eth::{Address, address};

pub use self::collector::CoverageHitCollector;

pub const COVERAGE_ADDRESS: Address = address!("0xc0bEc0BEc0BeC0bEC0beC0bEC0bEC0beC0beC0BE");
