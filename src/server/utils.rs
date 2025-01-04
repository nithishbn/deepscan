pub(crate) struct Normalizer {
    pub(crate) max_abs: f64,
}

impl Normalizer {
    // Normalize a single value and return its corresponding color
    pub fn get_color(&self, value: f64) -> String {
        if self.max_abs == 0.0 {
            return "rgb(255,255,255)".to_string(); // Handle case where max_abs is 0
        }

        let normalized = value / self.max_abs;

        if normalized < 0.0 {
            let intensity = ((1.0 + normalized) * 255.0).round() as u8; // Scale [-1, 0] to [0, 255]
            format!("rgb(255,{},{})", intensity, intensity) // Shades of red
        } else {
            let intensity = ((1.0 - normalized) * 255.0).round() as u8; // Scale [0, 1] to [255, 0]
            format!("rgb({},{},255)", intensity, intensity) // Shades of blue
        }
    }
}
