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

# 清理目标
.PHONY: clean
clean:
	@echo "清理构建文件..."
	@rm -rf target/

# 深度清理（包括删除 .arceos 文件夹）
.PHONY: clean-all
clean-all: clean
	@echo "删除 .arceos 文件夹..."
	@rm -rf .arceos

# 帮助信息
.PHONY: help
help:
	@echo "可用的 make 目标："
	@echo "  default (或直接运行 make)  - 设置 arceos 依赖并构建"
	@echo "  setup-arceos              - 检查并克隆 arceos 仓库"
	@echo "  clean                     - 清理构建文件"
	@echo "  clean-all                 - 清理所有文件（包括 .arceos）"
	@echo "  help                      - 显示此帮助信息"
	@echo ""
	@echo "所有其他目标和参数都会透传给 .arceos/Makefile"
	@echo "例如："
	@echo "  make run                  - 构建并运行"
	@echo "  make ARCH=aarch64         - 指定架构构建"
	@echo "  make debug MODE=debug     - 调试模式构建"

# 透传所有其他目标到 .arceos
%: setup-arceos
	@echo "透传目标 '$@' 到 arceos..."
	@$(MAKE) -C .arceos A=$(shell pwd) LD_SCRIPT=link.x $@ $(MAKEFLAGS)