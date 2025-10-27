use core::sync::atomic::AtomicU16;

use embassy_sync::blocking_mutex::raw::NoopRawMutex;

use crate::{config, key_scanner, mapper, ring_fs};

pub async fn run<
    'd,
    const ROW_COUNT: usize,
    const COL_COUNT: usize,
    const LAYOUT_MAX: usize,
    const SCANNER_BUFFER_SIZE: usize,
    const REPORT_BUFFER_SIZE: usize,
>(
    layout_mapping: &'d [u16],
    key_scan_channel: &'d key_scanner::KeyScannerChannel<NoopRawMutex, SCANNER_BUFFER_SIZE>,
    mapper_channel: &'d mapper::MapperChannel<NoopRawMutex, REPORT_BUFFER_SIZE>,
    fs: &'d dyn ring_fs::RingFs<'d>,
    debounce_ms_atomic: &'d AtomicU16,
) {
    let mut mapper =
        mapper::Mapper::<'d, ROW_COUNT, COL_COUNT, LAYOUT_MAX, _, REPORT_BUFFER_SIZE>::new(
            mapper_channel,
            debounce_ms_atomic,
        );
    {
        if !match fs.file_reader_by_index(0) {
            Ok(fr) => {
                if let Err(err) = mapper.load_layout(config::ConfigFileIter::new(fr)) {
                    crate::info!("error loading layout {:?}", err);
                    false
                } else {
                    true
                }
            }
            Err(err) => {
                crate::info!("error reading layout {:?}", err);
                false
            }
        } && let Err(err) = mapper.load_layout(layout_mapping.iter().copied())
        {
            crate::info!("unexpected error loading layout {:?}", err);
        }
    }

    loop {
        if let mapper::ControlMessage::LoadLayout { file_location } =
            mapper.run(key_scan_channel).await
        {
            crate::debug!("load layout here {}", file_location);
            match fs.file_reader_by_location(file_location) {
                Ok(fr) => {
                    if let Err(err) = mapper.load_layout(config::ConfigFileIter::new(fr)) {
                        crate::info!("error loading layout {:?}", err);
                        mapper.load_layout(layout_mapping.iter().copied()).unwrap();
                    }
                }
                Err(err) => crate::info!("error reading layout {:?}", err),
            }
        }
    }
}
