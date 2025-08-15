#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import os
import subprocess
import sys
from .config import merge_config, get_make_variables, format_make_command
from .setup import setup_arceos
from . import build


def main(args):
    """运行项目"""
    print("执行 run 功能...")

    # 合并配置文件和命令行参数
    args = merge_config(args)

    # 首先执行 build
    print("运行前先构建项目...")
    build_result = build.main(args)
    if build_result != 0:
        print("构建失败，无法运行")
        return build_result

    # 获取 make 变量和环境变量
    make_vars, env_vars = get_make_variables(args)

    # 构建 make 命令
    cmd = format_make_command(make_vars, env_vars, "run")

    print(f"执行命令: {cmd}")

    try:
        # 设置环境变量
        env = os.environ.copy()
        env.update(env_vars)

        # 执行 make run 命令
        result = subprocess.run(cmd, shell=True, check=True, env=env)
        print("运行完成!")
        return 0
    except subprocess.CalledProcessError as e:
        print(f"运行失败，退出码: {e.returncode}")
        return e.returncode
    except Exception as e:
        print(f"运行过程中发生错误: {e}")
        return 1
