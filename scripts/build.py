#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import os
import subprocess
import sys
from typing import Optional
from .config import AxvisorConfig, create_config_from_args
from .setup import setup_arceos


def main(args) -> int:
    """构建项目"""
    print("执行 build 功能...")

    # 首先设置 arceos 依赖
    print("设置 arceos 依赖...")
    if not setup_arceos():
        print("设置 arceos 失败，无法继续构建")
        return 1

    # 创建配置对象
    config: AxvisorConfig = create_config_from_args(args)

    # 构建 make 命令
    cmd = config.format_make_command("")

    print(f"执行命令: {cmd}")

    try:
        # 执行 make 命令
        result = subprocess.run(
            cmd, shell=True, check=True, env=config.get_subprocess_env()
        )
        print("构建成功!")
        return 0
    except subprocess.CalledProcessError as e:
        print(f"构建失败，退出码: {e.returncode}")
        return e.returncode
    except Exception as e:
        print(f"构建过程中发生错误: {e}")
        return 1
