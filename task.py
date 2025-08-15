#!/usr/bin/env python3
# -*- coding: utf-8 -*-
import argparse
import sys
import importlib
from scripts.config import add_common_arguments


def main():
    parser = argparse.ArgumentParser(description="Axvisor 命令行工具")
    subparsers = parser.add_subparsers(dest="command", help="可用命令")

    # setup 命令
    setup_parser = subparsers.add_parser("setup", help="设置 arceos 依赖")

    # build 命令
    build_parser = subparsers.add_parser("build", help="构建项目")
    add_common_arguments(build_parser)

    # run 命令
    run_parser = subparsers.add_parser("run", help="运行项目")
    add_common_arguments(run_parser)

    args = parser.parse_args()

    if args.command == "setup":
        mod = importlib.import_module("scripts.setup")
        exit_code = mod.main(args)
        sys.exit(exit_code)
    elif args.command == "build":
        mod = importlib.import_module("scripts.build")
        exit_code = mod.main(args)
        sys.exit(exit_code)
    elif args.command == "run":
        mod = importlib.import_module("scripts.run")
        exit_code = mod.main(args)
        sys.exit(exit_code)
    else:
        parser.print_help()


if __name__ == "__main__":
    main()
