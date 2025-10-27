use core::sync::atomic::{AtomicUsize, Ordering};
use embassy_usb::{
    class::hid::{ReadError, ReportId, RequestHandler},
    driver::{Driver, Endpoint, EndpointError, EndpointIn, EndpointOut},
};

use crate::warn;

pub struct HidWriter<'d, D: Driver<'d>, const N: usize> {
    ep_in: D::EndpointIn,
}

impl<'d, D: Driver<'d>, const N: usize> HidWriter<'d, D, N> {
    pub fn new(ep_in: <D>::EndpointIn) -> Self {
        Self { ep_in }
    }

    /// Writes `report` to its interrupt endpoint.
    pub async fn write(&mut self, report: &[u8]) -> Result<(), EndpointError> {
        assert!(report.len() <= N);

        let max_packet_size = usize::from(self.ep_in.info().max_packet_size);
        let zlp_needed = report.len() < N && report.len().is_multiple_of(max_packet_size);
        for chunk in report.chunks(max_packet_size) {
            self.ep_in.write(chunk).await?;
        }

        if zlp_needed {
            self.ep_in.write(&[]).await?;
        }

        Ok(())
    }
}

pub struct HidReader<'d, D: Driver<'d>, const N: usize> {
    ep_out: D::EndpointOut,
    offset: &'d AtomicUsize,
}

impl<'d, D: Driver<'d>, const N: usize> HidReader<'d, D, N> {
    pub fn new(ep_out: <D>::EndpointOut, offset: &'d AtomicUsize) -> Self {
        Self { ep_out, offset }
    }

    /// Delivers output reports from the Interrupt Out pipe to `handler`.
    ///
    /// If `use_report_ids` is true, the first byte of the report will be used as
    /// the `ReportId` value. Otherwise the `ReportId` value will be 0.
    pub async fn run<T: RequestHandler>(mut self, use_report_ids: bool, handler: &mut T) -> ! {
        let offset = self.offset.load(Ordering::Acquire);
        assert!(offset == 0);
        let mut buf = [0; N];
        loop {
            match self.read(&mut buf).await {
                Ok(len) => {
                    let id = if use_report_ids { buf[0] } else { 0 };
                    handler.set_report(ReportId::Out(id), &buf[..len]);
                }
                Err(ReadError::BufferOverflow) => {
                    warn!(
                        "Host sent output report larger than the configured maximum output report length ({})",
                        N
                    );
                }
                Err(ReadError::Disabled) => self.ep_out.wait_enabled().await,
                Err(ReadError::Sync(_)) => unreachable!(),
            }
        }
    }

    /// Reads an output report from the Interrupt Out pipe.
    ///
    /// **Note:** Any reports sent from the host over the control pipe will be
    /// passed to [`RequestHandler::set_report()`] for handling. The application
    /// is responsible for ensuring output reports from both pipes are handled
    /// correctly.
    ///
    /// **Note:** If `N` > the maximum packet size of the endpoint (i.e. output
    /// reports may be split across multiple packets) and this method's future
    /// is dropped after some packets have been read, the next call to `read()`
    /// will return a [`ReadError::Sync`]. The range in the sync error
    /// indicates the portion `buf` that was filled by the current call to
    /// `read()`. If the dropped future used the same `buf`, then `buf` will
    /// contain the full report.
    pub async fn read(&mut self, buf: &mut [u8]) -> Result<usize, ReadError> {
        assert!(N != 0);
        assert!(buf.len() >= N);

        // Read packets from the endpoint
        let max_packet_size = usize::from(self.ep_out.info().max_packet_size);
        let starting_offset = self.offset.load(Ordering::Acquire);
        let mut total = starting_offset;
        loop {
            for chunk in buf[starting_offset..N].chunks_mut(max_packet_size) {
                match self.ep_out.read(chunk).await {
                    Ok(size) => {
                        total += size;
                        if size < max_packet_size || total == N {
                            self.offset.store(0, Ordering::Release);
                            break;
                        }
                        self.offset.store(total, Ordering::Release);
                    }
                    Err(err) => {
                        self.offset.store(0, Ordering::Release);
                        return Err(err.into());
                    }
                }
            }

            // Some hosts may send ZLPs even when not required by the HID spec, so we'll loop as long as total == 0.
            if total > 0 {
                break;
            }
        }

        if starting_offset > 0 {
            Err(ReadError::Sync(starting_offset..total))
        } else {
            Ok(total)
        }
    }
}
