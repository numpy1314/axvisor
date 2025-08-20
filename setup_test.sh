
#!/bin/bash

# 确保 tmp 目录存在
mkdir -p tmp

# 检查并下载 aarch64_usertests.zip
if [ ! -f "tmp/aarch64_usertests.zip" ]; then
    echo "Downloading aarch64_usertests.zip..."
    wget -P tmp https://github.com/arceos-hypervisor/nimbos/releases/download/v0.7/aarch64_usertests.zip
else
    echo "aarch64_usertests.zip already exists, skipping download."
fi

# 检查并下载 x86_64_usertests.zip
if [ ! -f "tmp/x86_64_usertests.zip" ]; then
    echo "Downloading x86_64_usertests.zip..."
    wget -P tmp https://github.com/arceos-hypervisor/nimbos/releases/download/v0.7/x86_64_usertests.zip
else
    echo "x86_64_usertests.zip already exists, skipping download."
fi

sudo rm -rf tmp/tmp_disk
mkdir -p tmp/tmp_disk
unzip tmp/aarch64_usertests.zip -d tmp/tmp_disk
mv tmp/tmp_disk/nimbos.bin tmp/tmp_disk/nimbos-aarch64.bin

sudo rm  .arceos/nimbos-aarch64.img
./task.py disk_img --image nimbos-aarch64.img
sudo mkdir -p tmp/img
sudo chown root:root tmp/tmp_disk/*
sudo mount .arceos/nimbos-aarch64.img tmp/img
sudo mv tmp/tmp_disk/* tmp/img
sudo umount tmp/img