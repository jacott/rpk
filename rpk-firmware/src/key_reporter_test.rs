use embassy_futures::block_on;
use embassy_time::{Duration, Instant};

use crate::usb_test_stub::{MyDriver, MyEndpointIn};

use super::*;

extern crate alloc;
use alloc::vec;

macro_rules! setup {
    ($messages:ident, $rep:ident, $x:tt) => {
        block_on(async {
            let ep_in = MyEndpointIn::default();
            let $messages = &ep_in.messages.clone();
            let hid_writer = HidWriter::<'_, MyDriver, 34>::new(ep_in);
            let mut $rep = Reporter::new(hid_writer);

            $x
        });
    };
}

#[test]
fn write_report() {
    setup!(messages, reporter, {
        reporter.write_report(&[1, 2, 3]).await;
        assert_eq!(&messages.get(), &vec![1, 2, 3]);
    });
}

#[test]
fn delay() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::Basic(4, true)).await;
        let start = Instant::now();
        reporter.report(KeyEvent::Delay(1)).await;
        let d = Instant::now() - start;
        assert!(d >= Duration::from_millis(1));
        reporter.report(KeyEvent::Basic(0xe2, true)).await;

        assert_eq!(&messages.get()[..5], &vec![6, 0, 16, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 4, 16, 0, 0]);
    });
}

#[test]
fn basic_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::Basic(5, true)).await;
        reporter.report(KeyEvent::Basic(4, true)).await;
        reporter.report(KeyEvent::Basic(0xe2, true)).await;
        reporter.report(KeyEvent::Basic(4, false)).await;
        reporter.report(KeyEvent::Basic(0xe2, true)).await;

        assert_eq!(messages.get().len(), 34);
        assert_eq!(&messages.get()[..5], &vec![6, 0, 48, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 4, 48, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 4, 32, 0, 0]);
    });
}

#[test]
fn modifiers_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::Modifiers(6, true)).await;
        reporter.report(KeyEvent::Modifiers(8, true)).await;
        reporter.report(KeyEvent::Modifiers(2, false)).await;
        reporter.report(KeyEvent::PendingModifiers(2, true)).await;
        reporter.report(KeyEvent::Basic(16, true)).await;
        reporter.report(KeyEvent::PendingModifiers(4, false)).await;
        reporter.report(KeyEvent::PendingModifiers(1, true)).await;
        reporter.report(KeyEvent::Pending).await;
        reporter.report(KeyEvent::Clear).await;

        let msg = messages.get();
        assert_eq!(msg.len(), 34);
        assert_eq!(&msg[..5], &vec![6, 6, 0, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 14, 0, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 12, 0, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 14, 0, 0, 1]);
        assert_eq!(&messages.get()[..5], &vec![6, 11, 0, 0, 1]);
        assert_eq!(&messages.get(), &vec![2, 0, 0, 0, 0, 0]);
        assert_eq!(&messages.get(), &vec![3, 0, 0]);
        assert_eq!(&messages.get(), &vec![4, 0, 0]);
        assert_eq!(&messages.get()[..5], &vec![6, 0, 0, 0, 0]);
    });
}

#[test]
fn consumer_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::Consumer(361)).await;
        reporter.report(KeyEvent::Consumer(104)).await;
        reporter.report(KeyEvent::Consumer(0)).await;

        let msg = messages.get();
        assert_eq!(msg.len(), 3);
        assert_eq!(&msg, &vec![4, 105, 1]);
        assert_eq!(&messages.get(), &vec![4, 104, 0]);
        assert_eq!(&messages.get(), &vec![4, 0, 0]);
    });
}

#[test]
fn sys_ctl_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::SysCtl(361)).await;
        reporter.report(KeyEvent::SysCtl(104)).await;
        reporter.report(KeyEvent::SysCtl(0)).await;

        let msg = messages.get();
        assert_eq!(msg.len(), 3);
        assert_eq!(&msg, &vec![3, 105, 1]);
        assert_eq!(&messages.get(), &vec![3, 104, 0]);
        assert_eq!(&messages.get(), &vec![3, 0, 0]);
    });
}
