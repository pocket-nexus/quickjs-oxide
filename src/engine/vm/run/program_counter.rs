//! Keep resume local while fault stays directly observable in the frame.
//! Frame fault writes stay at their actual dispatch/advancement sites.
//! Drop publishes resume on normal, Result-error and Rust-unwind exits.
//! Runtime active-PC publication remains exclusively in the existing driver.

pub(super) struct ProgramCounter<'a> {
    pub resume: usize,
    published_resume: &'a mut usize,
}
impl<'a> ProgramCounter<'a> {
    pub fn new(resume: &'a mut usize) -> Self {
        Self {
            resume: *resume,
            published_resume: resume,
        }
    }
}
impl Drop for ProgramCounter<'_> {
    fn drop(&mut self) {
        *self.published_resume = self.resume;
        #[cfg(feature = "profiling")]
        crate::engine::api::profiling::record_owned_execution_event("run_frame_resume_pc_write");
    }
}

#[cfg(test)]
mod hybrid_pc_tests {
    use super::ProgramCounter;

    #[test]
    fn hybrid_pc_preserves_both_values_on_result_error_and_unwind() {
        let mut fault = 3;
        let mut resume = 4;
        let result: Result<(), ()> = (|| {
            let mut pc = ProgramCounter::new(&mut resume);
            fault = 11;
            pc.resume = 12;
            Err(())
        })();
        assert!(result.is_err());
        assert_eq!((fault, resume), (11, 12));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut pc = ProgramCounter::new(&mut resume);
            fault = 21;
            pc.resume = 22;
            panic!("hybrid PC unwind probe");
        }));
        assert!(result.is_err());
        assert_eq!((fault, resume), (21, 22));
    }
}
