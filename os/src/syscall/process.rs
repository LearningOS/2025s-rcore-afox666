//! Process management syscalls
use crate::{mm::{translated_physaddr, MapPermission, PTEFlags, PageTable, VirtAddr}, task::{change_program_brk, current_user_token, exit_current_and_run_next, suspend_current_and_run_next, TASK_MANAGER}, timer::get_time_us};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
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

fn is_readable(va: usize) -> bool {
    let vpn = VirtAddr::from(va).floor();
    if let Some(pte) = PageTable::from_token(current_user_token()).translate(vpn) {
        pte.flags().contains(PTEFlags::U | PTEFlags::R)
    } else {
        false
    }
}

fn is_writeable(va: usize) -> bool {
    let vpn = VirtAddr::from(va).floor();
    if let Some(pte) = PageTable::from_token(current_user_token()).translate(vpn) {
        pte.flags().contains(PTEFlags::U | PTEFlags::W)
    } else {
        false
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(_trace_request: usize, _id: usize, _data: usize) -> isize {
    trace!("kernel: sys_trace");
    match _trace_request {
        0 => {
            if !is_readable(_id) {
                return -1;
            }
            let pa = translated_physaddr(current_user_token(), _id as *const u8);
            let phy_data = pa.0 as *mut u8;
            unsafe { (*phy_data).into() }
        },
        1 => {
            if !is_writeable(_id) {
                return -1;
            }
            let pa = translated_physaddr(current_user_token(), _id as *const u8);
            let phy_data = pa.0 as *mut u8; 
            unsafe { *phy_data = _data as u8 ; };
            return 0;
        },
        2 => TASK_MANAGER.get_syscall_counter(_id).try_into().unwrap(),
        _ => -1
    }
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

    if !TASK_MANAGER.find_area_insert(start_va, end_va, map_perm) {
        return -1;
    }
    return 0;
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap ");
    let start_va = VirtAddr::from(_start);
    if !start_va.aligned() {
        return -1;
    }
    let end_va: VirtAddr = (_start + _len).into();
    if !TASK_MANAGER.find_area_remove(start_va, end_va) {
        return -1;
    }
    return 0;
}
/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
