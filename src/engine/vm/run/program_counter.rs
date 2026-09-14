//! Materialize both PCs when the continuous no-callback run borrow ends.
//! This includes errors from slot authentication/retain, cold operations,
//! returns and suspension. Runtime observation publication belongs to the driver.

pub(super) struct ProgramCounter<'a> {
    pub fault: usize,
    pub resume: usize,
    published_fault: &'a mut usize,
    published_resume: &'a mut usize,
}
impl<'a> ProgramCounter<'a> {
    pub fn new(fault: &'a mut usize, resume: &'a mut usize) -> Self {
        Self {
            fault: *fault,
            resume: *resume,
            published_fault: fault,
            published_resume: resume,
        }
    }
}
impl Drop for ProgramCounter<'_> {
    fn drop(&mut self) {
        *self.published_fault = self.fault;
        *self.published_resume = self.resume;
        #[cfg(feature = "profiling")]
        {
            crate::engine::api::profiling::record_owned_execution_event("run_frame_fault_pc_write");
            crate::engine::api::profiling::record_owned_execution_event(
                "run_frame_resume_pc_write",
            );
        }
    }
}
