use serde::{Deserialize, Serialize};

pub(crate) struct Normalizer {
    pub(crate) max_abs: f64,
}

impl Normalizer {
    // Normalize a single value and return the RGB color
    pub fn get_color_rgb(&self, value: f64) -> String {
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

    // Normalize a single value and return the Hex color
    pub fn get_color_hex(&self, value: f64) -> String {
        if self.max_abs == 0.0 {
            return "#FFFFFF".to_string(); // Handle case where max_abs is 0
        }

        let normalized = value / self.max_abs;

        if normalized < 0.0 {
            let intensity = ((1.0 + normalized) * 255.0).round() as u8; // Scale [-1, 0] to [0, 255]
            format!("#FF{:02X}{:02X}", intensity, intensity) // Shades of red in hex
        } else {
            let intensity = ((1.0 - normalized) * 255.0).round() as u8; // Scale [0, 1] to [255, 0]
            format!("#{:02X}{:02X}FF", intensity, intensity) // Shades of blue in hex
        }
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct PosColor {
    pub pos: i32,
    pub color: String, // Assuming `color` is a String or any other type.
}
