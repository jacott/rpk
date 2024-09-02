use embassy_futures::block_on;
use embassy_time::{Duration, Instant};

use crate::test::usb_test_stub::{MyDriver, MyEndpointIn};

use super::*;

extern crate alloc;
use alloc::vec;

macro_rules! setup {
    ($messages:ident, $rep:ident, $x:tt) => {
        block_on(async {
            let ep_in = MyEndpointIn::default();
            let $messages = ep_in.messages.clone();
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
        let guard = messages.lock().unwrap();
        assert_eq!(&guard[0], &vec![1, 2, 3]);
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

        let guard = messages.lock().unwrap();
        assert_eq!(&guard[0][..5], &vec![6, 0, 16, 0, 0]);
        assert_eq!(&guard[1][..5], &vec![6, 4, 16, 0, 0]);
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

        let guard = messages.lock().unwrap();
        assert_eq!(guard[0].len(), 34);
        assert_eq!(&guard[1][..5], &vec![6, 0, 48, 0, 0]);
        assert_eq!(&guard[2][..5], &vec![6, 4, 48, 0, 0]);
        assert_eq!(&guard[3][..5], &vec![6, 4, 32, 0, 0]);
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

        let guard = messages.lock().unwrap();
        assert_eq!(guard.len(), 9);
        assert_eq!(guard[0].len(), 34);
        assert_eq!(&guard[0][..5], &vec![6, 6, 0, 0, 0]);
        assert_eq!(&guard[1][..5], &vec![6, 14, 0, 0, 0]);
        assert_eq!(&guard[2][..5], &vec![6, 12, 0, 0, 0]);
        assert_eq!(&guard[3][..5], &vec![6, 14, 0, 0, 1]);
        assert_eq!(&guard[4][..5], &vec![6, 11, 0, 0, 1]);
        assert_eq!(&guard[5], &vec![2, 0, 0, 0, 0, 0]);
        assert_eq!(&guard[6], &vec![3, 0, 0]);
        assert_eq!(&guard[7], &vec![4, 0, 0]);
        assert_eq!(&guard[8][..5], &vec![6, 0, 0, 0, 0]);
    });
}

#[test]
fn consumer_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::Consumer(361)).await;
        reporter.report(KeyEvent::Consumer(104)).await;
        reporter.report(KeyEvent::Consumer(0)).await;

        let guard = messages.lock().unwrap();
        assert_eq!(guard[0].len(), 3);
        assert_eq!(&guard[0], &vec![4, 105, 1]);
        assert_eq!(&guard[1], &vec![4, 104, 0]);
        assert_eq!(&guard[2], &vec![4, 0, 0]);
    });
}

#[test]
fn sys_ctl_report() {
    setup!(messages, reporter, {
        reporter.report(KeyEvent::SysCtl(361)).await;
        reporter.report(KeyEvent::SysCtl(104)).await;
        reporter.report(KeyEvent::SysCtl(0)).await;

        let guard = messages.lock().unwrap();
        assert_eq!(guard[0].len(), 3);
        assert_eq!(&guard[0], &vec![3, 105, 1]);
        assert_eq!(&guard[1], &vec![3, 104, 0]);
        assert_eq!(&guard[2], &vec![3, 0, 0]);
    });
}
