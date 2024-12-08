use std::{collections::HashMap, ops::Range, str::CharIndices};

use rpk_common::{
    keycodes::{
        key_range::{self, BASIC_0, BASIC_1, BASIC_A},
        macro_types,
    },
    PROTOCOL_VERSION,
};

use crate::{
    globals::{
        self,
        spec::{GlobalProp, GlobalType},
    },
    keycodes::{self, key_code, unshifted_char_code},
    ConfigError,
};

type Result<T> = core::result::Result<T, ConfigError>;
type IndexChar = (usize, char);
type NameRange = Range<usize>;

const TOO_MANY_RHS: &str = "Only one value may be assigned";
const TOO_MANY_MULTI_ALIAS_RHS: &str =
    "Only one value may be assigned to an multi-positioned alias";
const TOO_MANY_ROWS: &str = "Too many rows";
const TOO_MANY_COLS: &str = "Too many keys in row";
const UNKNOWN_ACTION: &str = "Unknown action/keycode";
const SYNTAX_ERROR: &str = "Syntax error";
const EOF: &str = "Unexpected end of file";

struct SourceIter<'source> {
    iter: CharIndices<'source>,
    current: IndexChar,
    next: Option<IndexChar>,
    len: usize,
}
impl<'source> SourceIter<'source> {
    pub fn new(iter: CharIndices<'source>, len: usize) -> Self {
        Self {
            iter,
            current: (usize::MAX, '\0'),
            next: None,
            len,
        }
    }

    fn put_back(&mut self, item: IndexChar) {
        assert!(self.next.is_none() && item.0 == self.current.0);
        self.next = Some(item);
    }

    fn next(&mut self) -> Option<IndexChar> {
        let mut in_comment = false;
        while let Some(item) = self.next.take().or_else(|| self.iter.next()) {
            match item.1 {
                '\n' => {}
                '#' => {
                    in_comment = true;
                    continue;
                }
                _ => {
                    if in_comment {
                        continue;
                    }
                }
            }
            self.current = item;
            return Some(item);
        }
        self.current.0 = self.len;
        None
    }

    fn find(&mut self, pred: impl Fn(char) -> bool) -> Option<IndexChar> {
        while let Some(item) = self.next() {
            if pred(item.1) {
                return Some(item);
            }
        }
        None
    }

    fn find_close_paren(&mut self, parens: (char, char)) -> Option<IndexChar> {
        let mut count = 1;
        while let Some(item) = self.next() {
            if item.1 == parens.0 {
                count += 1;
            } else if item.1 == parens.1 {
                if count == 1 {
                    return Some(item);
                } else {
                    count -= 1;
                }
            }
        }
        None
    }
}

struct Parser<'source> {
    iter: SourceIter<'source>,
    config: KeyboardConfig<'source>,
    mark_idx: usize,
}

fn non_ws_char(c: char) -> bool {
    !c.is_whitespace()
}

fn invalid_seq_char(c: char) -> bool {
    c.is_whitespace() || matches!(c, '(' | ')')
}

fn invalid_arg_char(c: char) -> bool {
    invalid_seq_char(c) || c == ','
}

fn invalid_section_char(c: char) -> bool {
    !matches!(c, '-' | '_' | '\\' | '.' | ':') && !c.is_alphanumeric()
}

pub struct KeyboardConfig<'source> {
    pub source: &'source str,
    pub global_map: HashMap<&'source str, GlobalProp>,
    pub temp_map: HashMap<&'source str, u16>,
    pub firmware_map: HashMap<&'source str, &'source str>,
    pub matrix_map: HashMap<String, Vec<u16>>,
    layers: HashMap<String, ConfigLayer>,
    macros_names: HashMap<Vec<u16>, u16>,
    macros: Vec<Macro>,
    next_layer: u16,
    pub row_count: u8,
    pub col_count: u8,
}

#[derive(Debug)]
pub struct ConfigLayer {
    codes: HashMap<u16, u16>,
    index: u16,
    suffix: u8,
}

#[derive(Debug, PartialEq)]
pub enum Macro {
    Modifier { keycode: u16, modifiers: u8 },
    Tap(Vec<u16>),
    HoldRelease { hold: u16, release: u16 },
    Hold(Vec<u16>),
    Release(Vec<u16>),
    DualAction(u16, u16),
    TimedDualAction(u16, u16, u16, u16),
    Delay(u16),
}
impl Macro {
    fn serialize(&self) -> Vec<u16> {
        match *self {
            Macro::Modifier { keycode, modifiers } => {
                vec![(modifiers as u16) << 8, keycode]
            }
            Macro::Tap(ref seq) => binary_seq(macro_types::TAP, seq),
            Macro::HoldRelease { hold, release } => {
                vec![macro_types::HOLD_RELEASE, hold, release]
            }
            Macro::Hold(ref seq) => binary_seq(macro_types::HOLD, seq),
            Macro::Release(ref seq) => binary_seq(macro_types::RELEASE, seq),
            Macro::DualAction(tap, hold) => {
                vec![macro_types::DUAL_ACTION, tap, hold]
            }
            Macro::TimedDualAction(tap, hold, time1, time2) => {
                if time2 == u16::MAX {
                    vec![macro_types::DUAL_ACTION, tap, hold, time1]
                } else {
                    vec![macro_types::DUAL_ACTION, tap, hold, time1, time2]
                }
            }
            Macro::Delay(n) => {
                vec![macro_types::DELAY, n]
            }
        }
    }
}

fn binary_seq(id: u16, seq: &[u16]) -> Vec<u16> {
    let mut result = Vec::with_capacity(seq.len() + 1);
    result.push(id);
    result.extend_from_slice(seq);
    result
}

impl<'source> Parser<'source> {
    fn new(source: &'source str) -> Self {
        Self {
            iter: SourceIter::new(source.char_indices(), source.len()),
            config: KeyboardConfig::new(source),
            mark_idx: 0,
        }
    }

    fn parse_sections(&mut self) -> Result<()> {
        self.config.scan_layer_names()?;
        loop {
            match self.next_non_ws() {
                None => return Ok(()),
                Some(start) => {
                    if start.1 == '[' {
                        self.mark_start();
                        let maybe_tag_end = self
                            .iter
                            .find(|c| matches!(c, ']' | ':' | '.') || invalid_section_char(c));
                        if let Some(tag_end) = maybe_tag_end {
                            let maybe_end = match tag_end.1 {
                                ':' | '.' => {
                                    self.iter.find(|c| c == ']' || invalid_section_char(c))
                                }
                                _ => maybe_tag_end,
                            };
                            if let Some(end) = maybe_end {
                                let tag_name = &self.config.source[start.0 + 1..tag_end.0];
                                match tag_name {
                                    "global" => self.parse_global(tag_end.0 + 1..end.0)?,
                                    "matrix" => self.parse_matrix()?,
                                    "firmware" => {
                                        self.assert_no_suffix(tag_end.0, end.0)?;
                                        self.parse_firmware()?
                                    }
                                    "aliases" => {
                                        self.assert_no_suffix(tag_end.0, end.0)?;
                                        self.parse_aliases()?
                                    }
                                    _ => self.parse_layer(tag_name)?,
                                }
                                continue;
                            }
                        }

                        return Err(ConfigError::new("missing ']'".into(), start.0..start.0 + 1));
                    } else {
                        return Err(ConfigError::new(
                            "expected '['".into(),
                            start.0..start.0 + 1,
                        ));
                    }
                }
            }
        }
    }

    fn parse_global(&mut self, suffix: NameRange) -> Result<()> {
        if suffix.start < suffix.end {
            let name = self.name(&suffix);
            let (name, field) = name.split_once('.').unwrap_or((name, ""));

            match self.config.take_global(name) {
                Ok(mut g) => {
                    while let Some(pos) = self.skip_whitespace() {
                        if pos.1 == '[' {
                            break;
                        }
                        match self.parse_assignment()? {
                            None => return Ok(()),
                            Some((left, right)) => {
                                let eol = self
                                    .iter
                                    .find(|c| c == '\n')
                                    .unwrap_or((self.config.source.len(), '\0'));
                                if let Err(err) = g.set_sub_field(
                                    field,
                                    self.name(&left),
                                    self.name(&(right.start..eol.0)),
                                ) {
                                    return Err(error_span(err, left.start..eol.0));
                                }
                            }
                        }
                    }
                    self.config.insert_global(name, g);

                    return Ok(());
                }
                Err(err) => return Err(error_span(err.message, suffix)),
            }
        }

        while let Some(pos) = self.skip_whitespace() {
            if pos.1 == '[' {
                return Ok(());
            }
            match self.parse_assignment()? {
                None => return Ok(()),
                Some((left, right)) => {
                    self.assign_global(&left, &right)?;
                    self.assert_no_more_values(TOO_MANY_RHS)?;
                }
            }
        }
        Ok(())
    }

    fn assign_global(&mut self, name_range: &NameRange, value_range: &NameRange) -> Result<()> {
        let name = self.name(name_range);
        match name {
            "unicode_prefix" | "unicode_suffix" => {
                let action = self.read_action(value_range.start..value_range.end)?;
                self.config.temp_map.insert(name, action);
            }
            _ => {
                let p = globals::DEFAULTS.get(name).ok_or_else(|| {
                    error_span(
                        format!("Invalid global '{}'", name).as_str(),
                        name_range.start..name_range.end,
                    )
                })?;

                use GlobalType::*;
                let value = GlobalProp {
                    index: p.index,
                    spec: match p.spec {
                        Timeout(_) => {
                            GlobalType::Timeout(self.config.parse_duration(value_range, 5000)?)
                        }
                        _ => unreachable!(),
                    },
                };

                self.config.global_map.insert(name, value);
            }
        };
        Ok(())
    }

    fn parse_matrix(&mut self) -> Result<()> {
        while let Some(pos) = self.skip_whitespace() {
            if pos.1 == '[' {
                return Ok(());
            }
            self.skip_whitespace();
            let mark = self.iter.current.0;
            match self.parse_assignment()? {
                None => return Ok(()),
                Some((left, right)) => {
                    self.mark_idx = mark;
                    let mut pos = self
                        .config
                        .key_position(self.name(&left))
                        .ok_or_else(|| self.error("Invalid key code"))?;
                    if (pos >> 8) as u8 >= self.config.row_count {
                        return Err(self.error(TOO_MANY_ROWS));
                    }

                    let mut right = Some(right);
                    while let Some(value) = &right {
                        let value = self.name(value);
                        if (pos & 0xff) as u8 >= self.config.col_count {
                            return Err(self.error(TOO_MANY_COLS));
                        }
                        self.config.assign_position_name(pos, value);
                        if let Some(code) = keycodes::key_code(value) {
                            self.config.assign_one_layer_code("main", pos, code);
                        }
                        pos += 1;
                        right = self.next_assignment_value();
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_aliases(&mut self) -> Result<()> {
        while let Some(pos) = self.skip_whitespace() {
            if pos.1 == '[' {
                return Ok(());
            }
            self.skip_whitespace();
            match self.parse_assignment()? {
                None => return Ok(()),
                Some((key, right)) => {
                    let value = self.name(&right);
                    if !self.config.assign_aliases(self.name(&key), value) {
                        return Err(error_span("Unknown key name", key));
                    }
                    self.assert_no_more_values(TOO_MANY_RHS)?;
                }
            }
        }
        Ok(())
    }

    fn parse_firmware(&mut self) -> Result<()> {
        while let Some(pos) = self.skip_whitespace() {
            if pos.1 == '[' {
                return Ok(());
            }
            self.skip_whitespace();
            match self.parse_assignment()? {
                None => return Ok(()),
                Some((key, right)) => {
                    let eol = self
                        .iter
                        .find(|c| c == '\n')
                        .unwrap_or((self.config.source.len(), '\0'));
                    let key = self.name(&key);
                    let value = self.trim_value(&(right.start..eol.0));
                    value
                        .replace("#", "//")
                        .parse::<proc_macro2::TokenStream>()
                        .map_err(|_| error_span(SYNTAX_ERROR, right.start..eol.0))?;
                    self.config.firmware_map.insert(key, value);
                }
            }
        }
        Ok(())
    }

    fn parse_layer(&mut self, name: &str) -> Result<()> {
        while let Some(pos) = self.skip_whitespace() {
            if pos.1 == '[' {
                return Ok(());
            }
            self.skip_whitespace();
            let mark = self.iter.current.0;
            match self.parse_assignment()? {
                None => return Ok(()),
                Some((left_range, right)) => {
                    self.mark_idx = mark;
                    let left = self.name(&left_range);

                    let alias_value = self.config.get_aliases(left);

                    let keypos = if let Some(list) = alias_value {
                        if list.len() == 1 {
                            list.first().copied()
                        } else {
                            None
                        }
                    } else {
                        self.config.key_position(left)
                    };

                    if let Some(mut keypos) = keypos {
                        if (keypos >> 8) as u8 >= self.config.row_count {
                            return Err(self.error(TOO_MANY_ROWS));
                        }
                        let mut right = Some(right);
                        while let Some(value) = &right {
                            if (keypos & 0xff) as u8 >= self.config.col_count {
                                return Err(self.error(TOO_MANY_COLS));
                            }

                            let code = self.read_action(value.to_owned())?;
                            self.config.assign_one_layer_code(name, keypos, code);

                            right = self.next_assignment_value();
                            keypos += 1;
                        }
                    } else if let Some(positions) = alias_value {
                        let positions = positions.clone();
                        let code = self.read_action(right)?;
                        for pos in positions {
                            self.config.assign_one_layer_code(name, pos, code);
                        }
                        self.assert_no_more_values(TOO_MANY_MULTI_ALIAS_RHS)?;
                    } else {
                        return Err(error_span(format!("key not found! {}", left), left_range));
                    }
                }
            }
        }
        Ok(())
    }

    fn assert_no_more_values(&mut self, msg: &str) -> Result<()> {
        self.mark_start();
        if self.next_assignment_value().is_some() {
            Err(self.error(msg))
        } else {
            Ok(())
        }
    }

    fn read_action(&mut self, name_range: NameRange) -> Result<u16> {
        let name = self.name(&name_range);

        if let Some(code) = keycodes::key_code(name) {
            Ok(code)
        } else if let Some(base_code) = if self.iter.current.1 == '(' {
            keycodes::action_code(name)
        } else {
            keycodes::modifier_macro(name)
        } {
            // actions
            match base_code {
                key_range::LAYER_MIN..=key_range::REPLACE_LAYERS_MIN => {
                    self.iter.next();
                    self.parse_layer_code(base_code)
                }
                key_range::MACROS_MIN => self.parse_macro(name_range),
                _ => unimplemented!("read_action: {}", base_code),
            }
        } else {
            Err(error_span(UNKNOWN_ACTION, name_range))
        }
    }

    fn read_arg(&mut self) -> NameRange {
        self.read(invalid_arg_char)
    }

    fn read_timeout(&mut self) -> Result<u16> {
        let nr = self.read_arg();
        self.config.parse_duration(&nr, 5000)
    }

    fn read(&mut self, f: impl Fn(char) -> bool) -> NameRange {
        if let Some(start) = self.next_non_ws() {
            if invalid_seq_char(start.1) {
                self.iter.put_back(start);
                return start.0..start.0;
            }
            self.mark_start();
            if let Some(end) = self.iter.find(f) {
                self.iter.put_back(end);
                start.0..end.0
            } else {
                start.0..self.iter.current.0
            }
        } else {
            self.iter.current.0..self.iter.current.0
        }
    }

    fn read_sequence(&mut self) -> NameRange {
        if let Some(start) = self.next_non_ws() {
            if invalid_seq_char(start.1) {
                self.iter.put_back(start);
                return start.0..start.0;
            }
            self.mark_start();
            if let Some(end) = self.iter.find(invalid_seq_char) {
                self.iter.put_back(end);
                start.0..end.0
            } else {
                start.0..self.iter.current.0
            }
        } else {
            self.iter.current.0..self.iter.current.0
        }
    }

    fn expect(&mut self, c: char) -> Result<()> {
        self.mark_start();
        if let Some(start) = self.next_non_ws() {
            if start.1 == c {
                return Ok(());
            }
        }

        Err(self.error(format!("Expected {} ", c)))
    }

    fn name(&self, name_range: &NameRange) -> &'source str {
        self.config.name(name_range)
    }

    fn trim_value(&self, name_range: &NameRange) -> &'source str {
        let mut value = &self.config.source[name_range.start..name_range.end];
        if let Some(i) = value.rfind('#') {
            value = &value[..i];
        }

        value.trim()
    }

    fn dualaction(&mut self, hold: u16) -> Result<u16> {
        self.expect(',')?;

        let tap_name = self.read_arg();

        let tap = self.read_action(tap_name)?;

        let Some(c) = self.next_non_ws() else {
            return Err(self.error(EOF));
        };

        Ok(if c.1 == ')' {
            self.add_macro(Macro::DualAction(tap, hold))
        } else {
            let t1 = self.read_timeout()?;
            let Some(c) = self.next_non_ws() else {
                return Err(self.error(EOF));
            };
            if c.1 == ')' {
                self.add_macro(Macro::TimedDualAction(tap, hold, t1, u16::MAX))
            } else {
                let t2 = self.read_timeout()?;
                self.expect(')')?;
                self.add_macro(Macro::TimedDualAction(tap, hold, t1, t2))
            }
        })
    }

    fn parse_macro(&mut self, name_range: NameRange) -> Result<u16> {
        let name = self.name(&name_range);
        let id = match name {
            "macro" => {
                self.iter.next();
                let seq = self.macro_sequence()?;
                self.expect(')')?;
                if seq.len() == 2
                    && matches!(self.get_macro(seq[0]), Some(Macro::Hold(..)))
                    && matches!(self.get_macro(seq[1]), Some(Macro::Release(..)))
                {
                    self.add_macro(Macro::HoldRelease {
                        hold: seq[0],
                        release: seq[1],
                    })
                } else {
                    self.add_macro(Macro::Tap(seq))
                }
            }
            "hold" => {
                self.iter.next();
                let seq = self.macro_sequence()?;
                self.expect(')')?;
                self.add_macro(Macro::Hold(seq))
            }
            "release" => {
                self.iter.next();
                let seq = self.macro_sequence()?;
                self.expect(')')?;
                self.add_macro(Macro::Release(seq))
            }
            "overload" => {
                self.iter.next();
                let name = self.read_arg();
                if name.is_empty() {
                    return Err(self.error("Missing name"));
                }
                let hold = self.get_layer_index(name)? + key_range::LAYER_MIN;

                self.dualaction(hold)?
            }
            "dualaction" => {
                self.iter.next();
                let hold_name = self.read_arg();
                let hold = self.read_action(hold_name)?;

                self.dualaction(hold)?
            }
            "unicode" => {
                self.iter.next();
                let uc = self.read_hex_codes()?;
                self.expect(')')?;
                let mac = Macro::Tap(self.config.unicode_to_seq(uc));
                self.add_macro(mac)
            }
            "delay" => {
                self.iter.next();
                let nr = self.read_arg();
                let d = self.config.parse_duration(&nr, 5000)?;
                self.expect(')')?;
                let mac = Macro::Delay(d);
                self.add_macro(mac)
            }
            name => {
                let (modifiers, keycode) = name
                    .rsplit_once('-')
                    .ok_or_else(|| error_span(UNKNOWN_ACTION, name_range.clone()))?;
                let modifiers = keycodes::modifiers_to_bit_map(modifiers).ok_or_else(|| {
                    ConfigError::new(
                        format!("Invalid modifiers '{}'", modifiers),
                        name_range.start..name_range.start + modifiers.len(),
                    )
                })?;
                let keycode = keycodes::key_code(keycode)
                    .ok_or_else(|| error_span(UNKNOWN_ACTION, name_range))?;

                self.add_macro(Macro::Modifier { keycode, modifiers })
            }
        };
        Ok(id + key_range::MACROS_MIN)
    }

    fn read_hex_codes(&mut self) -> Result<char> {
        let mut result: u32 = 0;
        let start = self.iter.current.0;
        while let Some(ic) = self.iter.next() {
            match ic.1 {
                'a'..='f' => {
                    result = (result << 4) + 10 + (ic.1 as u32) - ('a' as u32);
                }
                'A'..='F' => {
                    result = (result << 4) + 10 + (ic.1 as u32) - ('A' as u32);
                }
                '0'..='9' => {
                    result = (result << 4) + (ic.1 as u32) - ('0' as u32);
                }
                ' ' => {}
                ')' => {
                    self.iter.put_back(ic);
                    break;
                }
                _ => {
                    return Err(error_span(
                        "Invalid unicode digit",
                        self.iter.current.0..self.iter.current.0 + 1,
                    ))
                }
            }
        }
        char::from_u32(result)
            .ok_or_else(|| error_span("Invalid unicode", start..self.iter.current.0 + 1))
    }

    fn macro_sequence(&mut self) -> Result<Vec<u16>> {
        let mut seq = Vec::new();
        loop {
            let range = self.read_sequence();
            if range.is_empty() {
                break;
            }
            let name = self.name(&range);
            if let Some(code) = keycodes::key_code(name) {
                seq.push(code);
            } else if keycodes::action_code(name).is_some() && self.iter.current.1 == '(' {
                seq.push(self.read_action(range)?);
            } else {
                let (modifiers, keycode) = name.rsplit_once('-').unwrap_or(("", name));
                let mod_mac = if keycodes::modifiers_to_bit_map(modifiers).is_some() {
                    keycodes::key_code(keycode).is_some()
                } else {
                    false
                };
                if mod_mac {
                    seq.push(self.parse_macro(range)?);
                } else {
                    for c in name.chars() {
                        let u = unshifted_char_code(c);
                        if u != c {
                            seq.push(
                                key_range::MACROS_MIN
                                    + self.add_macro(Macro::Modifier {
                                        keycode: keycodes::char_to_code(u),
                                        modifiers: keycodes::SHIFT_MOD,
                                    }),
                            );
                        } else {
                            let code = keycodes::char_to_code(c);
                            if code != 0 {
                                seq.push(code)
                            } else {
                                let mac = Macro::Tap(self.config.unicode_to_seq(c));
                                seq.push(key_range::MACROS_MIN + self.add_macro(mac));
                            }
                        }
                    }
                }
            }
        }
        Ok(seq)
    }

    fn get_macro(&self, code: u16) -> Option<&Macro> {
        if matches!(code, key_range::MACROS_MIN..=key_range::MACROS_MAX) {
            self.config
                .macros
                .get((code - key_range::MACROS_MIN) as usize)
        } else {
            None
        }
    }

    fn add_macro(&mut self, mac: Macro) -> u16 {
        let bin = mac.serialize();
        if let Some(id) = self.config.macros_names.get(&bin) {
            return *id;
        }
        let id = self.config.macros.len() as u16;

        self.config.macros.push(mac);
        self.config.macros_names.insert(bin, id);
        id
    }

    fn get_layer_index(&mut self, name_range: NameRange) -> Result<u16> {
        let name = &self.config.source[name_range.start..name_range.end];
        if let Some(index) = self.config.get_layer_index(name) {
            Ok(index)
        } else {
            Err(error_span(
                format!("Unknown layer name {}", name),
                name_range,
            ))
        }
    }

    fn parse_layer_code(&mut self, base_code: u16) -> Result<u16> {
        self.mark_start();
        if let Some(start) = self.next_non_ws() {
            self.mark_idx = start.0;
            if let Some(end) = self.iter.find(|c| c == ')' || c.is_whitespace()) {
                let end_mark = self.iter.current.0 - 1;
                if end.1 == ')'
                    || self
                        .next_non_ws()
                        .and_then(|c| if c.1 == '(' { Some(c) } else { None })
                        .is_some()
                {
                    return Ok(base_code + self.get_layer_index(start.0..end.0)?);
                }
                self.iter.current.0 = end_mark;
            }
        }
        Err(self.error("Invalid layer(...) action"))
    }

    fn parse_assignment(&mut self) -> Result<Option<(NameRange, NameRange)>> {
        if let Some(left) = self.read_word()? {
            if self.match_char('=') {
                if let Some(right) = self.next_assignment_value() {
                    return Ok(Some((left, right)));
                } else {
                    return Err(self.error("Missing RHS"));
                }
            } else {
                return Err(self.error("Missing ="));
            }
        }
        Ok(None)
    }

    fn next_assignment_value(&mut self) -> Option<NameRange> {
        let start = self.iter.find(|c| c == '\n' || !c.is_whitespace())?;
        self.mark_start();
        if start.1 == '\n' {
            return None;
        }
        if let Some(parens) = match_paren(start.1) {
            match self.iter.find_close_paren(parens) {
                Some(end) => Some(start.0..end.0),
                None => Some(start.0..self.iter.current.0),
            }
        } else {
            match self.iter.find(invalid_seq_char) {
                Some(end) => {
                    self.iter.put_back(end);
                    Some(start.0..end.0)
                }
                None => Some(start.0..self.iter.current.0),
            }
        }
    }

    fn mark_start(&mut self) {
        self.mark_idx = self.iter.current.0;
    }

    fn error(&self, message: impl Into<String>) -> ConfigError {
        ConfigError::new(message.into(), self.mark_idx..self.iter.current.0)
    }

    fn read_word(&mut self) -> Result<Option<NameRange>> {
        match self.iter.find(|c| c == '\n' || !c.is_whitespace()) {
            None => Ok(None),
            Some(start) => {
                self.mark_start();
                if start.1 == '\n' {
                    return Ok(None);
                }
                match self.iter.find(|c| c.is_whitespace()) {
                    Some(end) => {
                        if end.1 == '\n' {
                            self.iter.put_back(end);
                        }
                        Ok(Some(start.0..end.0))
                    }
                    None => Err(self.error("Expected word")),
                }
            }
        }
    }

    fn match_char(&mut self, c: char) -> bool {
        match self.next_non_ws() {
            None => false,
            Some(start) => start.1 == c,
        }
    }

    fn build_config(self) -> KeyboardConfig<'source> {
        self.config
    }

    fn skip_whitespace(&mut self) -> Option<IndexChar> {
        self.next_non_ws().inspect(|&item| {
            self.iter.put_back(item);
        })
    }

    #[inline]
    fn next_non_ws(&mut self) -> Option<IndexChar> {
        self.iter.find(non_ws_char)
    }

    fn assert_no_suffix(&self, tag_end: usize, end: usize) -> Result<()> {
        if tag_end == end {
            Ok(())
        } else {
            Err(ConfigError::new(
                "suffix not allowed here".into(),
                tag_end..end,
            ))
        }
    }
}

fn match_paren(start: char) -> Option<(char, char)> {
    Some(match start {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        '{' => ('{', '}'),
        _ => return None,
    })
}

const DEFAULT_LAYERS: [(&str, u8); 6] = [
    ("control", 1),
    ("shift", 2),
    ("alt", 4),
    ("gui", 8),
    ("altgr", 0x40),
    ("main", 0),
];

fn data_to_usize(x: u16) -> usize {
    u16::from_le(x) as usize
}

impl<'source> KeyboardConfig<'source> {
    fn new(source: &'source str) -> Self {
        let mut layers: HashMap<String, ConfigLayer> = Default::default();
        for (i, (name, code)) in DEFAULT_LAYERS.into_iter().enumerate() {
            layers.insert(name.into(), ConfigLayer::new(i as u16, code));
        }

        Self {
            source,
            global_map: Default::default(),
            temp_map: Default::default(),
            firmware_map: Default::default(),
            matrix_map: Default::default(),
            layers,
            macros_names: Default::default(),
            macros: Default::default(),
            next_layer: DEFAULT_LAYERS.len() as u16,
            row_count: 0,
            col_count: 0,
        }
    }

    pub fn deserialize(data: &[u16]) -> Self {
        assert!(data.len() > 14);
        assert_eq!(data[0], PROTOCOL_VERSION);

        let mut config = Self::new("");

        config.row_count = (u16::from_le(data[1]) >> 8) as u8;
        config.col_count = (u16::from_le(data[1]) & 0xff) as u8;
        let layer_count = (u16::from_le(data[2]) & 0xff) as usize;
        // TODO let macros_count = u16::from_le(data[3]) as usize;
        let globals_count = u16::from_le(data[4]) as usize;

        config.deserialize_globals(&mut data[5..5 + globals_count].iter().copied());

        let data = &data[5 + globals_count..]; // TODO read globals

        for (i, (name, _)) in DEFAULT_LAYERS.into_iter().enumerate() {
            let layer = config.layers.get_mut(name).unwrap();
            layer.set_binary(
                &data[data_to_usize(data[i])..data_to_usize(data[1 + i])],
                config.row_count as usize,
                config.col_count as usize,
            );
        }

        for i in DEFAULT_LAYERS.len()..layer_count {
            let name = format!("layer{}", i);
            config.new_layer(name.as_str(), 0);
            let layer = config.layers.get_mut(name.as_str()).unwrap();
            layer.set_binary(
                &data[data_to_usize(data[i])..data_to_usize(data[1 + i])],
                config.row_count as usize,
                config.col_count as usize,
            );
        }

        // TODO read in macros

        config
    }

    pub fn serialize(&self) -> Vec<u16> {
        let layer_count = self.layers.len();
        let macros_count = self.macros.len();

        let globals = self.serialize_globals();

        let layer_base = 5 + globals.len();
        let mut out = vec![0; 1 + layer_count + macros_count + layer_base];

        out[0] = PROTOCOL_VERSION.to_le();
        out[1] = ((self.col_count as u16) | ((self.row_count as u16) << 8)).to_le();
        out[2] = (layer_count as u16).to_le();
        out[3] = (macros_count as u16).to_le();
        out[4] = (globals.len() as u16).to_le();

        out[5..layer_base].copy_from_slice(globals.as_slice());

        let mut layers = self.layers.values().collect::<Vec<_>>();
        layers.sort_by(|a, b| Ord::cmp(&a.index, &b.index));
        for (i, mut l) in layers
            .iter()
            .map(|l| l.serialize(self.row_count as usize, self.col_count as usize))
            .enumerate()
        {
            out[layer_base + i] = ((out.len() - layer_base) as u16).to_le();
            out.append(&mut l);
        }
        let macro_base = layer_base + layer_count;

        for (i, mut m) in self.macros.iter().map(|m| m.serialize()).enumerate() {
            out[macro_base + i] = ((out.len() - layer_base) as u16).to_le();
            out.append(&mut m);
        }

        out[macro_base + macros_count] = ((out.len() - layer_base) as u16).to_le();

        out
    }

    pub fn deserialize_globals(&mut self, data: &mut impl Iterator<Item = u16>) {
        while let Some(gp) = GlobalProp::deserialize(data) {
            if let Some(name) = gp.default_name() {
                self.global_map.insert(name, gp);
            }
        }
    }

    pub fn serialize_globals(&self) -> Vec<u16> {
        let mut out = self.global_map.values().collect::<Vec<_>>();
        out.sort_by(|a, b| Ord::cmp(&a.index, &b.index));
        out.into_iter().flat_map(|v| v.serialize()).collect()
    }

    fn name(&self, name_range: &NameRange) -> &'source str {
        let start = if name_range.len() > 1 && self.source[name_range.start..].starts_with('\\') {
            name_range.start + 1
        } else {
            name_range.start
        };
        &self.source[start..name_range.end]
    }

    fn unicode_to_seq(&mut self, uc: char) -> Vec<u16> {
        let uc = uc as u32;

        let mut seq = vec![*self.temp_map.get("unicode_prefix").unwrap_or(&0)];

        for i in 0..7 {
            let i = (6 - i) << 2;
            let d = ((uc >> i) & 0xf) as u16;

            let kc = match d {
                0 if seq.len() == 1 => continue,
                0 => BASIC_0,
                10..=15 => BASIC_A + d - 10,
                _ => BASIC_1 + d - 1,
            };
            seq.push(kc);
        }

        seq.push(*self.temp_map.get("unicode_suffix").unwrap_or(&0));

        seq
    }

    fn parse_duration(&mut self, value_range: &NameRange, max: u16) -> Result<u16> {
        if let Ok(n) = self.name(value_range).parse::<u16>() {
            if n <= max {
                return Ok(n);
            }
        }
        Err(error_span(
            format!("Invalid duration; only 0 to {} milliseconds are valid", max),
            value_range.start..value_range.end,
        ))
    }

    pub fn key_position(&self, name: &str) -> Option<u16> {
        if let Some(name) = name.strip_prefix("0x") {
            if let Ok(pos) = u16::from_str_radix(name, 16) {
                return Some(match name.len() {
                    2 => (pos & 0xf0) << 4 | (pos & 0xf),
                    3..=4 => pos,
                    _ => return None,
                });
            }
        }
        None
    }

    pub fn global(&self, name: &str) -> Option<GlobalProp> {
        self.global_map.get(name).copied()
    }

    fn take_global(&mut self, name: &'source str) -> Result<GlobalProp> {
        match self.global_map.remove(name) {
            Some(g) => Ok(g),
            None => GlobalProp::new_default(name).map_err(|e| ConfigError::from(e.as_str())),
        }
    }

    fn insert_global(&mut self, name: &'source str, value: GlobalProp) {
        self.global_map.insert(name, value);
    }

    fn assign_aliases(&mut self, key: &str, value: &str) -> bool {
        if let Some(pos) = self.key_position(key) {
            self.assign_position_name(pos, value);
            return true;
        }
        if let Some(positions) = self.get_aliases(key) {
            let positions = positions.clone();
            for pos in positions {
                self.assign_position_name(pos, value);
            }
            return true;
        }
        false
    }

    fn assign_position_name(&mut self, pos: u16, name: &str) {
        if let Some(code) = key_code(name) {
            let name = format!("{code:04X}");
            if let Some(v) = self.matrix_map.get_mut(name.as_str()) {
                v.push(pos);
            } else {
                self.matrix_map.insert(name, vec![pos]);
            }
        } else if let Some(v) = self.matrix_map.get_mut(name) {
            v.push(pos);
        } else {
            self.matrix_map.insert(name.into(), vec![pos]);
        }
    }

    pub fn get_aliases(&self, name: &str) -> Option<&Vec<u16>> {
        if let Some(code) = key_code(name) {
            self.matrix_map.get(format!("{code:04X}").as_str())
        } else {
            self.matrix_map.get(name)
        }
    }

    pub fn code_at(&self, name: &str, rowcol: u16) -> u16 {
        if let Some(layer) = self.layers.get(name) {
            return layer.code_at(rowcol);
        }
        0
    }

    fn assign_one_layer_code(&mut self, name: &str, pos: u16, code: u16) {
        self.layers.get_mut(name).unwrap().set_code(pos, code);
    }

    fn new_layer(&mut self, name: &str, code: u8) {
        self.layers
            .insert(name.into(), ConfigLayer::new(self.next_layer, code));

        self.next_layer += 1;
    }

    fn get_layer_index(&self, name: &str) -> Option<u16> {
        self.layers.get(name).map(|l| l.index)
    }

    fn scan_layer_names(&mut self) -> Result<()> {
        #[derive(PartialEq)]
        enum State {
            Ready,
            Read(usize, usize),
            Search,
        }
        let mut in_comment = false;
        let mut escaped = false;
        let mut paren = 0;
        let mut parens = ('(', ')');
        let mut state = State::Ready;
        for (i, c) in self.source.char_indices() {
            match c {
                _ if escaped => {
                    escaped = false;
                }
                '\n' if paren == 0 => {
                    state = State::Ready;
                    in_comment = false;
                }
                _ if in_comment => {}
                '#' => {
                    in_comment = true;
                }
                '\\' => {
                    escaped = true;
                }
                _ if paren > 0 => {
                    if c == parens.0 {
                        paren += 1;
                    } else if c == parens.1 {
                        paren -= 1;
                    }
                }
                _ => match state {
                    State::Read(s, e) => match c {
                        ':' if s == e => {
                            state = State::Read(s, i);
                        }
                        ']' => {
                            self.ensure_section(s, e, i)?;
                            state = State::Search;
                        }
                        _ => {}
                    },
                    State::Ready => match c {
                        '[' => {
                            state = State::Read(i + 1, i + 1);
                        }
                        _ if c.is_whitespace() => {}
                        _ => {
                            state = State::Search;
                            if let Some(p) = match_paren(c) {
                                parens = p;
                                paren = 1;
                            }
                        }
                    },
                    State::Search => {}
                },
            }
        }
        Ok(())
    }

    fn ensure_section(&mut self, s: usize, e: usize, i: usize) -> Result<()> {
        let (name, suffix) = if s == e {
            (&self.source[s..i], &self.source[s..e])
        } else {
            (&self.source[s..e], &self.source[e + 1..i])
        };

        match name {
            "matrix" => {
                let mut iter = suffix.split_terminator('x');
                if let (Some(row_count), Some(col_count)) = (iter.next(), iter.next()) {
                    if let (Ok(row_count), Ok(col_count)) =
                        (row_count.parse::<u8>(), col_count.parse::<u8>())
                    {
                        self.row_count = row_count;
                        self.col_count = col_count;
                        return Ok(());
                    }
                }

                return Err(ConfigError::new(
                    "expected [matrix:rxc] where r and c are row column size".into(),
                    s..i,
                ));
            }
            "aliases" => {}
            _ => {
                let code = keycodes::modifiers_to_bit_map(suffix).ok_or_else(|| {
                    ConfigError::new(format!("Invalid layer suffix '{}'", suffix), e + 1..i)
                })?;

                if let Some(layer) = self.layers.get(name) {
                    if layer.suffix != code && code != 0 {
                        return Err(ConfigError::new(
                            format!(
                                "layer suffix may not be changed; {} != {}",
                                keycodes::modifiers_to_string(layer.suffix),
                                suffix,
                            ),
                            s..i,
                        ));
                    }
                } else {
                    self.new_layer(name, code);
                }
            }
        }

        Ok(())
    }

    pub fn firmware_get(&self, arg: &str) -> Option<&str> {
        self.firmware_map
            .get(arg)
            .or_else(|| {
                let arg = arg.to_uppercase();
                self.firmware_map.get(arg.as_str())
            })
            .copied()
    }
}

impl ConfigLayer {
    fn new(index: u16, suffix: u8) -> Self {
        Self {
            codes: Default::default(),
            index,
            suffix,
        }
    }

    fn set_binary(&mut self, data: &[u16], row_count: usize, col_count: usize) {
        self.suffix = u16::from_le(data[0]) as u8;

        let data = &data[1..];
        if row_count * col_count == data.len() {
            for row in 0..row_count {
                for col in 0..col_count {
                    self.codes.insert(
                        ((row << 8) + col) as u16,
                        u16::from_le(data[row * col_count + col]),
                    );
                }
            }
        } else {
            let mut k = 0;
            for (i, v) in data.iter().enumerate() {
                let v = u16::from_le(*v);
                if i & 1 == 0 {
                    k = v;
                } else {
                    self.codes.insert(k, v);
                }
            }
        }
    }

    fn serialize(&self, row_count: usize, col_count: usize) -> Vec<u16> {
        let mut bin = vec![0];
        bin[0] = (self.suffix as u16).to_le();
        if self.codes.len() * 3 > row_count * col_count {
            // normal array
            bin.resize(1 + row_count * col_count, 0);
            for (k, v) in self.codes.iter() {
                let i = (k >> 8) as usize * col_count + (k & 0xff) as usize;
                let _ = (k, v, i, k >> 8, (k & 0xff));
                bin[i + 1] = v.to_le();
            }
        } else {
            bin.resize(1 + self.codes.len() * 2, 0);
            let mut codes = self.codes.iter().collect::<Vec<_>>();
            codes.sort_by_key(|k| k.0);
            for (i, (k, v)) in codes.into_iter().enumerate() {
                bin[i * 2 + 1] = k.to_le();
                bin[i * 2 + 2] = v.to_le();
            }
            // sparse array
        }
        bin
    }

    pub fn code_at(&self, pos: u16) -> u16 {
        *self.codes.get(&pos).unwrap_or(&0)
    }

    fn set_code(&mut self, pos: u16, code: u16) {
        self.codes.insert(pos, code);
    }
}

fn error_span(message: impl Into<String>, range: NameRange) -> ConfigError {
    ConfigError::new(message.into(), range)
}

pub fn compile(source: &str) -> Result<KeyboardConfig> {
    let mut parser = Parser::new(source);

    parser.parse_sections()?;
    Ok(parser.build_config())
}

#[cfg(test)]
#[path = "compiler_test.rs"]
mod test;
