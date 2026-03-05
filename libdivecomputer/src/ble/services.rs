use uuid::{Uuid, uuid};

/// Known BLE service UUIDs for dive computer brands.
pub const KNOWN_SERVICES: &[(Uuid, &str)] = &[
    (
        uuid!("0000fefb-0000-1000-8000-00805f9b34fb"),
        "Heinrichs-Weikamp (Telit/Stollmann)",
    ),
    (
        uuid!("2456e1b9-26e2-8f83-e744-f34f01e9d701"),
        "Heinrichs-Weikamp (U-Blox)",
    ),
    (
        uuid!("544e326b-5b72-c6b0-1c46-41c1bc448118"),
        "Mares BlueLink Pro",
    ),
    (
        uuid!("98ae7120-e62e-11e3-badd-0002a5d5c51b"),
        "Suunto (EON Steel/Core, G5)",
    ),
    (
        uuid!("cb3c4555-d670-4670-bc20-b61dbc851e9a"),
        "Pelagic (i770R, i200C, Pro Plus X, Geo 4.0)",
    ),
    (
        uuid!("ca7b0001-f785-4c38-b599-c7c5fbadb034"),
        "Pelagic (i330R, DSX)",
    ),
    (
        uuid!("fdcdeaaa-295d-470e-bf15-04217b7aa0a0"),
        "ScubaPro (G2, G3)",
    ),
    (
        uuid!("fe25c237-0ece-443c-b0aa-e02033e7029d"),
        "Shearwater (Perdix/Teric/Peregrine/Tern)",
    ),
    (uuid!("0000fcef-0000-1000-8000-00805f9b34fb"), "Divesoft"),
    (uuid!("6e400001-b5a3-f393-e0a9-e50e24dc10b8"), "Cressi"),
    (
        uuid!("6e400001-b5a3-f393-e0a9-e50e24dcca9e"),
        "Nordic Semi UART",
    ),
    (
        uuid!("00000001-8c3b-4f2c-a59e-8c08224f3253"),
        "Halcyon Symbios",
    ),
];
