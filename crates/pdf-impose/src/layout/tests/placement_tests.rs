use super::*;

#[test]
fn test_scaling_modes() {
    // Test fit mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fit);
    assert!((scale - 0.5).abs() < 0.001, "Fit should use smaller scale");

    // Test fill mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::Fill);
    assert!(
        (scale - 400.0 / 600.0).abs() < 0.001,
        "Fill should use larger scale"
    );

    // Test none mode
    let scale = calculate_scale(800.0, 600.0, 400.0, 400.0, ScalingMode::None);
    assert!((scale - 1.0).abs() < 0.001, "None should return 1.0");
}
