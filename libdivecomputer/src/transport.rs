use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::error::LibError;

/// Transport types supported by libdivecomputer.
#[repr(u32)]
#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Hash, Ord, PartialOrd)]
#[non_exhaustive]
pub enum Transport {
    Serial = 1 << 0,
    Usb = 1 << 1,
    UsbHid = 1 << 2,
    Irda = 1 << 3,
    Bluetooth = 1 << 4,
    Ble = 1 << 5,
    UsbStorage = 1 << 6,
}

impl fmt::Display for Transport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Serial => "Serial",
            Self::Usb => "USB",
            Self::UsbHid => "USB HID",
            Self::Irda => "IrDA",
            Self::Bluetooth => "Bluetooth",
            Self::Ble => "BLE",
            Self::UsbStorage => "USB Storage",
        };
        write!(f, "{s}")
    }
}

impl FromStr for Transport {
    type Err = LibError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "Serial" | "serial" => Ok(Self::Serial),
            "USB" | "usb" => Ok(Self::Usb),
            "USB HID" | "usb-hid" | "usb_hid" => Ok(Self::UsbHid),
            "IrDA" | "irda" => Ok(Self::Irda),
            "Bluetooth" | "bluetooth" => Ok(Self::Bluetooth),
            "BLE" | "ble" => Ok(Self::Ble),
            "USB Storage" | "usb-storage" | "usb_storage" => Ok(Self::UsbStorage),
            _ => Err(LibError::InvalidArguments(format!(
                "unknown transport: '{s}'"
            ))),
        }
    }
}

/// A set of transport flags, decoded from a C bitfield.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportSet {
    bits: u32,
}

impl TransportSet {
    /// Decode a bitfield from the C library into a `TransportSet`.
    pub fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    /// Check if a specific transport is present.
    pub fn contains(&self, transport: Transport) -> bool {
        self.bits & (transport as u32) != 0
    }

    /// Return all transports present as a Vec.
    pub fn to_vec(&self) -> Vec<Transport> {
        let all = [
            Transport::Serial,
            Transport::Usb,
            Transport::UsbHid,
            Transport::Irda,
            Transport::Bluetooth,
            Transport::Ble,
            Transport::UsbStorage,
        ];
        all.iter().filter(|t| self.contains(**t)).copied().collect()
    }

    /// Raw bits.
    pub fn bits(&self) -> u32 {
        self.bits
    }
}

impl From<u32> for TransportSet {
    fn from(bits: u32) -> Self {
        Self::from_bits(bits)
    }
}

impl IntoIterator for TransportSet {
    type Item = Transport;
    type IntoIter = std::vec::IntoIter<Transport>;

    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

impl IntoIterator for &TransportSet {
    type Item = Transport;
    type IntoIter = std::vec::IntoIter<Transport>;

    fn into_iter(self) -> Self::IntoIter {
        self.to_vec().into_iter()
    }
}

impl fmt::Display for TransportSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<String> = self.to_vec().iter().map(|t| t.to_string()).collect();
        write!(f, "{}", names.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_str_valid_variants() {
        assert_eq!("Serial".parse::<Transport>().unwrap(), Transport::Serial);
        assert_eq!("serial".parse::<Transport>().unwrap(), Transport::Serial);
        assert_eq!("BLE".parse::<Transport>().unwrap(), Transport::Ble);
        assert_eq!("ble".parse::<Transport>().unwrap(), Transport::Ble);
        assert_eq!("USB HID".parse::<Transport>().unwrap(), Transport::UsbHid);
        assert_eq!("usb-hid".parse::<Transport>().unwrap(), Transport::UsbHid);
        assert_eq!("usb_hid".parse::<Transport>().unwrap(), Transport::UsbHid);
        assert_eq!("USB".parse::<Transport>().unwrap(), Transport::Usb);
        assert_eq!("usb".parse::<Transport>().unwrap(), Transport::Usb);
        assert_eq!("IrDA".parse::<Transport>().unwrap(), Transport::Irda);
        assert_eq!("irda".parse::<Transport>().unwrap(), Transport::Irda);
        assert_eq!(
            "Bluetooth".parse::<Transport>().unwrap(),
            Transport::Bluetooth
        );
        assert_eq!(
            "bluetooth".parse::<Transport>().unwrap(),
            Transport::Bluetooth
        );
        assert_eq!(
            "USB Storage".parse::<Transport>().unwrap(),
            Transport::UsbStorage
        );
        assert_eq!(
            "usb-storage".parse::<Transport>().unwrap(),
            Transport::UsbStorage
        );
        assert_eq!(
            "usb_storage".parse::<Transport>().unwrap(),
            Transport::UsbStorage
        );
    }

    #[test]
    fn from_str_invalid() {
        let err = "nonsense".parse::<Transport>().unwrap_err();
        assert!(matches!(err, LibError::InvalidArguments(_)));
    }

    #[test]
    fn display_round_trip() {
        let all = [
            Transport::Serial,
            Transport::Usb,
            Transport::UsbHid,
            Transport::Irda,
            Transport::Bluetooth,
            Transport::Ble,
            Transport::UsbStorage,
        ];
        for t in all {
            let s = t.to_string();
            let parsed: Transport = s.parse().unwrap();
            assert_eq!(parsed, t);
        }
    }

    #[test]
    fn transport_set_contains_and_to_vec() {
        let set = TransportSet::from_bits(Transport::Serial as u32 | Transport::Ble as u32);
        assert!(set.contains(Transport::Serial));
        assert!(set.contains(Transport::Ble));
        assert!(!set.contains(Transport::Usb));
        assert!(!set.contains(Transport::Bluetooth));

        let vec = set.to_vec();
        assert_eq!(vec, vec![Transport::Serial, Transport::Ble]);
    }

    #[test]
    fn transport_set_empty() {
        let set = TransportSet::from_bits(0);
        assert!(set.to_vec().is_empty());
        assert!(!set.contains(Transport::Serial));
    }

    #[test]
    fn transport_set_into_iterator() {
        let set = TransportSet::from_bits(Transport::Usb as u32 | Transport::Irda as u32);
        let collected: Vec<Transport> = set.into_iter().collect();
        assert_eq!(collected, vec![Transport::Usb, Transport::Irda]);
    }

    #[test]
    fn transport_set_ref_into_iterator() {
        let set = TransportSet::from_bits(Transport::Serial as u32);
        let collected: Vec<Transport> = (&set).into_iter().collect();
        assert_eq!(collected, vec![Transport::Serial]);
    }

    #[test]
    fn transport_set_display() {
        let set = TransportSet::from_bits(Transport::Serial as u32 | Transport::Ble as u32);
        assert_eq!(set.to_string(), "Serial, BLE");

        let empty = TransportSet::from_bits(0);
        assert_eq!(empty.to_string(), "");
    }

    #[test]
    fn transport_set_from_u32() {
        let set: TransportSet = (Transport::Usb as u32).into();
        assert!(set.contains(Transport::Usb));
        assert_eq!(set.bits(), Transport::Usb as u32);
    }
}
