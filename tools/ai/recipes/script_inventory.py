#!/usr/bin/env python3
"""只读盘点实验脚本及重复函数；不导入或运行被检查的脚本。"""
from __future__ import annotations
import argparse
import ast
from collections import defaultdict
from datetime import datetime, timezone
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import ai


def inventory(roots, since=None):
    files, errors = [], []
    file_groups, function_groups = defaultdict(list), defaultdict(list)
    seen = set()
    for root in roots:
        root = Path(root).resolve(strict=True)
        if not root.is_dir():
            raise ValueError(f'不是目录：{root}')
        for directory, dirs, names in os.walk(root, followlinks=False):
            dirs[:] = sorted(d for d in dirs if d not in {'.git', '__pycache__', 'target', 'node_modules'}
                             and not (Path(directory)/d).is_symlink())
            for name in sorted(names):
                path = Path(directory)/name
                if path.suffix.lower() not in {'.py', '.ps1'} or path.is_symlink() or path in seen:
                    continue
                seen.add(path)
                stat = path.stat()
                if since is not None and stat.st_mtime < since:
                    continue
                sha = ai.file_hash(path)
                files.append({'path': str(path), 'size': stat.st_size, 'sha256': sha,
                              'modifiedAtUtc': datetime.fromtimestamp(stat.st_mtime, timezone.utc).isoformat()})
                file_groups[sha].append(str(path))
                if path.suffix.lower() != '.py':
                    continue
                try:
                    tree = ast.parse(path.read_text(encoding='utf-8-sig'), filename=str(path))
                except (SyntaxError, UnicodeError) as error:
                    errors.append({'path': str(path), 'error': str(error)})
                    continue
                for node in ast.walk(tree):
                    if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.end_lineno-node.lineno >= 3:
                        digest = ai.digest(ast.dump(node, include_attributes=False).encode())
                        function_groups[digest].append({'path': str(path), 'name': node.name,
                                                        'line': node.lineno, 'endLine': node.end_lineno})
    return {'schema': 'gkt-script-inventory-v1', 'files': files, 'fileCount': len(files),
            'duplicateFiles': [{'sha256': k, 'paths': v} for k, v in sorted(file_groups.items()) if len(v)>1],
            'duplicateFunctions': [{'astSha256': k, 'occurrences': v} for k, v in sorted(function_groups.items()) if len(v)>1],
            'parseErrors': errors, 'scope': '文件字节及Python函数AST完全重复；不证明语义近似，也不运行实验。'}


def main(argv=None):
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        raise ValueError('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--root', action='append', required=True)
    parser.add_argument('--since', help='带时区的ISO时间；省略则盘点全部脚本')
    parser.add_argument('--out', required=True)
    args = parser.parse_args(argv)
    since = None
    if args.since:
        date = datetime.fromisoformat(args.since)
        if date.tzinfo is None:
            raise ValueError('--since须包含时区，例如+08:00')
        since = date.timestamp()
    target = Path(args.out)
    if target.exists():
        raise FileExistsError(f'禁止覆盖：{target}')
    result = inventory(args.root, since)
    target.parent.mkdir(parents=True, exist_ok=True)
    with target.open('x', encoding='utf-8') as stream:
        stream.write(ai.dumps(result)+'\n')
    print(f"脚本盘点完成：{result['fileCount']}份，完全重复文件{len(result['duplicateFiles'])}组，重复函数{len(result['duplicateFunctions'])}组；未运行任何实验。")


if __name__ == '__main__':
    main()
