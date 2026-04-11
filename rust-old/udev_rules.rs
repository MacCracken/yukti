//! Udev rule management — create, validate, install, and reload rules.
//!
//! With the `udev-rules` feature enabled, delegates to agnosys for rule
//! rendering, validation, filesystem operations, and udevadm reload.
//! Without it, all functions return [`YuktiError`] or empty results.
//!
//! This complements yukti's raw kernel udev module (netlink hotplug,
//! sysfs enumeration) with userland rule management via `udevadm`.

use crate::error::{Result, YuktiError};
use std::path::{Path, PathBuf};

/// A udev rule definition.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UdevRule {
    /// Rule file name (without `.rules` extension).
    pub name: String,
    /// Match conditions as key-value pairs (e.g., `("SUBSYSTEM", "usb")`, `("ATTR{idVendor}", "1234")`).
    pub match_attrs: Vec<(String, String)>,
    /// Actions as key-value pairs (e.g., `("MODE", "0666")`, `("SYMLINK+", "mydevice")`).
    pub actions: Vec<(String, String)>,
}

/// Device info from udevadm query.
#[derive(Debug, Clone)]
pub struct UdevDeviceInfo {
    pub syspath: String,
    pub devpath: String,
    pub subsystem: String,
    pub devtype: Option<String>,
    pub driver: Option<String>,
    pub devnode: Option<String>,
}

/// Render a udev rule to its `.rules` file format.
#[must_use]
pub fn render_rule(rule: &UdevRule) -> String {
    #[cfg(feature = "udev-rules")]
    {
        let agnosys_rule = agnosys::udev::UdevRule {
            name: rule.name.clone(),
            match_attrs: rule.match_attrs.clone(),
            actions: rule.actions.clone(),
        };
        agnosys::udev::render_udev_rule(&agnosys_rule)
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let mut parts: Vec<String> = rule
            .match_attrs
            .iter()
            .map(|(k, v)| format!("{k}==\"{v}\""))
            .collect();
        parts.extend(rule.actions.iter().map(|(k, v)| format!("{k}=\"{v}\"")));
        parts.join(", ")
    }
}

/// Validate a udev rule for correctness and safety.
///
/// Checks for empty name, dangerous action keys (`RUN`, `PROGRAM`, `IMPORT`),
/// and valid match/action key formats.
pub fn validate_rule(rule: &UdevRule) -> Result<()> {
    #[cfg(feature = "udev-rules")]
    {
        if rule.actions.is_empty() {
            return Err(YuktiError::Udev(
                "rule must have at least one action".into(),
            ));
        }
        let agnosys_rule = agnosys::udev::UdevRule {
            name: rule.name.clone(),
            match_attrs: rule.match_attrs.clone(),
            actions: rule.actions.clone(),
        };
        agnosys::udev::validate_rule(&agnosys_rule)
            .map_err(|e| YuktiError::Udev(format!("rule validation failed: {e}")))
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        if rule.name.is_empty() {
            return Err(YuktiError::Udev("rule name cannot be empty".into()));
        }
        if rule.match_attrs.is_empty() {
            return Err(YuktiError::Udev("rule must have at least one match".into()));
        }
        if rule.actions.is_empty() {
            return Err(YuktiError::Udev(
                "rule must have at least one action".into(),
            ));
        }
        Ok(())
    }
}

/// Write a udev rule to the rules directory.
///
/// Requires the `udev-rules` feature and root privileges.
pub fn write_rule(rule: &UdevRule, rules_dir: &Path) -> Result<PathBuf> {
    #[cfg(feature = "udev-rules")]
    {
        let agnosys_rule = agnosys::udev::UdevRule {
            name: rule.name.clone(),
            match_attrs: rule.match_attrs.clone(),
            actions: rule.actions.clone(),
        };
        agnosys::udev::write_udev_rule(&agnosys_rule, rules_dir)
            .map_err(|e| YuktiError::Udev(format!("failed to write rule: {e}")))
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let _ = (rule, rules_dir);
        Err(YuktiError::Udev(
            "rule management requires the 'udev-rules' feature".into(),
        ))
    }
}

/// Remove a udev rule by name from the rules directory.
///
/// Requires the `udev-rules` feature and root privileges.
pub fn remove_rule(name: &str, rules_dir: &Path) -> Result<()> {
    #[cfg(feature = "udev-rules")]
    {
        agnosys::udev::remove_udev_rule(name, rules_dir)
            .map_err(|e| YuktiError::Udev(format!("failed to remove rule: {e}")))
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let _ = (name, rules_dir);
        Err(YuktiError::Udev(
            "rule management requires the 'udev-rules' feature".into(),
        ))
    }
}

/// Reload udev rules via `udevadm control --reload-rules`.
///
/// Requires the `udev-rules` feature and root privileges.
pub fn reload_rules() -> Result<()> {
    #[cfg(feature = "udev-rules")]
    {
        agnosys::udev::reload_udev_rules()
            .map_err(|e| YuktiError::Udev(format!("failed to reload rules: {e}")))
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        Err(YuktiError::Udev(
            "rule management requires the 'udev-rules' feature".into(),
        ))
    }
}

/// Trigger a device re-evaluation via `udevadm trigger`.
///
/// Requires the `udev-rules` feature.
pub fn trigger_device(syspath: &str) -> Result<()> {
    #[cfg(feature = "udev-rules")]
    {
        agnosys::udev::trigger_device(syspath)
            .map_err(|e| YuktiError::Udev(format!("failed to trigger device: {e}")))
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let _ = syspath;
        Err(YuktiError::Udev(
            "rule management requires the 'udev-rules' feature".into(),
        ))
    }
}

/// Query device info via `udevadm info`.
///
/// Requires the `udev-rules` feature. Without it, returns an error.
pub fn query_device(syspath: &str) -> Result<UdevDeviceInfo> {
    #[cfg(feature = "udev-rules")]
    {
        let info = agnosys::udev::get_device_info(syspath)
            .map_err(|e| YuktiError::Udev(format!("device query failed: {e}")))?;
        Ok(UdevDeviceInfo {
            syspath: info.syspath,
            devpath: info.devpath,
            subsystem: info.subsystem,
            devtype: info.devtype,
            driver: info.driver,
            devnode: info.devnode,
        })
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let _ = syspath;
        Err(YuktiError::Udev(
            "device query requires the 'udev-rules' feature".into(),
        ))
    }
}

/// List devices via `udevadm info --export-db`, optionally filtered by subsystem.
///
/// Requires the `udev-rules` feature. Without it, returns an empty vec.
#[must_use]
pub fn list_devices(subsystem: Option<&str>) -> Vec<UdevDeviceInfo> {
    #[cfg(feature = "udev-rules")]
    {
        agnosys::udev::list_devices(subsystem)
            .unwrap_or_default()
            .into_iter()
            .map(|info| UdevDeviceInfo {
                syspath: info.syspath,
                devpath: info.devpath,
                subsystem: info.subsystem,
                devtype: info.devtype,
                driver: info.driver,
                devnode: info.devnode,
            })
            .collect()
    }
    #[cfg(not(feature = "udev-rules"))]
    {
        let _ = subsystem;
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_rule() -> UdevRule {
        UdevRule {
            name: "99-yukti-test".into(),
            match_attrs: vec![
                ("SUBSYSTEM".into(), "usb".into()),
                ("ATTR{idVendor}".into(), "1234".into()),
            ],
            actions: vec![("MODE".into(), "0666".into())],
        }
    }

    #[test]
    fn render_rule_produces_output() {
        let rendered = render_rule(&test_rule());
        assert!(!rendered.is_empty());
        assert!(rendered.contains("SUBSYSTEM"));
        assert!(rendered.contains("usb"));
    }

    #[test]
    fn validate_rule_good() {
        assert!(validate_rule(&test_rule()).is_ok());
    }

    #[test]
    fn validate_rule_empty_name() {
        let mut rule = test_rule();
        rule.name = String::new();
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn validate_rule_no_matches() {
        let mut rule = test_rule();
        rule.match_attrs.clear();
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn validate_rule_no_actions() {
        let mut rule = test_rule();
        rule.actions.clear();
        assert!(validate_rule(&rule).is_err());
    }

    #[test]
    fn write_rule_nonexistent_dir() {
        let result = write_rule(&test_rule(), Path::new("/nonexistent/rules.d"));
        assert!(result.is_err());
    }

    #[test]
    fn remove_rule_nonexistent_dir() {
        let result = remove_rule("test", Path::new("/nonexistent/rules.d"));
        assert!(result.is_err());
    }

    #[test]
    fn trigger_device_nonexistent() {
        let result = trigger_device("/sys/devices/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn query_device_nonexistent() {
        let result = query_device("/sys/devices/nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn list_devices_returns_vec() {
        let devices = list_devices(None);
        let _ = devices; // may be empty in CI
    }

    #[test]
    fn udev_rule_serde_roundtrip() {
        let rule = test_rule();
        let json = serde_json::to_string(&rule).unwrap();
        let back: UdevRule = serde_json::from_str(&json).unwrap();
        assert_eq!(back.name, "99-yukti-test");
        assert_eq!(back.match_attrs.len(), 2);
        assert_eq!(back.actions.len(), 1);
    }

    #[test]
    fn udev_rule_debug() {
        let rule = test_rule();
        let dbg = format!("{:?}", rule);
        assert!(dbg.contains("yukti-test"));
    }

    #[test]
    fn udev_device_info_debug() {
        let info = UdevDeviceInfo {
            syspath: "/sys/devices/pci0000:00".into(),
            devpath: "/devices/pci0000:00".into(),
            subsystem: "pci".into(),
            devtype: None,
            driver: Some("pcieport".into()),
            devnode: None,
        };
        let dbg = format!("{:?}", info);
        assert!(dbg.contains("pci"));
    }
}
