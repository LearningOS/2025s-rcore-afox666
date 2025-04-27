//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    loader::get_app_data_by_name, mm::{translated_physaddr, translated_refmut, translated_str, MapPermission, VirtAddr}, task::{add_task, current_task, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TaskControlBlock}, timer::get_time_us
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel:pid[{}] sys_exit", current_task().unwrap().pid.0);
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel:pid[{}] sys_yield", current_task().unwrap().pid.0);
    suspend_current_and_run_next();
    0
}

pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid pid:{}", current_task().unwrap().pid.0);
    current_task().unwrap().pid.0 as isize
}

pub fn sys_fork() -> isize {
    trace!("kernel:pid[{}] sys_fork", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.pid.0;
    // modify trap context of new_task, because it returns immediately after switching
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    // we do not have to move to next instruction since we have done it before
    // for child process, fork returns 0
    trap_cx.x[10] = 0;
    // add new task to scheduler
    add_task(new_task);
    new_pid as isize
}

pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let token = current_user_token();
    let path = translated_str(token, path);
    if let Some(data) = get_app_data_by_name(path.as_str()) {
        let task = current_task().unwrap();
        task.exec(data);
        0
    } else {
        -1
    }
}

/// If there is not a child process whose pid is same as given, return -1.
/// Else if there is a child process but it is still running, return -2.
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel::pid[{}] sys_waitpid [{}]", current_task().unwrap().pid.0, pid);
    let task = current_task().unwrap();
    // find a child process

    // ---- access current PCB exclusively
    let mut inner = task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.getpid())
    {
        debug!("no such pid[{}]", pid);
        return -1;
        // ---- release current PCB
    }
    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        // ++++ temporarily access child PCB exclusively
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.getpid())
        // ++++ release child PCB
    });
    if let Some((idx, _)) = pair {
        let child = inner.children.remove(idx);
        // confirm that child will be deallocated after being removed from children list
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.getpid();
        // ++++ temporarily access child PCB exclusively
        let exit_code = child.inner_exclusive_access().exit_code;
        // ++++ release child PCB
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        debug!("found pid[{}]", found_pid);
        found_pid as isize
    } else {
        // debug!("pid[{}] is running", pid);
        -2
    }
    // ---- release current PCB automatically
}

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let pa = translated_physaddr(current_user_token(), _ts as *const u8);
    let phy_ts = pa.0 as *mut TimeVal;

    let us = get_time_us();
    let time_val = TimeVal {
        sec : us / 1_000_000,
        usec: us % 1_000_000,
    };

    unsafe { *phy_ts = time_val };
    0
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct Flag(pub usize);

impl Flag {
    pub fn is_readable(&self) -> bool {
        self.0 & 0x1 == 0x1
    }

    pub fn is_writeable(&self) -> bool {
        self.0 & 0x2 == 0x2
    }

    pub fn is_execute(&self) -> bool {
        self.0 & 0x4 == 0x4
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap ");

    debug!("mmap start: {}, len: {}, port: {}", _start, _len, _port);

    let start_va = VirtAddr::from(_start);
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (_start + _len).into();
    if _port & !0x7 != 0 || _port & 0x7 == 0 {
        return -1;
    }

    let mut map_perm: MapPermission = MapPermission::U;
    let pte_flag = Flag(_port);
    if pte_flag.is_readable() {
        map_perm |= MapPermission::R;
    }
    if pte_flag.is_writeable() {
        map_perm |= MapPermission::W;
    }
    if pte_flag.is_execute() {
        map_perm |= MapPermission::X;
    }

    if !current_task().unwrap().find_area_insert(start_va, end_va, map_perm) {
        return -1;
    }
    return 0;
}

/// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap ");
    let start_va = VirtAddr::from(_start);
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (_start + _len).into();

    if !current_task().unwrap().find_area_remove(start_va, end_va) {
        return -1;
    }
    return 0;
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel:pid[{}] sys_sbrk", current_task().unwrap().pid.0);
    if let Some(old_brk) = current_task().unwrap().change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// YOUR JOB: Implement spawn.
/// HINT: fork + exec =/= spawn
pub fn sys_spawn(_path: *const u8) -> isize {
    trace!(
        "kernel:pid[{}] sys_spawn NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    let token = current_user_token();
    let path = translated_str(token, _path);
    if let Some(data) = get_app_data_by_name(&path) {
        let new_task: Arc<TaskControlBlock> = current_task().unwrap().spawn(data);
        let pid = new_task.getpid() as isize;
        add_task(new_task);
        debug!("add task pid[{}]", pid);
        return pid;
    } else {
        debug!("kernel: sys spawn error...");
        return -1;
    }
}

// YOUR JOB: Set task priority.
pub fn sys_set_priority(_prio: isize) -> isize {
    trace!(
        "kernel:pid[{}] sys_set_priority NOT IMPLEMENTED",
        current_task().unwrap().pid.0
    );
    if _prio < 2 {
        return -1;
    }
    current_task().unwrap().set_priority(_prio as usize);
    return _prio;
}
