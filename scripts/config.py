#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import os
import argparse


def load_config_file(config_path=".hvconfig.toml"):
    """从配置文件加载配置"""
    if os.path.exists(config_path):
        try:
            import toml

            with open(config_path, "r", encoding="utf-8") as f:
                return toml.load(f)
        except ImportError:
            print("警告：需要安装 toml 库来读取 .hvconfig.toml 文件")
            return {}
        except Exception as e:
            print(f"警告：读取配置文件 {config_path} 失败: {e}")
            return {}
    return {}


def add_common_arguments(parser):
    """为解析器添加通用参数"""
    parser.add_argument(
        "--features", type=str, help="Hypervisor features (comma-separated)"
    )
    parser.add_argument(
        "--arceos-features", type=str, help="ArceOS features (comma-separated)"
    )
    parser.add_argument(
        "--arceos-args", type=str, help="ArceOS arguments (comma-separated)"
    )
    parser.add_argument(
        "--plat",
        type=str,
        default="aarch64-generic",
        help="Platform (default: aarch64-generic)",
    )
    parser.add_argument(
        "--vmconfigs", type=str, help="VM configuration file path (comma-separated)"
    )


def array_to_comma_separated(value):
    """将数组转换为逗号分隔的字符串"""
    if isinstance(value, list):
        # 过滤掉空字符串
        filtered_values = [str(v) for v in value if v]
        return ",".join(filtered_values) if filtered_values else ""
    return str(value) if value else ""


def merge_config(args, config_file=None):
    """合并命令行参数和配置文件，命令行参数优先"""
    if config_file is None:
        config_file = load_config_file()

    # 如果命令行参数未指定，则使用配置文件中的值
    if args.features is None and "features" in config_file:
        args.features = array_to_comma_separated(config_file["features"])

    if (
        getattr(args, "arceos_features", None) is None
        and "arceos_features" in config_file
    ):
        args.arceos_features = array_to_comma_separated(config_file["arceos_features"])

    if getattr(args, "arceos_args", None) is None and "arceos_args" in config_file:
        args.arceos_args = array_to_comma_separated(config_file["arceos_args"])

    if args.plat == "aarch64-generic" and "plat" in config_file:
        args.plat = config_file["plat"]

    if args.vmconfigs is None and "vmconfigs" in config_file:
        args.vmconfigs = array_to_comma_separated(config_file["vmconfigs"])

    return args


def get_make_variables(args):
    """根据参数生成 make 变量和环境变量"""
    make_vars = {}
    env_vars = {}

    # 基本的 make 变量
    make_vars["A"] = os.getcwd()
    make_vars["LD_SCRIPT"] = "link.x"

    if args.plat:
        make_vars["MYPLAT"] = f"axplat-{args.plat}"

    make_vars["APP_FEATURES"] = f"plat-{args.plat}"
    if args.features:
        make_vars["APP_FEATURES"] += f",{args.features}"

    if hasattr(args, "arceos_features") and args.arceos_features:
        make_vars["FEATURES"] = args.arceos_features

    if hasattr(args, "arceos_args") and args.arceos_args:
        for arg in args.arceos_args.split(","):
            key, value = arg.split("=", 1) if "=" in arg else (arg, "y")
            make_vars[key.strip()] = value.strip()

    # 处理 vmconfigs 作为环境变量
    if args.vmconfigs:
        env_vars["AXVISOR_VM_CONFIGS"] = args.vmconfigs

    return make_vars, env_vars


def format_make_command(make_vars, env_vars=None, target=""):
    """格式化 make 命令，包含环境变量"""
    cmd_parts = []

    # 添加 make 命令
    cmd_parts.extend(["make", "-C", ".arceos"])

    # 添加 make 变量
    for key, value in make_vars.items():
        cmd_parts.append(f"{key}={value}")

    cmd_parts.append("QEMU_ARGS=\"-machine virtualization=on\"")

    if target:
        cmd_parts.append(target)

    return " ".join(cmd_parts)
