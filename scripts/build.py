#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import os
import subprocess
import sys
from .config import merge_config, get_make_variables, format_make_command
from .setup import setup_arceos


def main(args):
    """构建项目"""
    print("执行 build 功能...")

    # 首先设置 arceos 依赖
    print("设置 arceos 依赖...")
    if not setup_arceos():
        print("设置 arceos 失败，无法继续构建")
        return 1

    # 合并配置文件和命令行参数
    args = merge_config(args)

    # 获取 make 变量和环境变量
    make_vars, env_vars = get_make_variables(args)

    # 构建 make 命令
    cmd = format_make_command(make_vars, env_vars, "")

    print(f"执行命令: {cmd}")

    try:
        # 设置环境变量
        env = os.environ.copy()
        env.update(env_vars)

        # 执行 make 命令
        result = subprocess.run(cmd, shell=True, check=True, env=env)
        print("构建成功!")
        return 0
    except subprocess.CalledProcessError as e:
        print(f"构建失败，退出码: {e.returncode}")
        return e.returncode
    except Exception as e:
        print(f"构建过程中发生错误: {e}")
        return 1
