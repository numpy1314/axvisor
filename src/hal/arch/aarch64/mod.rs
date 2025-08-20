mod api;
pub mod cache;

pub fn inject_interrupt(irq: usize) {
    debug!("Injecting virtual interrupt: {}", irq);

    let mut gic = rdrive::get_one::<rdrive::driver::Intc>()
        .expect("Failed to get GIC driver")
        .lock()
        .unwrap();
    if let Some(gic) = gic.typed_mut::<arm_gic_driver::v2::Gic>() {
        use arm_gic_driver::{
            IntId,
            v2::{VirtualInterruptConfig, VirtualInterruptState},
        };

        let gich = gic.hypervisor_interface().expect("Failed to get GICH");
        gich.enable();
        gich.set_virtual_interrupt(
            0,
            VirtualInterruptConfig::software(
                unsafe { IntId::raw(irq as _) },
                None,
                0,
                VirtualInterruptState::Pending,
                false,
                true,
            ),
        );
        return;
    }

    if let Some(_gic) = gic.typed_mut::<arm_gic_driver::v3::Gic>() {
        inject_interrupt_gic_v3(irq as _);
        return;
    }

    panic!("no gic driver found")
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

#[cfg(target_arch = "aarch64")]
pub fn inject_interrupt_gic_v3(vector: usize) {
    use arm_gic_driver::v3::*;

    // mask
    const LR_VIRTIRQ_MASK: usize = (1 << 32) - 1;
    const LR_STATE_MASK: u64 = 0x3 << 62; // bits [63:62]
    const LR_STATE_PENDING: u64 = 0x1 << 62; // pending state
    const LR_STATE_ACTIVE: u64 = 0x2 << 62; // active state

    debug!("Injecting virtual interrupt: vector={}", vector);
    let elsr = ICH_ELRSR_EL2.read(ICH_ELRSR_EL2::STATUS);
    let lr_num = ICH_VTR_EL2.read(ICH_VTR_EL2::LISTREGS) as usize + 1;

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

    ICH_LR0_EL2.write(
        ICH_LR_EL2::VINTID.val(vector as u64) + ICH_LR_EL2::STATE::Pending + ICH_LR_EL2::GROUP::SET,
    );

    // Ensure the virtual interrupt interface is enabled
    let en = ICH_HCR_EL2.is_set(ICH_HCR_EL2::EN);
    if !en {
        // Check EN bit
        warn!("Virtual interrupt interface not enabled, enabling now");
        ICH_HCR_EL2.modify(ICH_HCR_EL2::EN::SET);
    }

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
