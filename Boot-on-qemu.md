## Compile AxVisor

* get deps
```bash
./tool/dev_env.py
cd crates/arceos && git checkout rk3588_jd4_qemu
cd crates/axvm && git checkout dtb
cd crates/arm_vcpu && git checkout 4_level_paging
cd crates/axaddrspace && git checkout 4_level_paging
```

* build dtb

```bash
dtc -o configs/vms/qemu_gicv3.dtb -O dtb -I dts configs/vms/qemu_gicv3.dts
```

```bash
make ARCH=aarch64 PLATFORM=configs/platforms/aarch64-qemu-virt-hv.toml defconfig
make ARCH=aarch64 PLATFORM=configs/platforms/aarch64-qemu-virt-hv.toml LOG=debug VM_CONFIGS=configs/vms/linux-qemu-aarch64.toml:configs/vms/arceos-aarch64.toml GICV3=y NET=y SMP=2 run DISK_IMG=configs/vms/ubuntu-22.04-rootfs_ext4.img SECOND_SERIAL=y

telnet localhost 4321
```