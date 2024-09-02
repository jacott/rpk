use rpk_common::{
    globals,
    keycodes::key_range::{LAYER_MAX, LAYER_MIN, MACROS_MAX, MACROS_MIN},
    mouse::{MouseAnalogSetting, MouseConfig},
    PROTOCOL_VERSION,
};

use crate::transformer::{macros::Macro, KeyPlusMod};

pub const MAIN_BASE: u16 = 5;

pub struct Manager<const ROWS: usize, const COLS: usize, const CODE_SIZE: usize> {
    mapping: [u16; CODE_SIZE],
    globals: Globals,
    mouse_profiles: [MouseConfig; 3],
    layout_bottom: usize,
    layout_top: usize,
    macro_dir_base: usize,
    memo_bottom: usize,
    memo_top: usize,
    macro_stack: usize,
}

#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum LoadError {
    OutOfSpace,
    VersionMismatch,
    RowColMismatch,
    Corrupt,
}

pub(crate) struct Globals {
    pub(crate) values: [u16; 2],
}
impl Default for Globals {
    fn default() -> Self {
        Self { values: [180, 20] }
    }
}

#[derive(Debug)]
pub struct Layer<'l, const ROWS: usize, const COLS: usize>(&'l [u16]);

impl<const ROWS: usize, const COLS: usize> Layer<'_, ROWS, COLS> {
    pub fn get(&self, row: usize, column: usize) -> u16 {
        let slice = &self.0[1..];
        if slice.len() == ROWS * COLS {
            *slice.get(row * COLS + column).unwrap_or(&0u16)
        } else {
            search_code(slice, row, column)
        }
    }

    pub fn modifiers(&self) -> u8 {
        self.0[0] as u8
    }
}

impl<const ROWS: usize, const COLS: usize, const LAYOUT_MAX: usize> Default
    for Manager<ROWS, COLS, LAYOUT_MAX>
{
    fn default() -> Self {
        Self {
            mapping: [0; LAYOUT_MAX],
            globals: Default::default(),
            mouse_profiles: [
                MouseConfig::slow(),
                MouseConfig::normal(),
                MouseConfig::fast(),
            ],
            layout_bottom: 0,
            layout_top: 0,
            macro_dir_base: 0,
            memo_bottom: 0,
            memo_top: 0,
            macro_stack: 0,
        }
    }
}

impl<const ROWS: usize, const COLS: usize, const LAYOUT_MAX: usize>
    Manager<ROWS, COLS, LAYOUT_MAX>
{
    /// Load mapping config into `Manager`. The format of mapping is as follows:
    ///
    /// protocol version
    /// layer count
    /// row_count (high byte), column_count (low byte) should match `ROWS` and `COLS`
    /// layer positions, layers.
    /// Layer positions mark the index in codes where the layers start.
    /// Layers are dense if every entry has a value; size == ROWS * COLS
    /// Layers are sparse if size < ROWS * COLS in which case it is a list of ordered tuples where
    /// the first byte is the row, second is the column and the next word is the value
    pub fn load(&mut self, iter: impl IntoIterator<Item = u16>) -> Result<(), LoadError> {
        let mut iter = iter.into_iter();
        if iter.next().ok_or(LoadError::Corrupt)? != PROTOCOL_VERSION {
            return Err(LoadError::VersionMismatch);
        }
        {
            let n = u16::from_le(iter.next().ok_or(LoadError::Corrupt)?);

            if (n >> 8) as usize != ROWS || (n & 0xff) as usize != COLS {
                return Err(LoadError::RowColMismatch);
            }
        }

        let layer_count = iter.next().ok_or(LoadError::Corrupt)?;
        if layer_count < 6 {
            return Err(LoadError::Corrupt);
        }
        let macros_count = iter.next().ok_or(LoadError::Corrupt)?;
        let mut globals_count = iter.next().ok_or(LoadError::Corrupt)?;

        while globals_count != 0 {
            if globals_count < 2 {
                crate::error!("corrupt layout: globals_count is wrong");
                return Err(LoadError::Corrupt);
            }
            let i = iter.next().ok_or(LoadError::Corrupt)?;
            match i {
                globals::MOUSE_PROFILE1 | globals::MOUSE_PROFILE2 | globals::MOUSE_PROFILE3 => {
                    let mp = self
                        .mouse_profiles
                        .get_mut((i - globals::MOUSE_PROFILE1) as usize)
                        .ok_or(LoadError::Corrupt)?;
                    mp.movement =
                        MouseAnalogSetting::deserialize(&mut iter).ok_or(LoadError::Corrupt)?;
                    mp.scroll =
                        MouseAnalogSetting::deserialize(&mut iter).ok_or(LoadError::Corrupt)?;
                    globals_count -= 21;
                }
                globals::DUAL_ACTION_TIMEOUT | globals::DUAL_ACTION_TIMEOUT2 => {
                    globals_count -= 2;
                    let v = iter.next().ok_or(LoadError::Corrupt)?;
                    *self
                        .globals
                        .values
                        .get_mut(i as usize)
                        .ok_or(LoadError::Corrupt)? = v;
                }
                _ => return Err(LoadError::Corrupt),
            }
        }

        if layer_count >= (LAYER_MAX - LAYER_MIN) || macros_count >= (MACROS_MAX - MACROS_MIN) {
            crate::error!(
                "corrupt layout: layer_count {} or macros_count {} is out-of-range",
                layer_count,
                macros_count
            );
            return Err(LoadError::Corrupt);
        }

        let layer_start = (layer_count + macros_count) as usize;

        let mut i = 0;
        let mut p = 0;
        for (f, t) in iter.take(self.mapping.len()).zip(self.mapping.iter_mut()) {
            let n = u16::from_le(f);
            if i <= layer_start {
                if n > LAYOUT_MAX as u16 {
                    crate::error!("corrupt layout: layer/macro {} index is invalid", i);
                    return Err(LoadError::Corrupt);
                }
                if n <= p {
                    crate::error!("corrupt layout: layer/macro {} index is invalid", i);
                    return Err(LoadError::Corrupt);
                }
                p = n;
            }
            i += 1;
            *t = n;
        }

        if i >= LAYOUT_MAX {
            crate::error!("layout too big: LAYOUT_MAX is {}", LAYOUT_MAX);
            return Err(LoadError::Corrupt);
        }

        self.macro_dir_base = layer_count as usize;

        self.layout_bottom = i;
        self.clear_all();

        Ok(())
    }

    pub(super) fn clear_all(&mut self) {
        self.macro_stack = LAYOUT_MAX;
        self.memo_bottom = LAYOUT_MAX;
        self.memo_top = LAYOUT_MAX;
        self.clear_layers();
        self.set_layout(MAIN_BASE);
    }

    pub(super) fn clear_layers(&mut self) {
        self.layout_top = self.layout_bottom + 1;
    }

    pub(super) fn clear_modifier_layers(&mut self) {
        let layers = &mut self.mapping[self.layout_bottom + 1..=self.layout_top];
        let mut j = 0;

        for i in 0..layers.len() {
            unsafe {
                if layers[i] >= 5 {
                    *layers.get_unchecked_mut(j) = layers[i];
                    j += 1;
                }
            }
        }

        self.layout_top = self.layout_bottom + 1 + j;
    }

    pub fn find_code(&self, row: usize, column: usize) -> Option<KeyPlusMod> {
        for &layer_idx in self.mapping[self.layout_bottom..self.layout_top]
            .iter()
            .rev()
        {
            if let Some(layer) = self.get_layer(layer_idx) {
                let code = layer.get(row, column);
                if code != 0 {
                    return Some(KeyPlusMod::new(code, layer.modifiers()));
                }
            }
        }
        None
    }

    pub fn get_macro(&self, id: u16) -> Macro {
        let idx = id as usize + self.macro_dir_base;
        if idx + 1 >= self.mapping.len() {
            return Macro::Noop;
        }

        let s = self.mapping[idx] as usize;
        let e = self.mapping[idx + 1] as usize;
        if e < s || e > self.mapping.len() {
            return Macro::Noop;
        }

        Macro::decode(s, self.mapping.get(s..e))
    }

    pub fn get_layer(&self, layer_num: u16) -> Option<Layer<'_, ROWS, COLS>> {
        let idx = layer_num as usize;
        if idx + 1 >= self.mapping.len() || idx >= self.macro_dir_base {
            crate::error!("corrupt layout: layer index out of range {}", layer_num);
            return None;
        }

        let s = self.mapping[idx] as usize;
        let e = self.mapping[idx + 1] as usize;
        // todo use first macro rather than mapping.len
        if e < s || e > self.mapping.len() {
            crate::error!("corrupt layout: layer address out of range {}..{}", s, e);
            return None;
        }

        self.mapping
            .get(s..(self.mapping[idx + 1] as usize))
            .map(Layer)
    }

    pub fn macro_stack(&self) -> usize {
        self.macro_stack
    }

    pub fn set_layout(&mut self, n: u16) {
        self.mapping[self.layout_bottom] = n;
    }

    pub fn push_layer(&mut self, n: u16) -> bool {
        if self.layout_top + 1 >= self.macro_stack {
            return false;
        }
        self.mapping[self.layout_top] = n;
        self.layout_top += 1;
        true
    }

    pub fn pop_layer(&mut self, n: u16) -> bool {
        if let Some((i, _)) = self.mapping[self.layout_bottom..self.layout_top]
            .iter()
            .copied()
            .enumerate()
            .rfind(|(_, v)| *v == n)
        {
            self.mapping.copy_within(
                self.layout_bottom + i + 1..self.layout_top,
                self.layout_bottom + i,
            );

            self.layout_top -= 1;
            return true;
        }
        false
    }

    pub(crate) fn macro_code(&self, location: usize) -> u16 {
        self.mapping[location]
    }

    pub(crate) fn update_macro(&mut self, mac: &Macro) {
        mac.update(&mut self.mapping[self.macro_stack..]);
    }

    pub(crate) fn push_macro(&mut self, mac: Macro) -> Macro {
        self.defrag_stack();
        let (mac, len) = mac.push(&mut self.mapping[self.layout_top..self.macro_stack]);
        self.macro_stack -= len;
        mac
    }

    pub(crate) fn pop_macro(&mut self) -> Macro {
        let (mac, len) = Macro::pop(&self.mapping[self.macro_stack..self.memo_bottom]);
        self.macro_stack += len;
        mac
    }

    pub(crate) fn global(&self, index: usize) -> u16 {
        self.globals.values[index]
    }

    pub(crate) fn get_mouse_profile(&self, index: usize) -> Option<&MouseConfig> {
        self.mouse_profiles.get(index)
    }

    pub(crate) fn push_memo(&mut self, memo: &[u16]) -> bool {
        self.defrag_stack();
        if self.macro_stack != self.memo_bottom
            || self.memo_bottom <= memo.len() + 1 + self.layout_top
        {
            false
        } else {
            self.macro_stack -= memo.len() + 1;
            self.mapping[self.macro_stack..self.memo_bottom - 1].copy_from_slice(memo);
            self.mapping[self.memo_bottom - 1] = memo.len() as u16;
            self.memo_bottom = self.macro_stack;
            true
        }
    }

    pub(crate) fn pop_memo(&mut self, receiver: impl FnOnce(&[u16])) -> bool {
        if self.memo_bottom == self.memo_top {
            false
        } else {
            let end = self.memo_top - 1;
            let start = end - self.mapping[end] as usize;
            self.memo_top = start;
            receiver(&self.mapping[start..end]);
            true
        }
    }

    fn defrag_stack(&mut self) {
        let diff = LAYOUT_MAX - self.memo_top;
        if diff == 0 {
            return;
        }
        self.mapping
            .copy_within(self.macro_stack..self.memo_top, self.macro_stack + diff);

        self.macro_stack += diff;
        self.memo_bottom += diff;
        self.memo_top += diff;
    }
}

fn search_code(mut codes: &[u16], row: usize, column: usize) -> u16 {
    let cmp = (row as u16) << 8 | (column as u16);

    let mut s = codes.len();

    loop {
        s = (s >> 1) & !1;
        // len < 2 would work but s avoids the bounds checker
        if codes.len() <= s {
            if codes.len() == 2 && cmp == codes[0] {
                return codes[1];
            }
            return 0;
        }
        let v = codes[s];

        #[allow(clippy::comparison_chain)]
        if cmp < v {
            codes = &codes[..s];
        } else if cmp > v {
            if codes.len() > s + 2 {
                codes = &codes[s + 2..]
            } else {
                return 0;
            }
        } else {
            return if codes.len() > s + 1 { codes[s + 1] } else { 0 };
        }
    }
}

#[cfg(test)]
#[path = "layout_test.rs"]
mod test;
