use crate::hal::CacheOp;
use memory_addr::VirtAddr;
/// LoongArch64 data cache range operation
pub fn dcache_range(_op: CacheOp, _addr: VirtAddr, _size: usize) {
    // TODO: Implement LoongArch64 dcache range flush
}

/// LoongArch64 instruction cache range operation  
pub fn icache_range(_op: CacheOp, _addr: VirtAddr, _size: usize) {
    // TODO: Implement LoongArch64 icache range flush
}
