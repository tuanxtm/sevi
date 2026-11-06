//! MT19937 and the legacy Python integer-seed/getrandbits conventions.
#[derive(Clone)]
pub struct Mt19937 {
    state: [u32; 624],
    index: usize,
}
impl Mt19937 {
    pub fn new(seed: u32) -> Self {
        let mut s = Self {
            state: [0; 624],
            index: 624,
        };
        s.state[0] = seed;
        for i in 1..624 {
            s.state[i] = 1812433253u32
                .wrapping_mul(s.state[i - 1] ^ (s.state[i - 1] >> 30))
                .wrapping_add(i as u32)
        }
        s
    }
    pub fn python(seed: u64) -> Self {
        let mut s = Self::new(19650218);
        let keys = if seed > u32::MAX as u64 {
            vec![seed as u32, (seed >> 32) as u32]
        } else {
            vec![seed as u32]
        };
        let (mut i, mut j) = (1, 0);
        for _ in 0..624.max(keys.len()) {
            s.state[i] = (s.state[i]
                ^ ((s.state[i - 1] ^ (s.state[i - 1] >> 30)).wrapping_mul(1664525)))
            .wrapping_add(keys[j])
            .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= 624 {
                s.state[0] = s.state[623];
                i = 1
            }
            if j >= keys.len() {
                j = 0
            }
        }
        for _ in 0..623 {
            s.state[i] = (s.state[i]
                ^ ((s.state[i - 1] ^ (s.state[i - 1] >> 30)).wrapping_mul(1566083941)))
            .wrapping_sub(i as u32);
            i += 1;
            if i >= 624 {
                s.state[0] = s.state[623];
                i = 1
            }
        }
        s.state[0] = 0x80000000;
        s
    }
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= 624 {
            for i in 0..624 {
                let y = (self.state[i] & 0x80000000) | (self.state[(i + 1) % 624] & 0x7fffffff);
                self.state[i] =
                    self.state[(i + 397) % 624] ^ (y >> 1) ^ if y & 1 != 0 { 0x9908b0df } else { 0 }
            }
            self.index = 0
        }
        let mut y = self.state[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c5680;
        y ^= (y << 15) & 0xefc60000;
        y ^= y >> 18;
        y
    }
    pub fn random(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        ((a as u64 * 67108864 + b as u64) as f64) / 9007199254740992.0
    }
    pub fn below(&mut self, n: usize) -> usize {
        assert!(n > 0 && n <= u32::MAX as usize);
        let k = 32 - (n as u32).leading_zeros();
        loop {
            let value = self.next_u32() >> (32 - k);
            if (value as usize) < n {
                return value as usize;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn python_seed_42() {
        let mut r = Mt19937::python(42);
        assert_eq!(r.random(), 0.6394267984578837);
        assert_eq!(r.random(), 0.025010755222666936);
        assert_eq!(r.random(), 0.27502931836911926);
    }
    #[test]
    fn standard_seed() {
        let mut r = Mt19937::new(5489);
        assert_eq!(r.next_u32(), 3499211612);
        assert_eq!(r.next_u32(), 581869302);
    }
}
