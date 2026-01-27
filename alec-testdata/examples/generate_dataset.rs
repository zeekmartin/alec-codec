//! Example: Generate datasets for different industries.
//!
//! Run with: cargo run --example generate_dataset

use alec_testdata::{
    generate_dataset, Dataset, DatasetManifest, GeneratorConfig, SensorManifest,
};
use alec_testdata::industries::{
    agriculture::{create_farm_sensors, AgriculturalScenario},
    satellite::{create_satellite_sensors, SatelliteScenario},
    manufacturing::{create_factory_sensors, ManufacturingScenario},
    smart_city::{create_city_sensors, SmartCityScenario},
    logistics::{create_logistics_sensors, LogisticsScenario},
    energy::{create_grid_sensors, EnergyScenario},
};

fn main() {
    println!("ALEC Testdata Generator");
    println!("=======================\n");

    // Generate agriculture datasets
    generate_agriculture_datasets();

    // Generate satellite datasets
    generate_satellite_datasets();

    // Generate manufacturing datasets
    generate_manufacturing_datasets();

    // Generate smart city datasets
    generate_smart_city_datasets();

    // Generate logistics datasets
    generate_logistics_datasets();

    // Generate energy datasets
    generate_energy_datasets();

    println!("\nAll datasets generated successfully!");
}

fn generate_agriculture_datasets() {
    println!("Generating Agriculture datasets...");

    let scenarios = [
        (AgriculturalScenario::Normal, "farm_normal_24h", 24.0),
        (AgriculturalScenario::Drought, "farm_drought_event", 24.0),
        (AgriculturalScenario::SensorFailure, "farm_sensor_failure", 24.0),
        (AgriculturalScenario::IrrigationCycle, "farm_irrigation_cycle", 8.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(60)
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_farm_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("agriculture")
            .with_description(&format!("{:?} scenario", scenario));

        // Save CSV
        let csv_path = format!("datasets/agriculture/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        // Create and save manifest
        let manifest = create_manifest(&dataset, &sensors, name, "agriculture");
        let manifest_path = format!("datasets/agriculture/{}.manifest.json", name);
        if let Err(e) = manifest.to_json_file(&manifest_path) {
            eprintln!("  Warning: Could not save manifest: {}", e);
        }
    }
}

fn generate_satellite_datasets() {
    println!("Generating Satellite datasets...");

    let scenarios = [
        (SatelliteScenario::Stationary, "satellite_stationary_24h", 24.0),
        (SatelliteScenario::MovingAsset, "satellite_moving_asset_8h", 8.0),
        (SatelliteScenario::BatteryCritical, "satellite_battery_critical", 12.0),
        (SatelliteScenario::SignalLoss, "satellite_signal_loss", 6.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(600) // 10 min for satellite
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_satellite_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("satellite");

        let csv_path = format!("datasets/satellite/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        let manifest = create_manifest(&dataset, &sensors, name, "satellite");
        let _ = manifest.to_json_file(format!("datasets/satellite/{}.manifest.json", name));
    }
}

fn generate_manufacturing_datasets() {
    println!("Generating Manufacturing datasets...");

    let scenarios = [
        (ManufacturingScenario::NormalShift, "factory_normal_shift_8h", 8.0),
        (ManufacturingScenario::MachineCycle, "factory_machine_cycle_1h", 1.0),
        (ManufacturingScenario::BearingFailure, "factory_bearing_failure", 4.0),
        (ManufacturingScenario::LeakEvent, "factory_leak_event", 4.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(1) // 1 sec for manufacturing
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_factory_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("manufacturing");

        let csv_path = format!("datasets/manufacturing/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        let manifest = create_manifest(&dataset, &sensors, name, "manufacturing");
        let _ = manifest.to_json_file(format!("datasets/manufacturing/{}.manifest.json", name));
    }
}

fn generate_smart_city_datasets() {
    println!("Generating Smart City datasets...");

    let scenarios = [
        (SmartCityScenario::Weekday, "city_weekday_24h", 24.0),
        (SmartCityScenario::Weekend, "city_weekend_24h", 24.0),
        (SmartCityScenario::Accident, "city_accident_event", 24.0),
        (SmartCityScenario::Festival, "city_festival_event", 24.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(60)
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_city_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("smart_city");

        let csv_path = format!("datasets/smart_city/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        let manifest = create_manifest(&dataset, &sensors, name, "smart_city");
        let _ = manifest.to_json_file(format!("datasets/smart_city/{}.manifest.json", name));
    }
}

fn generate_logistics_datasets() {
    println!("Generating Logistics datasets...");

    let scenarios = [
        (LogisticsScenario::NormalRoute, "delivery_route_4h", 4.0),
        (LogisticsScenario::MultiStop, "delivery_multi_stop_8h", 8.0),
        (LogisticsScenario::ColdChainBreach, "cold_chain_breach", 6.0),
        (LogisticsScenario::RefrigerationFailure, "refrigeration_failure", 4.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(60)
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_logistics_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("logistics");

        let csv_path = format!("datasets/logistics/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        let manifest = create_manifest(&dataset, &sensors, name, "logistics");
        let _ = manifest.to_json_file(format!("datasets/logistics/{}.manifest.json", name));
    }
}

fn generate_energy_datasets() {
    println!("Generating Energy datasets...");

    let scenarios = [
        (EnergyScenario::Normal, "grid_normal_24h", 24.0),
        (EnergyScenario::IndustrialLoad, "grid_industrial_load", 24.0),
        (EnergyScenario::PhaseImbalance, "grid_phase_imbalance", 12.0),
        (EnergyScenario::HarmonicEvent, "grid_harmonic_event", 12.0),
    ];

    for (scenario, name, hours) in scenarios {
        let config = GeneratorConfig::new()
            .with_sample_interval_secs(1)
            .with_duration_hours(hours)
            .with_seed(42);

        let sensors = create_grid_sensors(scenario);
        let dataset = generate_dataset(&config, &sensors)
            .with_name(name)
            .with_industry("energy");

        let csv_path = format!("datasets/energy/{}.csv", name);
        if let Err(e) = dataset.to_csv(&csv_path) {
            eprintln!("  Warning: Could not save {}: {}", csv_path, e);
        } else {
            println!("  Created {}", csv_path);
        }

        let manifest = create_manifest(&dataset, &sensors, name, "energy");
        let _ = manifest.to_json_file(format!("datasets/energy/{}.manifest.json", name));
    }
}

fn create_manifest(
    dataset: &Dataset,
    sensors: &[alec_testdata::SensorConfig],
    name: &str,
    industry: &str,
) -> DatasetManifest {
    let mut manifest = DatasetManifest::new(name, industry)
        .with_description(&dataset.metadata.description.clone().unwrap_or_default())
        .with_timing(
            dataset.duration_ms(),
            dataset.len(),
            dataset.metadata.sample_interval_ms.unwrap_or(60_000),
        )
        .with_seed(42);

    for sensor in sensors {
        if let Some(stats) = dataset.stats(&sensor.id) {
            manifest = manifest.add_sensor(
                SensorManifest::new(
                    &sensor.id,
                    &sensor.unit,
                    stats.min,
                    stats.max,
                    "auto-detected",
                )
            );
        }
    }

    manifest
}
