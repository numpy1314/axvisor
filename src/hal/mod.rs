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

#[cfg_attr(target_arch = "aarch64", path = "arch/aarch64/mod.rs")]
#[cfg_attr(target_arch = "x86_64", path = "arch/x86_64/mod.rs")]
#[cfg_attr(target_arch = "loongarch64", path = "arch/loongarch64/mod.rs")]
pub mod arch;

use crate::vmm;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub enum CacheOp {
    /// Write back to memory
    Clean,
    /// Invalidate cache
    Invalidate,
    /// Clean and invalidate
    CleanAndInvalidate,
}

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
            info!(
                "Core {} is initializing hardware virtualization support...",
                cpu_id
            );
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

#[axvisor_api::api_mod_impl(axvisor_api::host)]
mod host_api_impl {
    extern fn get_host_cpu_num() -> usize {
        std::os::arceos::modules::axconfig::plat::CPU_NUM
    }
}
