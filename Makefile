# The pathes of the VM configurations
ifneq ($(VM_CONFIGS),)
  export AXVISOR_VM_CONFIGS=$(VM_CONFIGS)
endif

# 默认目标
.PHONY: default
default: setup-arceos
	@echo "执行 arceos 构建..."
	@$(MAKE) -C .arceos A=$(shell pwd) LD_SCRIPT=link.x  $(MAKEFLAGS)

# 设置 arceos 依赖
.PHONY: setup-arceos
setup-arceos:
	@if [ ! -d ".arceos" ]; then \
		echo "正在克隆 arceos 仓库..."; \
		git clone https://github.com/arceos-hypervisor/arceos -b vmm-dev .arceos; \
		echo "arceos 仓库克隆完成"; \
	else \
		echo ".arceos 文件夹已存在"; \
	fi

# 透传所有其他目标到 .arceos
run: setup-arceos
	@$(MAKE) -C .arceos A=$(shell pwd) LD_SCRIPT=link.x $@ $(MAKEFLAGS) run

clean: setup-arceos
	@$(MAKE) -C .arceos A=$(shell pwd) LD_SCRIPT=link.x $@ $(MAKEFLAGS) clean

disk_img: setup-arceos
	@$(MAKE) -C .arceos A=$(shell pwd) LD_SCRIPT=link.x $@ $(MAKEFLAGS) disk_img