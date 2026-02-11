/// Speed of sound in air (m/s) and density (kg/m³) as a function of
/// temperature in °C. Uses the ideal-gas approximation.
pub fn speed_of_sound_and_density(temperature_c: f64) -> (f64, f64) {
    let t_kelvin = temperature_c + 273.15;
    // c = 331.3 * sqrt(T/273.15)
    let c = 331.3 * (t_kelvin / 273.15).sqrt();
    // ρ = p / (R_specific * T), with p = 101325 Pa, R_specific = 287.05 J/(kg·K)
    let rho = 101325.0 / (287.05 * t_kelvin);
    (c, rho)
}

/// Cross-sectional area from diameter (both in metres).
pub fn area_from_diameter(diameter: f64) -> f64 {
    std::f64::consts::PI * (diameter / 2.0).powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speed_of_sound_at_20c() {
        let (c, rho) = speed_of_sound_and_density(20.0);
        assert!((c - 343.2).abs() < 0.5, "c = {c}");
        assert!((rho - 1.204).abs() < 0.01, "rho = {rho}");
    }
}
