use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec::Vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);

        if process_inner.mutex_need.is_empty() {
            process_inner.mutex_need.push(Vec::new());
            process_inner.mutex_alloc.push(Vec::new());
        }

        process_inner.mutex_need[0].push(0);
        process_inner.mutex_alloc[0].push(0);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid,
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    if process_inner.is_dead_lock_detect_enabled() {
        debug!("start dead lock detect");
        process_inner.mutex_need[tid][mutex_id] += 1;
        let mut found = false;
        for (_tid, is_finish) in process_inner.finished.iter().enumerate() {
            if *is_finish {
                debug!("tid[{}] finished", _tid);
                continue;
            } else {
                let mut ok = true;
                for need in &process_inner.mutex_need[_tid] {
                    debug!(
                        "need = {:?}, locked = {:?}",
                        *need,
                        process_inner.mutex_list[mutex_id]
                            .as_ref()
                            .unwrap()
                            .is_locked()
                    );

                    if *need
                        <= process_inner.mutex_list[mutex_id]
                            .as_ref()
                            .unwrap()
                            .is_locked()
                    {
                        continue;
                    } else {
                        ok = false;
                        break;
                    }
                }

                if ok {
                    found = true;
                } else {
                    continue;
                }
            }
        }
        if !found {
            return -0xDEAD;
        }
    }

    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);

    mutex.lock();

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    if process_inner.is_dead_lock_detect_enabled() {
        process_inner.mutex_need[tid][mutex_id] -= 1;
    }
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    debug!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));

        if process_inner.semaphore_need.is_empty() {
            process_inner.semaphore_need.push(Vec::new());
            process_inner.semaphore_alloc.push(Vec::new());
        }

        process_inner.semaphore_alloc[0].push(0);
        process_inner.semaphore_need[0].push(0);
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    process_inner.finished[tid] = true;
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    let tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    debug!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down semid[{}]",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid,
        sem_id
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());

    // debug!(
    //     "process_inner.semaphore_need.len() = {},process_inner.semaphore_need[0].len() = {}",
    //     process_inner.semaphore_need.len(),
    //     process_inner.semaphore_need[0].len()
    // );

    process_inner.semaphore_need[tid][sem_id] += 1;
    let mut found = false;
    if process_inner.is_dead_lock_detect_enabled() {
        for (_tid, is_finish) in process_inner.finished.iter().enumerate() {
            if *is_finish {
                // debug!("tid[{}] finished", _tid);
                continue;
            } else {
                let mut ok = true;
                for (_sem_id, need) in process_inner.semaphore_need[_tid].iter().enumerate() {
                    if _sem_id == 0 {
                        continue;
                    }

                    let sem_count = process_inner.semaphore_list[_sem_id]
                        .as_ref()
                        .unwrap()
                        .sem_count();

                    let mut res_num = sem_count;
                    if res_num < 0 {
                        res_num = 0;
                    }

                    debug!(
                        "tid=={},sem_id=={},need = {:?},count = {:?}",
                        _tid, _sem_id, *need, res_num
                    );

                    if *need <= res_num {
                        continue;
                    } else {
                        ok = false;
                        break;
                    }
                }

                if ok {
                    found = true;
                    break;
                } else {
                    continue;
                }
            }
        }
        if !found {
            debug!(
                "kernel:pid[{}] tid[{}] sys_semaphore_down fail",
                current_task().unwrap().process.upgrade().unwrap().getpid(),
                tid
            );
            return -0xDEAD;
        }
    }

    drop(process_inner);

    debug!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down success",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );

    sem.down();
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();

    process_inner.semaphore_need[tid][sem_id] -= 1;
    debug!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down return",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        tid
    );
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// YOUR JOB: Implement deadlock detection, but might not all in this syscall
pub fn sys_enable_deadlock_detect(_enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect");
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    match _enabled {
        0 => {
            process_inner.dead_lock_detect = false;
            return 0;
        }
        1 => {
            process_inner.dead_lock_detect = true;
            return 0;
        }
        _ => {
            return -1;
        }
    }
}
