use std::fmt;

use serde::Serialize;
use serde_repr::Deserialize_repr;

/// Dive computer device family.
#[repr(u32)]
#[derive(
    Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize_repr, Default, Hash, Ord, PartialOrd,
)]
#[non_exhaustive]
pub enum Family {
    #[default]
    None = 0,

    // Suunto
    SuuntoSolution = 1 << 16,
    SuuntoEon,
    SuuntoVyper,
    SuuntoVyper2,
    SuuntoD9,
    SuuntoEonSteel,

    // Reefnet
    ReefnetSensus = 2 << 16,
    ReefnetSensusPro,
    ReefnetSensusUltra,

    // Uwatec
    UwatecAladin = 3 << 16,
    UwatecMemoMouse,
    UwatecSmart,
    UwatecMeridian,
    UwatecG2,

    // Oceanic
    OceanicVtPro = 4 << 16,
    OceanicVeo250,
    OceanicAtom2,

    // Mares
    MaresNemo = 5 << 16,
    MaresPuck,
    MaresDarwin,
    MaresIconHD,

    // Heinrichs Weikamp
    HwOstc = 6 << 16,
    HwFrog,
    HwOstc3,

    // Cressi
    CressiEdy = 7 << 16,
    CressiLeonardo,
    CressiGoa,

    // Zeagle
    ZeagleN2ition3 = 8 << 16,

    // Atomic Aquatics
    AtomicsCobalt = 9 << 16,

    // Shearwater
    ShearwaterPredator = 10 << 16,
    ShearwaterPetrel,

    // Dive Rite
    DiveRiteNitekQ = 11 << 16,

    // Citizen
    CitizenAqualand = 12 << 16,

    // DiveSystem
    DiveSystemIDive = 13 << 16,

    // Cochran
    CochranCommander = 14 << 16,

    // Tecdiving
    TecdivingDivecomputerEu = 15 << 16,

    // McLean
    McLeanExtreme = 16 << 16,

    // Liquivision
    LiquivisionLynx = 17 << 16,

    // Sporasub
    SporasubSp2 = 18 << 16,

    // Deep Six
    DeepSixExcursion = 19 << 16,

    // Seac Screen
    SeacScreen = 20 << 16,

    // Deepblu Cosmiq
    DeepbluCosmiq = 21 << 16,

    // Oceans S1
    OceansS1 = 22 << 16,

    // Divesoft Freedom
    DivesoftFreedom = 23 << 16,

    // Halcyon
    HalcyonSymbios = 24 << 16,
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Family::None => "None",
            Family::SuuntoSolution => "Suunto Solution",
            Family::SuuntoEon => "Suunto Eon",
            Family::SuuntoVyper => "Suunto Vyper",
            Family::SuuntoVyper2 => "Suunto Vyper 2",
            Family::SuuntoD9 => "Suunto D9",
            Family::SuuntoEonSteel => "Suunto Eon Steel",
            Family::ReefnetSensus => "Reefnet Sensus",
            Family::ReefnetSensusPro => "Reefnet Sensus Pro",
            Family::ReefnetSensusUltra => "Reefnet Sensus Ultra",
            Family::UwatecAladin => "Uwatec Aladin",
            Family::UwatecMemoMouse => "Uwatec Memo Mouse",
            Family::UwatecSmart => "Uwatec Smart",
            Family::UwatecMeridian => "Uwatec Meridian",
            Family::UwatecG2 => "Uwatec G2",
            Family::OceanicVtPro => "Oceanic Vt Pro",
            Family::OceanicVeo250 => "Oceanic Veo 250",
            Family::OceanicAtom2 => "Oceanic Atom 2",
            Family::MaresNemo => "Mares Nemo",
            Family::MaresPuck => "Mares Puck",
            Family::MaresDarwin => "Mares Darwin",
            Family::MaresIconHD => "Mares Icon HD",
            Family::HwOstc => "HW OSTC",
            Family::HwFrog => "HW Frog",
            Family::HwOstc3 => "HW OSTC 3",
            Family::CressiEdy => "Cressi Edy",
            Family::CressiLeonardo => "Cressi Leonardo",
            Family::CressiGoa => "Cressi Goa",
            Family::ZeagleN2ition3 => "Zeagle N2ition 3",
            Family::AtomicsCobalt => "Atomics Cobalt",
            Family::ShearwaterPredator => "Shearwater Predator",
            Family::ShearwaterPetrel => "Shearwater Petrel",
            Family::DiveRiteNitekQ => "Dive Rite Nitek Q",
            Family::CitizenAqualand => "Citizen Aqualand",
            Family::DiveSystemIDive => "DiveSystem iDive",
            Family::CochranCommander => "Cochran Commander",
            Family::TecdivingDivecomputerEu => "Tecdiving DivecomputerEU",
            Family::McLeanExtreme => "McLean Extreme",
            Family::LiquivisionLynx => "Liquivision Lynx",
            Family::SporasubSp2 => "Sporasub SP2",
            Family::DeepSixExcursion => "Deep Six Excursion",
            Family::SeacScreen => "Seac Screen",
            Family::DeepbluCosmiq => "Deepblu Cosmiq",
            Family::OceansS1 => "Oceans S1",
            Family::DivesoftFreedom => "Divesoft Freedom",
            Family::HalcyonSymbios => "Halcyon Symbios",
        };
        write!(f, "{s}")
    }
}

impl From<u32> for Family {
    fn from(value: u32) -> Self {
        match value {
            0x00010000 => Family::SuuntoSolution,
            0x00010001 => Family::SuuntoEon,
            0x00010002 => Family::SuuntoVyper,
            0x00010003 => Family::SuuntoVyper2,
            0x00010004 => Family::SuuntoD9,
            0x00010005 => Family::SuuntoEonSteel,
            0x00020000 => Family::ReefnetSensus,
            0x00020001 => Family::ReefnetSensusPro,
            0x00020002 => Family::ReefnetSensusUltra,
            0x00030000 => Family::UwatecAladin,
            0x00030001 => Family::UwatecMemoMouse,
            0x00030002 => Family::UwatecSmart,
            0x00030003 => Family::UwatecMeridian,
            0x00030004 => Family::UwatecG2,
            0x00040000 => Family::OceanicVtPro,
            0x00040001 => Family::OceanicVeo250,
            0x00040002 => Family::OceanicAtom2,
            0x00050000 => Family::MaresNemo,
            0x00050001 => Family::MaresPuck,
            0x00050002 => Family::MaresDarwin,
            0x00050003 => Family::MaresIconHD,
            0x00060000 => Family::HwOstc,
            0x00060001 => Family::HwFrog,
            0x00060002 => Family::HwOstc3,
            0x00070000 => Family::CressiEdy,
            0x00070001 => Family::CressiLeonardo,
            0x00070002 => Family::CressiGoa,
            0x00080000 => Family::ZeagleN2ition3,
            0x00090000 => Family::AtomicsCobalt,
            0x000A0000 => Family::ShearwaterPredator,
            0x000A0001 => Family::ShearwaterPetrel,
            0x000B0000 => Family::DiveRiteNitekQ,
            0x000C0000 => Family::CitizenAqualand,
            0x000D0000 => Family::DiveSystemIDive,
            0x000E0000 => Family::CochranCommander,
            0x000F0000 => Family::TecdivingDivecomputerEu,
            0x00100000 => Family::McLeanExtreme,
            0x00110000 => Family::LiquivisionLynx,
            0x00120000 => Family::SporasubSp2,
            0x00130000 => Family::DeepSixExcursion,
            0x00140000 => Family::SeacScreen,
            0x00150000 => Family::DeepbluCosmiq,
            0x00160000 => Family::OceansS1,
            0x00170000 => Family::DivesoftFreedom,
            0x00180000 => Family::HalcyonSymbios,
            _ => Family::None,
        }
    }
}

impl From<&str> for Family {
    fn from(s: &str) -> Self {
        match s {
            "Suunto Solution" => Family::SuuntoSolution,
            "Suunto Eon" => Family::SuuntoEon,
            "Suunto Vyper" => Family::SuuntoVyper,
            "Suunto Vyper 2" => Family::SuuntoVyper2,
            "Suunto D9" => Family::SuuntoD9,
            "Suunto Eon Steel" => Family::SuuntoEonSteel,
            "Reefnet Sensus" => Family::ReefnetSensus,
            "Reefnet Sensus Pro" => Family::ReefnetSensusPro,
            "Reefnet Sensus Ultra" => Family::ReefnetSensusUltra,
            "Uwatec Aladin" => Family::UwatecAladin,
            "Uwatec Memo Mouse" => Family::UwatecMemoMouse,
            "Uwatec Smart" => Family::UwatecSmart,
            "Uwatec Meridian" => Family::UwatecMeridian,
            "Uwatec G2" => Family::UwatecG2,
            "Oceanic Vt Pro" => Family::OceanicVtPro,
            "Oceanic Veo 250" => Family::OceanicVeo250,
            "Oceanic Atom 2" => Family::OceanicAtom2,
            "Mares Nemo" => Family::MaresNemo,
            "Mares Puck" => Family::MaresPuck,
            "Mares Darwin" => Family::MaresDarwin,
            "Mares Icon HD" => Family::MaresIconHD,
            "HW OSTC" => Family::HwOstc,
            "HW Frog" => Family::HwFrog,
            "HW OSTC 3" => Family::HwOstc3,
            "Cressi Edy" => Family::CressiEdy,
            "Cressi Leonardo" => Family::CressiLeonardo,
            "Cressi Goa" => Family::CressiGoa,
            "Zeagle N2ition 3" => Family::ZeagleN2ition3,
            "Atomics Cobalt" => Family::AtomicsCobalt,
            "Shearwater Predator" => Family::ShearwaterPredator,
            "Shearwater Petrel" => Family::ShearwaterPetrel,
            "Dive Rite Nitek Q" => Family::DiveRiteNitekQ,
            "Citizen Aqualand" => Family::CitizenAqualand,
            "DiveSystem iDive" => Family::DiveSystemIDive,
            "Cochran Commander" => Family::CochranCommander,
            "Tecdiving DivecomputerEU" => Family::TecdivingDivecomputerEu,
            "McLean Extreme" => Family::McLeanExtreme,
            "Liquivision Lynx" => Family::LiquivisionLynx,
            "Sporasub SP2" => Family::SporasubSp2,
            "Deep Six Excursion" => Family::DeepSixExcursion,
            "Seac Screen" => Family::SeacScreen,
            "Deepblu Cosmiq" => Family::DeepbluCosmiq,
            "Oceans S1" => Family::OceansS1,
            "Divesoft Freedom" => Family::DivesoftFreedom,
            "Halcyon Symbios" => Family::HalcyonSymbios,
            _ => Family::None,
        }
    }
}

impl From<&String> for Family {
    fn from(s: &String) -> Self {
        Self::from(s.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_u32_known() {
        assert_eq!(Family::from(0x00010000u32), Family::SuuntoSolution);
        assert_eq!(Family::from(0x000A0001u32), Family::ShearwaterPetrel);
        assert_eq!(Family::from(0x00060002u32), Family::HwOstc3);
        assert_eq!(Family::from(0x00180000u32), Family::HalcyonSymbios);
    }

    #[test]
    fn from_u32_unknown() {
        assert_eq!(Family::from(0xFFFFFFFFu32), Family::None);
        assert_eq!(Family::from(0u32), Family::None);
    }

    #[test]
    fn from_str_known() {
        assert_eq!(Family::from("Suunto Solution"), Family::SuuntoSolution);
        assert_eq!(Family::from("Shearwater Petrel"), Family::ShearwaterPetrel);
        assert_eq!(Family::from("HW OSTC 3"), Family::HwOstc3);
        assert_eq!(Family::from("Halcyon Symbios"), Family::HalcyonSymbios);
    }

    #[test]
    fn from_str_unknown() {
        assert_eq!(Family::from("Nonexistent Family"), Family::None);
    }

    #[test]
    fn from_ref_string() {
        let s = String::from("Suunto D9");
        assert_eq!(Family::from(&s), Family::SuuntoD9);
    }

    #[test]
    fn display_formatting() {
        assert_eq!(Family::None.to_string(), "None");
        assert_eq!(Family::SuuntoEonSteel.to_string(), "Suunto Eon Steel");
        assert_eq!(Family::HwOstc.to_string(), "HW OSTC");
        assert_eq!(
            Family::ShearwaterPredator.to_string(),
            "Shearwater Predator"
        );
    }

    #[test]
    fn display_round_trip_for_from_str() {
        // Display output should round-trip through From<&str>
        let families = [
            Family::SuuntoSolution,
            Family::ShearwaterPetrel,
            Family::HwOstc3,
            Family::CressiLeonardo,
        ];
        for f in families {
            let displayed = f.to_string();
            assert_eq!(Family::from(displayed.as_str()), f);
        }
    }
}
