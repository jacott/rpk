use std::{
    sync::{Arc, Mutex},
    thread::spawn,
};

use super::*;

type BulkData = Vec<(u8, Vec<u8>)>;

#[derive(Default)]
struct TestInterface {
    out: Arc<Mutex<BulkData>>,
    inp: Arc<Mutex<BulkData>>,
}
impl TestInterface {
    fn add_in(&self, ep: u8, vec: Vec<u8>) {
        let mut guard = self.inp.lock().unwrap();
        guard.insert(0, (ep, vec));
    }

    fn get_out(&self) -> BulkData {
        let guard = self.out.lock().unwrap();
        guard.clone()
    }
}
impl KeyboardInterface for TestInterface {
    fn bulk_out(&self, endpoint: u8, buf: Vec<u8>) -> Result<()> {
        let mut guard = self.out.lock().unwrap();
        guard.push((endpoint, buf));
        Ok(())
    }

    fn bulk_in(&self, endpoint: u8, max_len: u16) -> Result<Vec<u8>> {
        let mut guard = self.inp.lock().unwrap();
        let (ep, msg) = guard.pop().unwrap_or((endpoint, vec![]));
        assert_eq!(ep, endpoint);
        assert!(max_len as usize > msg.len());
        Ok(msg)
    }
}

#[test]
fn file_info_from() {
    let now = Utc::now();
    let mut data = vec![];
    data.extend_from_slice(&(54321u32).to_le_bytes());
    data.extend_from_slice(&(123u32).to_le_bytes());
    data.extend_from_slice(&(now.timestamp_micros() / 1000).to_le_bytes());
    data.push(FileType::Config.as_u8());
    data.push(5);
    data.extend_from_slice(b"file1notthis");

    let ans = FileInfo::from(data.as_slice());
    assert_eq!((now - ans.timestamp).num_milliseconds(), 0);
    assert_eq!(ans.length, 123);
    assert_eq!(ans.location, 54321);
    assert_eq!(ans.index, 0);
    assert!(matches!(ans.file_type, FileType::Config));
    assert_eq!(ans.filename, "file1");
}

fn new_ctl() -> Arc<KeyboardCtl<TestInterface>> {
    Arc::new(KeyboardCtl::<TestInterface> {
        epout: 1,
        epin: 2,
        intf: Default::default(),
        handlers: Default::default(),
    })
}

#[test]
fn list_files() {
    let ctl = new_ctl();

    let ctl2 = ctl.clone();
    spawn(move || {
        ctl2.listen();
    });

    let mut data = vec![host_recv::FILE_INFO, 1, 2, 3, 50, 0, 0, 0, 1, 2, 3, 4];
    data.extend_from_slice(b"filename");

    ctl.intf.add_in(2, data.clone());
    ctl.intf.add_in(2, data);
    ctl.intf.add_in(2, vec![0]);

    let files: Vec<FileInfo> = ctl.list_files().collect();

    assert_eq!(files.len(), 2);
    let out = ctl.intf.get_out();
    assert_eq!(out.len(), 3);
    assert_eq!(out[0].0, 1);
    assert_eq!(out[1].1, vec![5, 1, 0, 0, 0]);
}

#[test]
fn stats() {
    let ctl = new_ctl();

    let ctl2 = ctl.clone();
    spawn(move || {
        ctl2.listen();
    });

    let mut msg = vec![host_recv::STATS];
    let uptime = 123456789u32;
    msg.extend_from_slice(&uptime.to_le_bytes());
    ctl.intf.add_in(2, msg);

    let stats = ctl.fetch_stats().unwrap();

    let out = ctl.intf.get_out();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].0, 1);
    assert_eq!(out[0].1, vec![6]);

    assert_eq!(stats.uptime, Duration::from_millis(uptime as u64));
}
