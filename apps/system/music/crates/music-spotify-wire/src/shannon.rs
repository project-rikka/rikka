const WORDS: usize = 16;
const FOLD: usize = WORDS;
const INITIAL_KONST: u32 = 0x6996_c53a;
const KEY_WORD: usize = 13;
const WORD_BITS: u32 = 32;

#[derive(Clone, Copy)]
enum Direction {
    Encrypt,
    Decrypt,
}

#[derive(Clone)]
pub struct Shannon {
    register: [u32; WORDS],
    crc: [u32; WORDS],
    keyed: [u32; WORDS],
    konst: u32,
    stream: u32,
    partial: u32,
    pending_bits: u32,
}

impl Shannon {
    #[must_use]
    pub fn new(key: &[u8]) -> Shannon {
        let mut register = [0u32; WORDS];
        register[0] = 1;
        register[1] = 1;
        for index in 2..WORDS {
            register[index] = register[index - 1] + register[index - 2];
        }
        let mut cipher = Shannon {
            register,
            crc: [0; WORDS],
            keyed: [0; WORDS],
            konst: INITIAL_KONST,
            stream: 0,
            partial: 0,
            pending_bits: 0,
        };
        cipher.load_key(key);
        cipher.konst = cipher.register[0];
        cipher.keyed = cipher.register;
        cipher
    }

    pub fn set_nonce(&mut self, nonce: u32) {
        self.register = self.keyed;
        self.konst = INITIAL_KONST;
        self.load_key(&nonce.to_be_bytes());
        self.konst = self.register[0];
        self.pending_bits = 0;
    }

    pub fn encrypt(&mut self, bytes: &mut [u8]) {
        self.process(bytes, Direction::Encrypt);
    }

    pub fn decrypt(&mut self, bytes: &mut [u8]) {
        self.process(bytes, Direction::Decrypt);
    }

    pub fn finish(&mut self, mac: &mut [u8]) {
        if self.pending_bits != 0 {
            self.absorb(self.partial);
        }
        self.cycle();
        self.register[KEY_WORD] ^= INITIAL_KONST ^ (self.pending_bits << 3);
        self.pending_bits = 0;
        for (word, crc) in self.register.iter_mut().zip(self.crc) {
            *word ^= crc;
        }
        self.diffuse();
        for chunk in mac.chunks_mut(4) {
            self.cycle();
            let stream = self.stream.to_le_bytes();
            chunk.copy_from_slice(&stream[..chunk.len()]);
        }
    }

    fn process(&mut self, bytes: &mut [u8], direction: Direction) {
        let mut start = 0;
        if self.pending_bits != 0 {
            while self.pending_bits != 0 && start < bytes.len() {
                self.mix_byte(&mut bytes[start], direction);
                start += 1;
            }
            if self.pending_bits != 0 {
                return;
            }
            self.absorb(self.partial);
        }
        let (words, tail) = bytes[start..].as_chunks_mut::<4>();
        for word in words {
            self.cycle();
            let mut value = u32::from_le_bytes(*word);
            match direction {
                Direction::Encrypt => {
                    self.absorb(value);
                    value ^= self.stream;
                }
                Direction::Decrypt => {
                    value ^= self.stream;
                    self.absorb(value);
                }
            }
            *word = value.to_le_bytes();
        }
        if !tail.is_empty() {
            self.cycle();
            self.partial = 0;
            self.pending_bits = WORD_BITS;
            for byte in tail {
                self.mix_byte(byte, direction);
            }
        }
    }

    fn mix_byte(&mut self, byte: &mut u8, direction: Direction) {
        let shift = WORD_BITS - self.pending_bits;
        let keystream = (self.stream >> shift).to_le_bytes()[0];
        match direction {
            Direction::Encrypt => {
                self.partial ^= u32::from(*byte) << shift;
                *byte ^= keystream;
            }
            Direction::Decrypt => {
                *byte ^= keystream;
                self.partial ^= u32::from(*byte) << shift;
            }
        }
        self.pending_bits -= 8;
    }

    fn load_key(&mut self, key: &[u8]) {
        let (words, tail) = key.as_chunks::<4>();
        for word in words {
            self.register[KEY_WORD] ^= u32::from_le_bytes(*word);
            self.cycle();
        }
        if !tail.is_empty() {
            let mut padded = [0u8; 4];
            padded[..tail.len()].copy_from_slice(tail);
            self.register[KEY_WORD] ^= u32::from_le_bytes(padded);
            self.cycle();
        }
        self.register[KEY_WORD] ^= u32::try_from(key.len()).unwrap_or(u32::MAX);
        self.cycle();
        self.crc = self.register;
        self.diffuse();
        for (word, crc) in self.register.iter_mut().zip(self.crc) {
            *word ^= crc;
        }
    }

    fn cycle(&mut self) {
        let feedback = sbox1(self.register[12] ^ self.register[13] ^ self.konst)
            ^ self.register[0].rotate_left(1);
        self.register.copy_within(1.., 0);
        self.register[WORDS - 1] = feedback;
        let mixed = sbox2(self.register[2] ^ self.register[15]);
        self.register[0] ^= mixed;
        self.stream = mixed ^ self.register[8] ^ self.register[12];
    }

    fn diffuse(&mut self) {
        for _ in 0..FOLD {
            self.cycle();
        }
    }

    fn absorb(&mut self, word: u32) {
        let crc = self.crc[0] ^ self.crc[2] ^ self.crc[15] ^ word;
        self.crc.copy_within(1.., 0);
        self.crc[WORDS - 1] = crc;
        self.register[KEY_WORD] ^= word;
    }
}

impl std::fmt::Debug for Shannon {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("Shannon").finish_non_exhaustive()
    }
}

fn sbox1(mut word: u32) -> u32 {
    word ^= word.rotate_left(5) | word.rotate_left(7);
    word ^= word.rotate_left(19) | word.rotate_left(22);
    word
}

fn sbox2(mut word: u32) -> u32 {
    word ^= word.rotate_left(7) | word.rotate_left(22);
    word ^= word.rotate_left(5) | word.rotate_left(19);
    word
}
