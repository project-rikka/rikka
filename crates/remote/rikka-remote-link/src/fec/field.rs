const POLYNOMIAL: u16 = 0x11D;

struct Tables {
    exp: [u8; 512],
    log: [u8; 256],
}

const fn tables() -> Tables {
    let mut exp = [0_u8; 512];
    let mut log = [0_u8; 256];
    let mut value: u16 = 1;
    let mut power = 0;
    while power < 255 {
        exp[power] = value as u8;
        log[value as usize] = power as u8;
        value <<= 1;
        if value & 0x100 != 0 {
            value ^= POLYNOMIAL;
        }
        power += 1;
    }
    let mut index = 255;
    while index < 512 {
        exp[index] = exp[index - 255];
        index += 1;
    }
    Tables { exp, log }
}

const TABLES: Tables = tables();

pub(crate) const fn mul(a: u8, b: u8) -> u8 {
    if a == 0 || b == 0 {
        return 0;
    }
    TABLES.exp[TABLES.log[a as usize] as usize + TABLES.log[b as usize] as usize]
}

pub(crate) const fn inv(a: u8) -> u8 {
    TABLES.exp[255 - TABLES.log[a as usize] as usize]
}

pub(crate) struct Nibbles {
    pub(crate) low: [[u8; 16]; 256],
    pub(crate) high: [[u8; 16]; 256],
}

const fn nibbles() -> Nibbles {
    let mut low = [[0_u8; 16]; 256];
    let mut high = [[0_u8; 16]; 256];
    let mut coefficient = 0;
    while coefficient < 256 {
        let mut nibble = 0;
        while nibble < 16 {
            low[coefficient][nibble] = mul(coefficient as u8, nibble as u8);
            high[coefficient][nibble] = mul(coefficient as u8, (nibble << 4) as u8);
            nibble += 1;
        }
        coefficient += 1;
    }
    Nibbles { low, high }
}

pub(crate) static NIBBLES: Nibbles = nibbles();
