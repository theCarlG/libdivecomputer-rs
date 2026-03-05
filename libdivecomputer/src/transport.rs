use std::fmt;

use serde::{Deserialize, Serialize};

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

impl From<&str> for Transport {
    fn from(s: &str) -> Self {
        match s {
            "Serial" => Self::Serial,
            "USB" => Self::Usb,
            "USB HID" => Self::UsbHid,
            "IrDA" => Self::Irda,
            "Bluetooth" => Self::Bluetooth,
            "BLE" => Self::Ble,
            "USB Storage" => Self::UsbStorage,
            _ => Self::Serial, // fallback
        }
    }
}

impl From<&String> for Transport {
    fn from(value: &String) -> Self {
        Self::from(value.as_str())
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
