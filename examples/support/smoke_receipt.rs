//! Owned public smoke completion. A closed window or an old report is not PASS.
use std::{
    ffi::OsStr,
    fs::{File, OpenOptions},
    io,
    path::Path,
    sync::mpsc::SyncSender,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Control {
    Normal,
    ForceFail,
    MissingReceipt,
}
impl Control {
    pub(crate) fn parse(value: Option<&OsStr>, has_report: bool) -> Result<Self, &'static str> {
        match value {
            None => Ok(Self::Normal),
            Some(_) if !has_report => {
                Err("Public smoke control requires an explicit smoke report path")
            }
            Some(v) if v == "fail" => Ok(Self::ForceFail),
            Some(v) if v == "missing-receipt" => Ok(Self::MissingReceipt),
            Some(_) => Err("Unknown public smoke control"),
        }
    }
}
pub(crate) struct Receipt {
    file: File,
    control: Control,
    result: SyncSender<bool>,
}
impl Receipt {
    pub(crate) fn create(
        path: &Path,
        control: Control,
        result: SyncSender<bool>,
    ) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::fs::OpenOptionsExt as _;
            options.share_mode(1);
        }
        // Reserve once before native/WebView creation, never truncate or reopen
        // the requested path. Keep the original owned handle through completion.
        Ok(Self {
            file: options.open(path)?,
            control,
            result,
        })
    }
    pub(crate) fn qualify_pass(&self, passed: bool) -> bool {
        passed && self.control == Control::Normal
    }
    pub(crate) fn forced_failure(&self) -> bool {
        self.control == Control::ForceFail
    }
    pub(crate) fn finish(mut self, contents: &[u8], passed: bool) -> io::Result<()> {
        if self.control == Control::MissingReceipt {
            // Leave an empty owned file, send no verdict; the real post-window
            // path must reject this, rather than accepting a normal native exit.
            return Ok(());
        }
        let written = write_receipt(&mut self.file, contents);
        let _delivery = self
            .result
            .try_send(self.qualify_pass(passed) && written.is_ok());
        written
    }
}
fn write_receipt(out: &mut impl io::Write, contents: &[u8]) -> io::Result<()> {
    out.write_all(contents)?;
    out.flush()
}
pub(crate) fn require_pass(verdict: Option<bool>) -> Result<(), &'static str> {
    match verdict {
        Some(true) => Ok(()),
        Some(false) => Err("Public smoke failed or its receipt could not be completed"),
        None => Err("Public smoke ended without a completed receipt"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, sync::mpsc};
    #[test]
    fn existing_report_bytes_and_time_are_preserved() {
        let owned = tempfile::tempdir().expect("owned fixture");
        let path = owned.path().join("report.json");
        let bytes = b"independently authored existing report";
        fs::write(&path, bytes).expect("owned existing file");
        let modified = fs::metadata(&path)
            .expect("metadata")
            .modified()
            .expect("time");
        let (sender, _receiver) = mpsc::sync_channel(1);
        assert!(Receipt::create(&path, Control::Normal, sender).is_err());
        assert_eq!(fs::read(&path).expect("unchanged"), bytes);
        assert_eq!(
            fs::metadata(&path)
                .expect("metadata")
                .modified()
                .expect("time"),
            modified
        );
    }
    #[test]
    fn completion_requires_success_and_writes_exact_owned_report() {
        let owned = tempfile::tempdir().expect("owned fixture");
        let path = owned.path().join("fresh.json");
        let (sender, receiver) = mpsc::sync_channel(1);
        Receipt::create(&path, Control::Normal, sender)
            .expect("fresh path")
            .finish(br#"{"assessment":"PASS"}"#, true)
            .expect("write and flush");
        assert!(require_pass(receiver.try_recv().ok()).is_ok());
        assert_eq!(
            fs::read(&path).expect("public report"),
            br#"{"assessment":"PASS"}"#
        );
    }
    #[test]
    fn failure_and_missing_receipt_propagate_through_owned_channel() {
        let owned = tempfile::tempdir().expect("owned fixture");
        for control in [Control::Normal, Control::ForceFail, Control::MissingReceipt] {
            let path = owned.path().join(if control == Control::Normal {
                "failed.json"
            } else if control == Control::ForceFail {
                "forced.json"
            } else {
                "missing.json"
            });
            let (sender, receiver) = mpsc::sync_channel(1);
            let receipt = Receipt::create(&path, control, sender).expect("owned path");
            let passed = receipt.qualify_pass(control != Control::Normal);
            assert!(!passed, "normal FAIL and forced controls cannot qualify");
            receipt
                .finish(br#"{"assessment":"FAIL"}"#, passed)
                .expect("owned report finish");
            assert!(require_pass(receiver.try_recv().ok()).is_err());
            if control == Control::MissingReceipt {
                assert_eq!(fs::metadata(path).expect("owned empty receipt").len(), 0);
            }
        }
    }
    #[test]
    fn invalid_parent_is_refused_without_creating_directories() {
        let owned = tempfile::tempdir().expect("owned fixture");
        let absent = owned.path().join("absent");
        let (sender, _receiver) = mpsc::sync_channel(1);
        assert!(Receipt::create(&absent.join("report.json"), Control::Normal, sender).is_err());
        assert!(!absent.exists());
    }
    #[test]
    fn failed_flush_is_not_successful_receipt_io() {
        struct BrokenFlush;
        impl io::Write for BrokenFlush {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
        }
        assert!(write_receipt(&mut BrokenFlush, b"public report").is_err());
    }
    #[test]
    fn negative_controls_require_explicit_report_mode() {
        assert!(Control::parse(Some(OsStr::new("fail")), false).is_err());
        assert!(Control::parse(Some(OsStr::new("unknown")), true).is_err());
        assert!(
            Control::parse(Some(OsStr::new("fail")), true).is_ok_and(|c| c == Control::ForceFail)
        );
        assert!(
            Control::parse(Some(OsStr::new("missing-receipt")), true)
                .is_ok_and(|c| c == Control::MissingReceipt)
        );
    }
}
