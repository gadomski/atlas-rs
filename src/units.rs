//! Light wrappers around values to enforce correct units.

/// Celsius degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Celsius(pub f32);

/// Millibar (pressure).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Millibar(pub f32);

/// A percentage, usually between zero and one hundred.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Percentage(pub f32);

/// A percentage represented as a value between zero and five (logic level voltages).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrionPercentage(pub f32);

/// Volts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Volt(pub f32);

/// Kilobytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Kilobyte(pub f32);

/// Meters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Meter(pub f32);

/// Degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Degree(pub f32);
