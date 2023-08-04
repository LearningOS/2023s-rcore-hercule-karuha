//!Implementation of [`TaskManager`]
use super::TaskControlBlock;
use crate::sync::UPSafeCell;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use lazy_static::*;
///A array of `TaskControlBlock` that is thread-safe
pub struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

/// A simple FIFO scheduler.
impl TaskManager {
    ///Creat an empty TaskManager
    pub fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    /// Add process back to ready queue
    pub fn add(&mut self, task: Arc<TaskControlBlock>) {
        self.ready_queue.push_back(task);
    }
    /// Take a process out of the ready queue
    pub fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        // self.ready_queue.pop_front()
        let mut min_stride_tcb: Option<Arc<TaskControlBlock>> = None;
        let mut min_stride = u32::MAX;

        // Iterate through the ready queue to find the TaskControlBlock with the smallest stride
        for tcb in &self.ready_queue {
            let stride = tcb.inner_exclusive_access().stride;
            if stride < min_stride {
                min_stride = stride;
                min_stride_tcb = Some(tcb.clone());
            }
        }

        // If we found a TaskControlBlock with the smallest stride, remove it from the ready queue
        if let Some(tcb) = min_stride_tcb {
            let index = self
                .ready_queue
                .iter()
                .position(|item| Arc::ptr_eq(item, &tcb));
            if let Some(index) = index {
                return self.ready_queue.remove(index);
            }
        }

        None
    }
}

lazy_static! {
    /// TASK_MANAGER instance through lazy_static!
    pub static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

/// Add process to ready queue
pub fn add_task(task: Arc<TaskControlBlock>) {
    //trace!("kernel: TaskManager::add_task");
    TASK_MANAGER.exclusive_access().add(task);
}

/// Take a process out of the ready queue
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    //trace!("kernel: TaskManager::fetch_task");
    TASK_MANAGER.exclusive_access().fetch()
}
