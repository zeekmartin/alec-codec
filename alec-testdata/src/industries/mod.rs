// ALEC Testdata - Industry modules
// Copyright (c) 2025 David Martin Venti
//
// Dual-licensed under AGPL-3.0 and Commercial License.
// See LICENSE file for details.

//! Industry-specific dataset generators.
//!
//! Each module provides pre-configured sensor sets and scenarios
//! for a specific industry vertical.

pub mod agriculture;
pub mod energy;
pub mod logistics;
pub mod manufacturing;
pub mod satellite;
pub mod smart_city;

pub use agriculture::{create_farm_sensors, AgriculturalScenario};
pub use energy::{create_grid_sensors, EnergyScenario};
pub use logistics::{create_logistics_sensors, LogisticsScenario};
pub use manufacturing::{create_factory_sensors, ManufacturingScenario};
pub use satellite::{create_satellite_sensors, SatelliteScenario};
pub use smart_city::{create_city_sensors, SmartCityScenario};

/// Industry type enumeration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Industry {
    Agriculture,
    Satellite,
    Manufacturing,
    SmartCity,
    Logistics,
    Energy,
}

impl Industry {
    /// Get industry name as string.
    pub fn as_str(&self) -> &'static str {
        match self {
            Industry::Agriculture => "agriculture",
            Industry::Satellite => "satellite",
            Industry::Manufacturing => "manufacturing",
            Industry::SmartCity => "smart_city",
            Industry::Logistics => "logistics",
            Industry::Energy => "energy",
        }
    }

    /// Get display name.
    pub fn display_name(&self) -> &'static str {
        match self {
            Industry::Agriculture => "Agriculture (AgTech)",
            Industry::Satellite => "Satellite IoT",
            Industry::Manufacturing => "Manufacturing (IIoT)",
            Industry::SmartCity => "Smart City",
            Industry::Logistics => "Logistics (Cold Chain)",
            Industry::Energy => "Energy (Smart Grid)",
        }
    }
}
