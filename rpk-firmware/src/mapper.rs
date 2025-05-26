use core::{cell::RefCell, cmp::min, sync::atomic};

use dual_action::DualActionTimer;
use embassy_futures::select::{select, Either};
use embassy_sync::{
    blocking_mutex::raw::{NoopRawMutex, RawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use macros::{Macro, TapDance};
use mouse::Mouse;
use rpk_common::{globals, keycodes::key_range};

use crate::{
    firmware_functions,
    key_scanner::{KeyScannerChannel, ScanKey},
    layout,
};

pub mod config_loader;

pub(crate) mod dual_action;
pub(crate) mod macros;
pub(crate) mod mouse;

#[derive(Copy, Clone, Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct TimedScanKey(pub(crate) ScanKey, pub(crate) u64);
impl TimedScanKey {
    pub fn same_key(&self, scan_key: &TimedScanKey) -> bool {
        self.0.same_key(scan_key.0)
    }

    pub fn is_none(&self) -> bool {
        self.0.is_none()
    }

    fn none() -> Self {
        Self(ScanKey::none(), 0)
    }

    fn as_memo(&self) -> [u16; 5] {
        [
            self.0.as_memo(),
            u64tou16(self.1, 48),
            u64tou16(self.1, 32),
            u64tou16(self.1, 16),
            u64tou16(self.1, 0),
        ]
    }

    fn from_memo(memo: &[u16]) -> Self {
        Self(
            ScanKey::from_memo(memo[0]),
            memo[1..5]
                .iter()
                .map(|i| *i as u64)
                .reduce(|a, i| (a << 16) + i)
                .unwrap_or(0),
        )
    }
}

#[inline(always)]
fn u64tou16(t: u64, shift: usize) -> u16 {
    ((t >> shift) & 0xffff) as u16
}

enum Oneshot {
    None,
    WaitUp(u16),
    Ready(u16),
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum KeyEvent {
    Basic(u8, bool),
    PendingModifiers(u8, bool),
    Modifiers(u8, bool),
    Consumer(u16),
    SysCtl(u16),
    MouseButton(u8),
    MouseMove(u8, u8, u8),
    Pending,
    Clear,
    Delay(u16),
}
impl KeyEvent {
    fn consumer(kc: u16, is_down: bool) -> Self {
        let kc = kc - key_range::CONSUMER_MIN;
        Self::Consumer(if is_down { kc } else { 0 })
    }

    fn sys_ctl(kc: u16, is_down: bool) -> Self {
        let kc = kc - key_range::SYS_CTL_MIN + key_range::SYS_CTL_BASE;
        Self::SysCtl(if is_down { kc } else { 0 })
    }

    fn basic(kc8: u8, is_down: bool) -> Self {
        Self::Basic(kc8, is_down)
    }

    fn mouse_button(mouse: u8) -> Self {
        Self::MouseButton(mouse)
    }

    fn mouse_move(mouse: u8, amount: i8, keys: u8) -> Self {
        Self::MouseMove(mouse, amount as u8, keys)
    }

    fn modifiers(modifiers: u8, is_down: bool, pending: bool) -> Self {
        if pending {
            Self::PendingModifiers(modifiers, is_down)
        } else {
            Self::Modifiers(modifiers, is_down)
        }
    }
}

pub enum ControlMessage {
    LoadLayout { file_location: u32 },
    TimerExpired,
    Exit,
}
#[derive(Default)]
pub struct ControlSignal(Signal<NoopRawMutex, ControlMessage>);
impl ControlSignal {
    pub fn load_layout(&self, file_location: u32) {
        self.0.signal(ControlMessage::LoadLayout { file_location });
    }

    #[cfg(test)]
    pub fn try_take(&self) -> Option<ControlMessage> {
        self.0.try_take()
    }
}

pub struct MapperTimer {
    expires_at: RefCell<Instant>,
    at_sig: Signal<NoopRawMutex, Instant>,
    ctl_sig: ControlSignal,
}
impl Default for MapperTimer {
    fn default() -> Self {
        Self {
            expires_at: RefCell::new(Instant::MAX),
            at_sig: Default::default(),
            ctl_sig: Default::default(),
        }
    }
}
impl MapperTimer {
    pub fn shutdown(&self) {
        self.at_sig.signal(Instant::MIN);
    }
    fn at(&self, expires_at: Instant) {
        self.at_sig.signal(expires_at);
    }

    async fn wait_control(&self) {
        self.set_expires_at(self.at_sig.wait().await);
    }

    pub async fn run(timer: &Self) {
        loop {
            match timer.get_expires_at() {
                Instant::MAX => timer.wait_control().await,
                Instant::MIN => break,
                expires_at => {
                    if let Either::First(_) =
                        select(Timer::at(expires_at), timer.wait_control()).await
                    {
                        timer.set_expires_at(Instant::MAX);
                        timer.signal(ControlMessage::TimerExpired);
                    }
                }
            }
        }
    }

    fn signal(&self, msg: ControlMessage) {
        self.ctl_sig.0.signal(msg);
    }

    fn get_expires_at(&self) -> Instant {
        let guard = self.expires_at.borrow();
        *guard
    }

    fn set_expires_at(&self, expires_at: Instant) {
        let mut guard = self.expires_at.borrow_mut();
        *guard = expires_at;
    }
}

pub struct MapperChannel<M: RawMutex, const N: usize>(Channel<M, KeyEvent, N>, MapperTimer);
impl<M: RawMutex, const N: usize> Default for MapperChannel<M, N> {
    fn default() -> Self {
        Self(Channel::new(), MapperTimer::default())
    }
}
impl<M: RawMutex, const N: usize> MapperChannel<M, N> {
    pub async fn receive(&self) -> KeyEvent {
        self.0.receive().await
    }

    pub fn timer(&self) -> &MapperTimer {
        &self.1
    }

    async fn wait_control(&self) -> ControlMessage {
        self.control().0.wait().await
    }

    pub fn control(&self) -> &ControlSignal {
        &self.1.ctl_sig
    }

    fn report(&self, message: KeyEvent) {
        if self.0.try_send(message).is_err() {
            self.clear_reports();
            let _ = self.0.try_send(KeyEvent::Clear);
        }
    }

    fn clear_reports(&self) {
        self.0.clear();
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct KeyPlusMod(u16, u8);
impl KeyPlusMod {
    pub fn new(code: u16, modifiers: u8) -> Self {
        Self(code, modifiers)
    }

    fn none() -> Self {
        Self(0, 0)
    }
}

const MIN_REPORT_BUFFER_SIZE: usize = 4;

const fn assert_sizes<const LAYOUT_MAX: usize, const REPORT_BUFFER_SIZE: usize>() -> bool {
    assert!(REPORT_BUFFER_SIZE >= MIN_REPORT_BUFFER_SIZE);
    assert!(LAYOUT_MAX > 64);
    true
}

pub struct Mapper<
    'c,
    const ROW_COUNT: usize,
    const COL_COUNT: usize,
    const LAYOUT_MAX: usize,
    M: RawMutex,
    const REPORT_BUFFER_SIZE: usize,
> {
    layout: layout::Manager<ROW_COUNT, COL_COUNT, LAYOUT_MAX>,
    active_actions: [[KeyPlusMod; COL_COUNT]; ROW_COUNT],
    mouse: mouse::Mouse,

    modifier_count: [i8; 8],
    report_channel: &'c MapperChannel<M, REPORT_BUFFER_SIZE>,
    wait_time: u64,
    oneshot: Oneshot,
    dual_action: DualActionTimer,
    tapdance: TapDance,
    last_scan_key: TimedScanKey,
    macro_running: Macro,
    memo_count: usize,
    now: u64,
    report_count: u16,
    debounce_ms_atomic: &'c atomic::AtomicU16,
    pending_down_modifiers: u8,
    pending_up_modifiers: u8,
}
impl<
        'c,
        const ROW_COUNT: usize,
        const COL_COUNT: usize,
        M: RawMutex,
        const LAYOUT_MAX: usize,
        const REPORT_BUFFER_SIZE: usize,
    > Mapper<'c, ROW_COUNT, COL_COUNT, LAYOUT_MAX, M, REPORT_BUFFER_SIZE>
{
    const OKAY: bool = assert_sizes::<LAYOUT_MAX, REPORT_BUFFER_SIZE>();
    pub fn new(
        report_channel: &'c MapperChannel<M, REPORT_BUFFER_SIZE>,
        debounce_ms_atomic: &'c atomic::AtomicU16,
    ) -> Self {
        assert!(Self::OKAY);
        Self {
            layout: layout::Manager::default(),
            active_actions: [[KeyPlusMod::none(); COL_COUNT]; ROW_COUNT],
            mouse: Mouse::default(),
            modifier_count: Default::default(),
            report_channel,
            wait_time: u64::MAX,
            oneshot: Oneshot::None,
            dual_action: Default::default(),
            tapdance: Default::default(),
            last_scan_key: TimedScanKey::none(),
            macro_running: Macro::Noop,
            memo_count: 0,
            now: 1,
            report_count: 0,
            debounce_ms_atomic,
            pending_down_modifiers: 0,
            pending_up_modifiers: 0,
        }
    }

    fn clear_all(&mut self) {
        for r in self.active_actions.iter_mut() {
            for c in r.iter_mut() {
                *c = KeyPlusMod::none();
            }
        }
        for m in self.modifier_count.iter_mut() {
            *m = 0;
        }
        self.macro_running = Macro::Noop;
        self.tapdance.clear();
        self.mouse.clear_all();
        self.layout.clear_all();
        self.dual_action = DualActionTimer::NoDual;
        self.report_channel.clear_reports();
        self.report(KeyEvent::Clear);
    }

    fn clear_layers(&mut self) {
        for m in self.modifier_count.iter_mut() {
            *m = 0;
        }
        self.layout.clear_layers();
        self.report(KeyEvent::Modifiers(0, false));
    }

    fn stop_active(&mut self) {
        for m in self.modifier_count.iter_mut() {
            *m = 0;
        }
        self.layout.clear_modifier_layers();
        self.report(KeyEvent::Clear);
    }

    pub async fn run<const SCANNER_BUFFER_SIZE: usize>(
        &mut self,
        key_scan_channel: &'c KeyScannerChannel<M, SCANNER_BUFFER_SIZE>,
    ) -> ControlMessage {
        'outer: loop {
            // run this first because no macros may be present when running memos
            while !matches!(self.macro_running, Macro::Noop) {
                self.wait_for_report_capacity().await;
                self.check_time();
            }

            // run second
            self.wait_for_report_capacity().await;
            if self.run_memo() {
                continue 'outer;
            }

            let event = select(
                key_scan_channel.receive(),
                self.report_channel.wait_control(),
            )
            .await;

            self.now = Instant::now().as_millis();

            // now look for events
            match event {
                Either::First(scan_key) => self.key_switch(TimedScanKey(scan_key, self.now)),
                Either::Second(ControlMessage::TimerExpired) => self.check_time(),
                Either::Second(ControlMessage::Exit) => return ControlMessage::Exit,
                Either::Second(ctl) => {
                    self.clear_all();
                    return ctl;
                }
            }
        }
    }

    fn dual_action_expired(&mut self) {
        self.dual_action.timer_expired();
        if let DualActionTimer::Hold { scan_key, .. } = &self.dual_action {
            self.key_switch(*scan_key);
        }
    }

    pub fn key_switch(&mut self, k: TimedScanKey) {
        if self.tapdance.is_running() {
            if !self.last_scan_key.same_key(&k) || self.tapdance.rem == 0 {
                self.tapdance_timeout();
                self.push_scan_key(&k);
            } else {
                self.last_scan_key = k;
                self.key_switch_1(k.0);
            }
            return;
        }

        self.last_scan_key = k;
        if self.dual_action.key_switch(k) {
            self.key_switch_1(k.0);
            return;
        }

        match self.dual_action {
            DualActionTimer::NoDual | DualActionTimer::Wait { .. } => {
                self.push_scan_key(&k);
            }
            DualActionTimer::Hold { scan_key, hold } => {
                if !scan_key.0.is_same_key(k.0) {
                    self.push_scan_key(&k);
                }
                self.dual_action = DualActionTimer::NoDual;
                self.last_scan_key = scan_key;
                self.run_action(hold, true);
                self.flush_modifiers(false);
            }
            DualActionTimer::Tap { scan_key, tap } => {
                self.dual_action = DualActionTimer::NoDual;
                self.last_scan_key = scan_key;
                if self.push_action(tap, false) {
                    self.run_action(tap, true);
                    self.flush_modifiers(false);
                }
            }
        }
    }

    fn key_switch_1(&mut self, k: ScanKey) {
        let rc = self.report_count;
        if k.is_down() {
            let Some(kc) = self.layout.find_code(k.row(), k.column()) else {
                return;
            };
            self.active_actions[k.row()][k.column()] = kc;
            if kc.1 != 0 {
                self.write_modifiers(kc.1, -1, true);
            }
            self.run_action(kc.0, true);
            if rc == self.report_count && kc.1 != 0 {
                self.write_modifiers(kc.1, 1, true);
            }
        } else {
            let kc = self.active_actions[k.row()][k.column()];
            self.run_action(kc.0, false);

            if rc != self.report_count && kc.1 != 0 {
                self.write_modifiers(kc.1, 1, true);
            }
            match self.oneshot {
                Oneshot::None => {}
                Oneshot::WaitUp(layern) => self.oneshot = Oneshot::Ready(layern),
                Oneshot::Ready(layern) => {
                    self.oneshot = Oneshot::None;
                    self.pop_layer(layern);
                }
            }
        };
        self.flush_modifiers(false);
    }

    fn flush_modifiers(&mut self, pending: bool) {
        if self.pending_up_modifiers != 0 || self.pending_down_modifiers != 0 {
            let clear = !(self.pending_up_modifiers & self.pending_down_modifiers);
            let up_mods = self.pending_up_modifiers & clear;
            let down_mods = self.pending_down_modifiers & clear;
            if up_mods != 0 {
                self.report_channel.report(KeyEvent::modifiers(
                    up_mods,
                    false,
                    pending || down_mods != 0,
                ));
            }
            if down_mods != 0 {
                self.report_channel
                    .report(KeyEvent::modifiers(down_mods, true, pending));
            }
            self.pending_up_modifiers = 0;
            self.pending_down_modifiers = 0;
        }
    }

    fn report(&mut self, message: KeyEvent) {
        self.report_count = self.report_count.wrapping_add(1);
        self.flush_modifiers(true);
        self.report_channel.report(message);
    }

    fn run_action(&mut self, action: u16, is_down: bool) {
        match action {
            key_range::MODIFIER_MIN..=key_range::MODIFIER_MAX => {
                self.modifier(action, is_down);
            }
            key_range::BASIC_MIN..=key_range::BASIC_MAX => {
                self.report(KeyEvent::basic(action as u8, is_down));
            }
            key_range::MACROS_MIN..=key_range::MACROS_MAX => {
                self.macros(action, is_down);
            }
            key_range::CONSUMER_MIN..=key_range::CONSUMER_MAX => {
                self.report(KeyEvent::consumer(action, is_down));
            }
            key_range::SYS_CTL_MIN..=key_range::SYS_CTL_MAX => {
                self.report(KeyEvent::sys_ctl(action, is_down));
            }
            key_range::MOUSE_MIN..=key_range::MOUSE_MAX => self.mouse(action, is_down),
            key_range::LAYER..=key_range::LAYERS_LAST => self.layer(action, is_down),
            key_range::FIRMWARE_MIN..=key_range::FIRMWARE_MAX => {
                self.firmware_action(action, is_down);
            }
            _ => {}
        }
    }

    fn firmware_action(&mut self, action: u16, is_down: bool) {
        let rc = self.report_count;
        match action {
            key_range::FW_RESET_KEYBOARD => {
                if !is_down {
                    firmware_functions::reset();
                }
            }
            key_range::FW_RESET_TO_USB_BOOT => {
                if !is_down {
                    firmware_functions::reset_to_usb_boot();
                }
            }
            key_range::FW_CLEAR_ALL => {
                self.clear_all();
            }
            key_range::FW_CLEAR_LAYERS => {
                self.clear_layers();
            }
            key_range::FW_STOP_ACTIVE => {
                self.stop_active();
            }
            _ => {
                crate::info!(
                    "not yet supported: {:?} {:?}",
                    action,
                    key_range::FW_RESET_KEYBOARD
                );
            }
        }
        if !is_down {
            self.report_count = rc;
        }
    }

    pub fn load_layout(
        &mut self,
        layout_mapping: impl IntoIterator<Item = u16>,
    ) -> Result<(), layout::LoadError> {
        self.layout.load(layout_mapping)?;
        self.mouse
            .set_config(self.layout.get_mouse_profile(1).unwrap());
        self.debounce_ms_atomic.store(
            self.layout.global(globals::DEBOUNCE_SETTLE_TIME as usize),
            atomic::Ordering::Relaxed,
        );

        Ok(())
    }

    fn macros(&mut self, code: u16, is_down: bool) {
        let id = code - key_range::MACROS_MIN;
        let mac = self.layout.get_macro(id);
        match &mac {
            Macro::Modifier(key_plus_mod) => {
                if is_down {
                    self.write_modifiers(key_plus_mod.1, 10, true);
                    self.run_action(key_plus_mod.0, is_down);
                } else {
                    self.run_action(key_plus_mod.0, is_down);
                    self.write_modifiers(key_plus_mod.1, -10, true);
                }
            }
            Macro::DualAction(tap, hold, t1, t2) => {
                self.start_dual_action(is_down, *tap, *hold, *t1, *t2);
            }
            Macro::TapDance(location, len) => {
                if *len > 2 {
                    self.tapdance(*location, *len);
                }
            }
            Macro::Noop => {}
            Macro::Sequence {
                mode,
                location,
                rem,
            } => {
                if *rem > 0 {
                    let run = match mode {
                        macros::SequenceMode::Tap | macros::SequenceMode::Hold => is_down,
                        macros::SequenceMode::Release => !is_down,
                    };
                    if run {
                        self.push_macro(mac);
                        self.next_macro_seq(*location, *rem, *mode);
                    }
                }
            }
            Macro::HoldRelease { hold, release } => {
                self.macros(if is_down { *hold } else { *release }, is_down);
            }
            Macro::Delay(n) => {
                if is_down {
                    self.report(KeyEvent::Delay(*n));
                }
            }
        }
    }

    fn push_macro(&mut self, mac: Macro) {
        let c = self.macro_running;
        self.layout.update_macro(&c);
        self.macro_running = self.layout.push_macro(mac);
    }

    fn pop_macro(&mut self) {
        self.macro_running = self.layout.pop_macro();
        self.set_wait_time();
    }

    fn mouse(&mut self, code: u16, is_down: bool) {
        let code = code - key_range::MOUSE_MIN;
        match code {
            key_range::MOUSE_ACCEL..=key_range::MOUSE_ACCEL_END => {
                self.mouse.set_config(
                    self.layout
                        .get_mouse_profile((code - key_range::MOUSE_ACCEL) as usize)
                        .unwrap(),
                );
            }
            _ => match self.mouse.action(code, is_down, self.now) {
                Some(key_event) => self.report(key_event),
                None => self.set_wait_time(),
            },
        }
    }

    fn check_time(&mut self) {
        if self.wait_time != u64::MAX {
            match self.macro_running {
                Macro::Noop => {
                    if self.now >= self.wait_time {
                        if self.now >= self.mouse.next_event_time() {
                            let pending = self.mouse.pending_events(self.now);
                            for event in pending {
                                self.report_channel.report(event);
                            }
                        } else if self.dual_action.wait_until() <= self.now {
                            self.dual_action_expired();
                        } else if self.tapdance.wait_until <= self.now {
                            self.tapdance_timeout();
                        }
                    }
                    self.set_wait_time();
                }
                Macro::Sequence {
                    mode,
                    location,
                    rem,
                } => {
                    self.next_macro_seq(location, rem, mode);
                }
                _ => {}
            }
        }
    }

    fn tapdance(&mut self, location: u32, len: u16) {
        if !self.tapdance.is_same(location, len) {
            let tl = location as usize;
            let tap_timeout = self.layout.macro_code(tl);
            self.tapdance.start(tap_timeout, location + 1, len - 1);
        }

        if self.tapdance.rem < 2 {
            if self.tapdance.rem == 1 {
                self.tapdance.location += 1;
                self.tapdance.rem -= 1;
            }
            self.tapdance.wait_until = self.now;
        } else {
            self.tapdance.location += 1;
            self.tapdance.rem -= 1;
            let timeout = if self.tapdance.tap_timeout == u16::MAX {
                self.layout.global(globals::TAPDANCE_TAP_TIMEOUT as usize)
            } else {
                self.tapdance.tap_timeout
            };

            if self.tapdance.rem & 1 == 1 {
                self.tapdance.wait_until = self.now + timeout as u64;
            }
        }
        self.set_wait_time();
    }

    fn tapdance_timeout(&mut self) {
        let location = self.tapdance.location as usize - 1;
        let action = self.layout.macro_code(location);

        self.tapdance.clear();
        self.set_wait_time();

        let k = self.last_scan_key.0;
        let mut aa = self.active_actions[k.row()][k.column()];
        let m = aa.1;

        if m != 0 {
            self.write_modifiers(m, -1, true);
        }
        aa.0 = action;
        aa.1 = m;
        self.active_actions[k.row()][k.column()] = aa;
        if k.is_down() {
            self.run_action(action, true);
        } else {
            let mut sk = self.last_scan_key;
            sk.0.set_down(false);
            if self.push_scan_key(&sk) {
                self.run_action(action, true);
            }
        }
        self.flush_modifiers(false);
    }

    fn next_macro_seq(&mut self, mut location: u32, mut rem: u16, mode: macros::SequenceMode) {
        let stack = self.layout.macro_stack();

        while self.room_to_report() {
            let tap = self.layout.macro_code(location as usize);

            if rem > 1 {
                location += 1;
                rem -= 1;
                self.macro_running = Macro::Sequence {
                    mode,
                    location,
                    rem,
                };
            } else {
                self.pop_macro();
            }
            match mode {
                macros::SequenceMode::Hold => {
                    self.run_action(tap, true);
                }
                macros::SequenceMode::Release => {
                    self.run_action(tap, false);
                }
                macros::SequenceMode::Tap => {
                    self.run_action(tap, true);
                    self.run_action(tap, false);
                }
            }
            self.flush_modifiers(false);
            if rem == 0 || self.layout.macro_stack() != stack {
                break;
            }
        }
        self.set_wait_time();
    }

    fn modifier(&mut self, key: u16, is_down: bool) {
        let idx = (key - key_range::MODIFIER_MIN) as usize;
        let layer = match idx {
            idx if idx < 4 => idx,
            6 => 4,
            idx => idx - 4,
        };
        if is_down {
            if layer != idx && idx != 6 {
                self.layout.push_right_mod_layer(layer as u16);
            } else {
                self.layout.push_layer(layer as u16);
            }
            self.write_modifiers(1 << idx, 1, false);
        } else {
            self.layout.pop_layer(layer as u16);
            self.write_modifiers(1 << idx, -1, false);
        };
    }

    fn layer(&mut self, key: u16, is_down: bool) {
        self.report_count = self.report_count.wrapping_add(1);
        let base = key_range::base_code(key);
        let layern = key - base;

        if layern > key_range::MAX_LAYER_N {
            crate::error!("Layer out of range {}", layern);
            return;
        }
        match base {
            key_range::LAYER => {
                if is_down {
                    self.push_layer(layern);
                } else {
                    self.pop_layer(layern);
                }
            }
            key_range::TOGGLE => {
                if is_down && !self.pop_layer(layern) {
                    self.push_layer(layern);
                }
            }
            key_range::SET_LAYOUT => {
                if is_down {
                    self.set_layout(layern);
                }
            }
            key_range::ONESHOT => {
                if is_down {
                    self.push_layer(layern);
                } else {
                    self.oneshot(layern);
                }
            }
            _ => {
                unimplemented!("unimplemented layer action: {}", base);
            }
        }
    }

    fn oneshot(&mut self, layern: u16) {
        self.oneshot = Oneshot::WaitUp(layern);
    }

    fn set_layout(&mut self, layern: u16) {
        self.layout.set_layout(layern);
    }

    fn push_layer(&mut self, layern: u16) {
        if self.layout.push_layer(layern) {
            self.write_modifiers(self.layout.get_layer(layern).unwrap().modifiers(), 1, false);
        }
    }

    fn pop_layer(&mut self, layern: u16) -> bool {
        if self.layout.pop_layer(layern) {
            self.write_modifiers(
                self.layout.get_layer(layern).unwrap().modifiers(),
                -1,
                false,
            );
            true
        } else {
            false
        }
    }

    /// Change the state of a modifier keeping count by accumulating `count` for each modifier. Only reports
    /// the state if crosses the 0 to 1 threshold. If pending is true then buffer the report until flushed
    /// either by another report or [flush_modifiers].
    ///
    /// If the state will overlow the i8 counter a special state is entered where any -ve `count` will set
    /// the counter to underflow and a +ve `count` at underflow will set it to `count`.
    fn write_modifiers(&mut self, modifiers: u8, count: i8, pending: bool) {
        if modifiers == 0 {
            return;
        }
        let mut pmods = 0;
        let down = count > 0;

        let mut changed = 0;
        let mut bits = modifiers;
        for i in 0..8 {
            if bits & 1 == 1 {
                let before = self.modifier_count[i] > 0;
                if !down && self.modifier_count[i] == i8::MAX {
                    self.modifier_count[i] = i8::MIN;
                } else if down && self.modifier_count[i] == i8::MIN {
                    self.modifier_count[i] = count;
                } else {
                    self.modifier_count[i] = self.modifier_count[i]
                        .checked_add(count)
                        .unwrap_or(if down { i8::MAX } else { i8::MIN });
                }
                if before != (self.modifier_count[i] > 0) {
                    pmods |= 1 << i;
                    changed += 1;
                }
            }
            bits >>= 1;
            if bits == 0 {
                if changed != 0 {
                    if pending {
                        if down {
                            self.pending_down_modifiers |= pmods;
                        } else {
                            self.pending_up_modifiers |= pmods;
                        }
                    } else if changed == 1 {
                        self.report(KeyEvent::basic(
                            key_range::MODIFIER_MIN as u8 + i as u8,
                            down,
                        ));
                    } else {
                        self.report(KeyEvent::modifiers(modifiers, down, pending));
                    }
                }
                return;
            }
        }
    }

    fn set_wait_time(&mut self) {
        let mut t = min(self.mouse.next_event_time(), self.dual_action.wait_until());
        if self.macro_running != Macro::Noop {
            t = min(t, self.now);
        } else if self.tapdance.is_running() {
            t = min(t, self.tapdance.wait_until);
        }

        if t != self.wait_time {
            self.wait_time = t;
            self.report_channel.timer().at(if t == u64::MAX {
                Instant::MAX
            } else {
                Instant::from_millis(t)
            });
        } else if t != u64::MAX {
            self.report_channel.timer().at(Instant::from_millis(t));
        }
    }

    fn room_to_report(&self) -> bool {
        self.report_channel.0.free_capacity() >= MIN_REPORT_BUFFER_SIZE
    }

    async fn wait_for_report_capacity(&self) {
        for _ in 0..10 {
            if self.room_to_report() {
                break;
            }
            Timer::after_millis(16).await;
        }
    }

    fn start_dual_action(&mut self, is_down: bool, tap: u16, hold: u16, time1: u16, time2: u16) {
        if is_down {
            if let DualActionTimer::Wait { hold, .. } = self.dual_action {
                self.run_action(hold, true);
                self.dual_action = DualActionTimer::NoDual;
            }
            let (time1, time2) = if time1 == u16::MAX {
                (
                    self.layout.global(globals::DUAL_ACTION_TIMEOUT as usize),
                    self.layout.global(globals::DUAL_ACTION_TIMEOUT2 as usize),
                )
            } else if time2 == u16::MAX {
                (
                    time1,
                    self.layout.global(globals::DUAL_ACTION_TIMEOUT2 as usize),
                )
            } else {
                (time1, time2)
            };

            self.dual_action
                .start(self.last_scan_key, tap, hold, time1, time2);
        } else {
            self.run_action(hold, false);
        }
        self.flush_modifiers(false);
        self.set_wait_time();
    }

    fn push_scan_key(&mut self, p2key: &TimedScanKey) -> bool {
        let memo = p2key.as_memo();
        if !self.layout.push_memo(&memo) {
            self.clear_all();
            false
        } else {
            self.memo_count += 1;
            true
        }
    }

    fn push_action(&mut self, tap: u16, is_down: bool) -> bool {
        if !self.layout.push_memo(&[tap, if is_down { 1 } else { 0 }]) {
            self.clear_all();
            false
        } else {
            self.memo_count += 1;
            true
        }
    }

    fn run_memo(&mut self) -> bool {
        if self.dual_action.is_no_timer() && self.memo_count != 0 {
            self.memo_count -= 1;
            let mut scan_key = None;
            let mut action = None;
            self.layout.pop_memo(|memo| match memo.len() {
                2 => {
                    action = Some((memo[0], memo[1] != 0));
                }
                5 => {
                    scan_key = Some(TimedScanKey::from_memo(memo));
                }
                n => {
                    unreachable!("len {n}")
                }
            });
            if let Some(scan_key) = scan_key {
                self.key_switch(scan_key);
            } else if let Some((action, is_down)) = action {
                self.run_action(action, is_down);
                self.flush_modifiers(false);
            }
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
#[path = "mapper_test.rs"]
mod test;
