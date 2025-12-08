//! HDMI device filtering logic

/// Keywords that identify HDMI audio devices
const HDMI_KEYWORDS: &[&str] = &[
    "hdmi",
    "nvidia high definition audio",
    "intel(r) display audio",
    "amd high definition audio",
    "display audio",
];

/// Filter for identifying HDMI audio devices
pub struct HdmiFilter;

impl HdmiFilter {
    /// Check if a device name indicates an HDMI device
    pub fn is_hdmi_device(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        HDMI_KEYWORDS
            .iter()
            .any(|keyword| name_lower.contains(keyword))
    }

    /// Check if a device ID indicates an HDMI device
    /// Device IDs sometimes contain hints about the device type
    pub fn is_hdmi_device_id(device_id: &str) -> bool {
        let id_lower = device_id.to_lowercase();
        id_lower.contains("hdmi") || id_lower.contains("display")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hdmi_detection() {
        assert!(HdmiFilter::is_hdmi_device("NVIDIA High Definition Audio"));
        assert!(HdmiFilter::is_hdmi_device("Intel(R) Display Audio"));
        assert!(HdmiFilter::is_hdmi_device(
            "AMD High Definition Audio Device"
        ));
        assert!(HdmiFilter::is_hdmi_device("HDMI Output"));
        assert!(!HdmiFilter::is_hdmi_device("Realtek Audio"));
        assert!(!HdmiFilter::is_hdmi_device("Speakers"));
    }
}
