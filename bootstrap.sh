#!/bin/bash
# -*- coding: utf-8 -*-

# Axvisor Bootstrap Script
# 此脚本用于安装 task.py 所需的 Python 依赖

set -e  # 遇到错误时退出

echo "=== Axvisor Bootstrap Script ==="
echo "正在安装 task.py 所需的 Python 依赖..."

# 检查 Python 版本
python_version=$(python3 --version 2>&1 | awk '{print $2}' | cut -d. -f1,2)
echo "检测到 Python 版本: $python_version"

# 检查是否有 pip
if ! command -v pip3 &> /dev/null; then
    echo "错误: pip3 未找到，请先安装 pip3"
    exit 1
fi

echo "检测到 pip3 版本: $(pip3 --version)"

# 检查 requirements.txt 文件是否存在
if [[ ! -f "requirements.txt" ]]; then
    echo "错误: requirements.txt 文件未找到"
    exit 1
fi

echo "正在安装 Python 依赖..."

# 检查是否在虚拟环境中
if [[ "$VIRTUAL_ENV" != "" ]]; then
    echo "检测到虚拟环境: $VIRTUAL_ENV"
    pip install -r requirements.txt
else
    echo "未检测到虚拟环境，使用 --user 安装"
    pip3 install --user -r requirements.txt
fi

echo "依赖安装完成!"

# 测试 task.py 是否可以正常运行
echo "测试 task.py..."
if ./task.py --help > /dev/null 2>&1; then
    echo "✓ task.py 运行正常"
else
    echo "✗ task.py 运行失败，请检查安装"
    exit 1
fi

echo "=== Bootstrap 完成 ==="
echo "您现在可以使用以下命令:"
echo "  ./task.py build          # 构建项目"
echo "  ./task.py run            # 运行项目"
echo "  ./task.py build --help   # 查看构建选项"
echo "  ./task.py run --help     # 查看运行选项"
