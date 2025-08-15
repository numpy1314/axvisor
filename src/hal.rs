use std::os::arceos::{
    self,
    modules::{
        axhal::percpu::this_cpu_id,
        axtask::{self, TaskExtRef},
    },
};

use axerrno::{AxResult, ax_err_type};
use memory_addr::{PAGE_SIZE_4K, align_up_4k};
use page_table_multiarch::PagingHandler;

use arceos::modules::{axalloc, axhal};
use axaddrspace::{AxMmHal, HostPhysAddr, HostVirtAddr};
use axvcpu::AxVCpuHal;
use axvm::{AxVMHal, AxVMPerCpu};

use crate::vmm;

/// Implementation for `AxVMHal` trait.
pub struct AxVMHalImpl;

impl AxVMHal for AxVMHalImpl {
    type PagingHandler = axhal::paging::PagingHandlerImpl;

    fn alloc_memory_region_at(base: HostPhysAddr, size: usize) -> bool {
        axalloc::global_allocator()
            .alloc_pages_at(
                base.as_usize(),
                align_up_4k(size) / PAGE_SIZE_4K,
                PAGE_SIZE_4K,
            )
            .map_err(|err| {
                error!(
                    "Failed to allocate memory region [{:?}~{:?}]: {:?}",
                    base,
                    base + size,
                    err
                );
            })
            .is_ok()
    }

    fn dealloc_memory_region_at(base: HostPhysAddr, size: usize) {
        axalloc::global_allocator().dealloc_pages(base.as_usize(), size / PAGE_SIZE_4K)
    }

    fn virt_to_phys(vaddr: HostVirtAddr) -> HostPhysAddr {
        axhal::mem::virt_to_phys(vaddr)
    }

    fn current_time_nanos() -> u64 {
        axhal::time::monotonic_time_nanos()
    }

    fn current_vm_id() -> usize {
        axtask::current().task_ext().vm.id()
    }

    fn current_vcpu_id() -> usize {
        axtask::current().task_ext().vcpu.id()
    }

    fn current_pcpu_id() -> usize {
        axhal::percpu::this_cpu_id()
    }

    fn vcpu_resides_on(vm_id: usize, vcpu_id: usize) -> AxResult<usize> {
        vmm::with_vcpu_task(vm_id, vcpu_id, |task| task.cpu_id() as usize)
            .ok_or_else(|| ax_err_type!(NotFound))
    }

    fn inject_irq_to_vcpu(vm_id: usize, vcpu_id: usize, irq: usize) -> axerrno::AxResult {
        vmm::with_vm_and_vcpu_on_pcpu(vm_id, vcpu_id, move |_, vcpu| {
            vcpu.inject_interrupt(irq).unwrap();
        })
    }
}

pub struct AxMmHalImpl;

impl AxMmHal for AxMmHalImpl {
    fn alloc_frame() -> Option<HostPhysAddr> {
        <AxVMHalImpl as AxVMHal>::PagingHandler::alloc_frame()
    }

    fn dealloc_frame(paddr: HostPhysAddr) {
        <AxVMHalImpl as AxVMHal>::PagingHandler::dealloc_frame(paddr)
    }

    #[inline]
    fn phys_to_virt(paddr: HostPhysAddr) -> HostVirtAddr {
        <AxVMHalImpl as AxVMHal>::PagingHandler::phys_to_virt(paddr)
    }

    fn virt_to_phys(vaddr: axaddrspace::HostVirtAddr) -> axaddrspace::HostPhysAddr {
        std::os::arceos::modules::axhal::mem::virt_to_phys(vaddr)
    }
}

pub struct AxVCpuHalImpl;

impl AxVCpuHal for AxVCpuHalImpl {
    type MmHal = AxMmHalImpl;

    #[cfg(target_arch = "aarch64")]
    fn irq_fetch() -> usize {
        0
    }

    #[cfg(target_arch = "aarch64")]
    fn irq_hanlder() {}
}

#[percpu::def_percpu]
static mut AXVM_PER_CPU: AxVMPerCpu<AxVCpuHalImpl> = AxVMPerCpu::<AxVCpuHalImpl>::new_uninit();

/// Init hardware virtualization support in each core.
pub(crate) fn enable_virtualization() {
    use core::sync::atomic::AtomicUsize;
    use core::sync::atomic::Ordering;

    use std::thread;

    use arceos::api::config;
    use arceos::api::task::{AxCpuMask, ax_set_current_affinity};

    static CORES: AtomicUsize = AtomicUsize::new(0);

    info!("Enabling hardware virtualization support on all cores...");

    for cpu_id in 0..config::plat::CPU_NUM {
        thread::spawn(move || {
            // Initialize cpu affinity here.
            assert!(
                ax_set_current_affinity(AxCpuMask::one_shot(cpu_id)).is_ok(),
                "Initialize CPU affinity failed!"
            );

            info!(
                "Enabling hardware virtualization support on core {}",
                cpu_id
            );

            vmm::init_timer_percpu();

            let percpu = unsafe { AXVM_PER_CPU.current_ref_mut_raw() };
            percpu
                .init(this_cpu_id())
                .expect("Failed to initialize percpu state");
            percpu
                .hardware_enable()
                .expect("Failed to enable virtualization");

            info!("Hardware virtualization support enabled on core {}", cpu_id);

            let _ = CORES.fetch_add(1, Ordering::Release);
        });
    }

    info!("Waiting for all cores to enable hardware virtualization...");

    // Wait for all cores to enable virtualization.
    while CORES.load(Ordering::Acquire) != config::plat::CPU_NUM {
        // Use `yield_now` instead of `core::hint::spin_loop` to avoid deadlock.
        thread::yield_now();
    }

    info!("All cores have enabled hardware virtualization support.");
}

#[axvisor_api::api_mod_impl(axvisor_api::memory)]
mod memory_api_impl {
    use super::*;

    extern fn alloc_frame() -> Option<HostPhysAddr> {
        <AxMmHalImpl as AxMmHal>::alloc_frame()
    }

    extern fn alloc_contiguous_frames(
        num_frames: usize,
        frame_align_pow2: usize,
    ) -> Option<HostPhysAddr> {
        arceos::modules::axalloc::global_allocator()
            .alloc_pages(num_frames, PAGE_SIZE_4K << frame_align_pow2)
            .map(|vaddr| <AxMmHalImpl as AxMmHal>::virt_to_phys(vaddr.into()))
            .ok()
    }

    extern fn dealloc_frame(paddr: HostPhysAddr) {
        <AxMmHalImpl as AxMmHal>::dealloc_frame(paddr)
    }

    extern fn dealloc_contiguous_frames(paddr: HostPhysAddr, num_frames: usize) {
        arceos::modules::axalloc::global_allocator().dealloc_pages(paddr.as_usize(), num_frames);
    }

    extern fn phys_to_virt(paddr: HostPhysAddr) -> HostVirtAddr {
        <AxMmHalImpl as AxMmHal>::phys_to_virt(paddr)
    }

    extern fn virt_to_phys(vaddr: HostVirtAddr) -> HostPhysAddr {
        <AxMmHalImpl as AxMmHal>::virt_to_phys(vaddr)
    }
}

#[axvisor_api::api_mod_impl(axvisor_api::time)]
mod time_api_impl {
    use super::*;
    use axvisor_api::time::{CancelToken, Nanos, Ticks, TimeValue};

    extern fn current_ticks() -> Ticks {
        axhal::time::current_ticks()
    }

    extern fn ticks_to_nanos(ticks: Ticks) -> Nanos {
        axhal::time::ticks_to_nanos(ticks)
    }

    extern fn nanos_to_ticks(nanos: Nanos) -> Ticks {
        axhal::time::nanos_to_ticks(nanos)
    }

    extern fn register_timer(
        deadline: TimeValue,
        handler: alloc::boxed::Box<dyn FnOnce(TimeValue) + Send + 'static>,
    ) -> CancelToken {
        vmm::timer::register_timer(deadline.as_nanos() as u64, handler)
    }

    extern fn cancel_timer(token: CancelToken) {
        vmm::timer::cancel_timer(token)
    }
}

#[axvisor_api::api_mod_impl(axvisor_api::vmm)]
mod vmm_api_impl {
    use super::*;
    use axvisor_api::vmm::{InterruptVector, VCpuId, VMId};

    extern fn current_vm_id() -> usize {
        <AxVMHalImpl as AxVMHal>::current_vm_id()
    }

    extern fn current_vcpu_id() -> usize {
        <AxVMHalImpl as AxVMHal>::current_vcpu_id()
    }

    extern fn vcpu_num(vm_id: VMId) -> Option<usize> {
        vmm::with_wm(vm_id, |vm| vm.vcpu_num())
    }

    extern fn active_vcpus(_vm_id: VMId) -> Option<usize> {
        todo!("active_vcpus")
    }

    extern fn inject_interrupt(vm_id: VMId, vcpu_id: VCpuId, vector: InterruptVector) {
        <AxVMHalImpl as AxVMHal>::inject_irq_to_vcpu(vm_id, vcpu_id, vector as usize).unwrap();
    }

    extern fn notify_vcpu_timer_expired(_vm_id: VMId, _vcpu_id: VCpuId) {
        todo!("notify_vcpu_timer_expired")
        // vmm::timer::notify_timer_expired(vm_id, vcpu_id);
    }
}

#[axvisor_api::api_mod_impl(axvisor_api::arch)]
mod arch_api_impl {
    #[cfg(target_arch = "aarch64")]
    extern fn hardware_inject_virtual_interrupt(irq: axvisor_api::vmm::InterruptVector) {
        debug!("Injecting virtual interrupt: {}", irq);
        unimplemented!();
        // use axstd::os::arceos::modules::axhal;
        // axhal::irq::inject_interrupt(irq as usize);
        // super::inject_interrupt(irq as usize);
    }

    #[cfg(target_arch = "aarch64")]
    extern fn read_vgicd_typer() -> u32 {
        // use axstd::os::arceos::modules::axhal::irq::MyVgic;
        // MyVgic::get_gicd().lock().get_typer()

        // use memory_addr::pa;
        // use std::os::arceos::modules::{axconfig, axhal};

        unimplemented!();
        // let typer_phys_addr = axconfig::devices::GICD_PADDR + 0x4;
        // let typer_virt_addr = axhal::mem::phys_to_virt(pa!(typer_phys_addr));

        // unsafe { core::ptr::read_volatile(typer_virt_addr.as_ptr_of::<u32>()) }
    }

    #[cfg(target_arch = "aarch64")]
    extern fn read_vgicd_iidr() -> u32 {
        // use axstd::os::arceos::modules::axhal::irq::MyVgic;
        // MyVgic::get_gicd().lock().get_iidr()
        0
    }

    #[cfg(target_arch = "aarch64")]
    extern fn get_host_gicd_base() -> memory_addr::PhysAddr {
        // use std::os::arceos::api::config;
        // unimplemented!();
        // config::devices::GICD_PADDR.into()
        0x800_0000.into()
    }

    #[cfg(target_arch = "aarch64")]
    extern fn get_host_gicr_base() -> memory_addr::PhysAddr {
        // use std::os::arceos::api::config;
        // unimplemented!();
        // config::devices::GICR_PADDR.into()
        // TODO parse from dtb
        0x80a_0000.into()
    }
}

#[axvisor_api::api_mod_impl(axvisor_api::host)]
mod host_api_impl {
    extern fn get_host_cpu_num() -> usize {
        std::os::arceos::modules::axconfig::plat::CPU_NUM
    }
}

/// Reads and returns the value of the given aarch64 system register.
macro_rules! read_sysreg {
    ($name:ident) => {
        {
            let mut value: u64;
            unsafe{::core::arch::asm!(
                concat!("mrs {value:x}, ", ::core::stringify!($name)),
                value = out(reg) value,
                options(nomem, nostack),
            );}
            value
        }
    }
}

/// Writes the given value to the given aarch64 system register.
macro_rules! write_sysreg {
    ($name:ident, $value:expr) => {
        {
            let v: u64 = $value;
            unsafe{::core::arch::asm!(
                concat!("msr ", ::core::stringify!($name), ", {value:x}"),
                value = in(reg) v,
                options(nomem, nostack),
            )}
        }
    }
}

pub fn inject_interrupt(vector: usize) {
    // mask
    const LR_VIRTIRQ_MASK: usize = (1 << 32) - 1;
    const LR_STATE_MASK: u64 = 0x3 << 62; // bits [63:62]
    const LR_STATE_PENDING: u64 = 0x1 << 62; // pending state
    const LR_STATE_ACTIVE: u64 = 0x2 << 62; // active state

    debug!("Injecting virtual interrupt: vector={}", vector);

    let elsr: u64 = read_sysreg!(ich_elrsr_el2);
    let vtr = read_sysreg!(ich_vtr_el2) as usize;
    let lr_num: usize = (vtr & 0xf) + 1;
    let mut free_lr = -1 as isize;

    // First, check if this interrupt is already pending/active
    for i in 0..lr_num {
        // find a free list register
        if (1 << i) & elsr > 0 {
            if free_lr == -1 {
                free_lr = i as isize;
            }
            continue;
        }
        let lr_val = read_lr(i);
        // if a virtual interrupt is enabled and equals to the physical interrupt irq_id
        if (lr_val as usize & LR_VIRTIRQ_MASK) == vector {
            let state = lr_val & LR_STATE_MASK;
            if state == LR_STATE_PENDING || state == LR_STATE_ACTIVE {
                debug!(
                    "virtual irq {} already pending/active in LR{}, skipping",
                    vector, i
                );
                return;
            }
        }
    }

    debug!("use free lr {} to inject irq {}", free_lr, vector);

    if free_lr == -1 {
        warn!(
            "No free list register to inject IRQ {}, checking ICH_HCR_EL2",
            vector
        );
        // Check if virtual interrupt interface is enabled
        let ich_hcr = read_sysreg!(ich_hcr_el2);
        debug!("ICH_HCR_EL2: 0x{:x}", ich_hcr);

        // Try to find and reuse an inactive LR
        for i in 0..lr_num {
            let lr_val = read_lr(i);
            let state = lr_val & LR_STATE_MASK;
            if state == 0 {
                // inactive state
                debug!("Reusing inactive LR{} for IRQ {}", i, vector);
                free_lr = i as isize;
                break;
            }
        }

        if free_lr == -1 {
            panic!("No free list register to inject IRQ {}", vector);
        }
    }

    let mut val = vector as u64; // vector
    val |= 1 << 60; // group 1
    val |= 1 << 62; // state pending
    // hardware interrupt not supported
    write_lr(free_lr as usize, val);

    // Ensure the virtual interrupt interface is enabled
    // let ich_hcr = read_sysreg!(ich_hcr_el2);
    // if (ich_hcr & 1) == 0 {
    //     // Check EN bit
    //     warn!("Virtual interrupt interface not enabled, enabling now");
    //     write_sysreg!(ich_hcr_el2, ich_hcr | 1);
    // }

    debug!(
        "Virtual interrupt {} injected successfully in LR{}",
        vector, free_lr
    );
}

fn read_lr(id: usize) -> u64 {
    let id = id as u64;
    match id {
        //TODO get lr size from gic reg
        0 => read_sysreg!(ich_lr0_el2),
        1 => read_sysreg!(ich_lr1_el2),
        2 => read_sysreg!(ich_lr2_el2),
        3 => read_sysreg!(ich_lr3_el2),
        4 => read_sysreg!(ich_lr4_el2),
        5 => read_sysreg!(ich_lr5_el2),
        6 => read_sysreg!(ich_lr6_el2),
        7 => read_sysreg!(ich_lr7_el2),
        8 => read_sysreg!(ich_lr8_el2),
        9 => read_sysreg!(ich_lr9_el2),
        10 => read_sysreg!(ich_lr10_el2),
        11 => read_sysreg!(ich_lr11_el2),
        12 => read_sysreg!(ich_lr12_el2),
        13 => read_sysreg!(ich_lr13_el2),
        14 => read_sysreg!(ich_lr14_el2),
        15 => read_sysreg!(ich_lr15_el2),
        _ => {
            panic!("invalid lr id {}", id);
        }
    }
}

fn write_lr(id: usize, val: u64) {
    let id = id as u64;
    match id {
        0 => write_sysreg!(ich_lr0_el2, val),
        1 => write_sysreg!(ich_lr1_el2, val),
        2 => write_sysreg!(ich_lr2_el2, val),
        3 => write_sysreg!(ich_lr3_el2, val),
        4 => write_sysreg!(ich_lr4_el2, val),
        5 => write_sysreg!(ich_lr5_el2, val),
        6 => write_sysreg!(ich_lr6_el2, val),
        7 => write_sysreg!(ich_lr7_el2, val),
        8 => write_sysreg!(ich_lr8_el2, val),
        9 => write_sysreg!(ich_lr9_el2, val),
        10 => write_sysreg!(ich_lr10_el2, val),
        11 => write_sysreg!(ich_lr11_el2, val),
        12 => write_sysreg!(ich_lr12_el2, val),
        13 => write_sysreg!(ich_lr13_el2, val),
        14 => write_sysreg!(ich_lr14_el2, val),
        15 => write_sysreg!(ich_lr15_el2, val),
        _ => {
            panic!("invalid lr id {}", id);
        }
    }
}
