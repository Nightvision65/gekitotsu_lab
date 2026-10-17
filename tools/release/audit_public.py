"""检查指定公开Git历史或源码ZIP的发布边界，不读取其他本地分支。"""
import argparse
import io
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import zipfile

ROOT = Path(__file__).resolve().parents[2]
PRIVATE_ROOTS = ('local-maintenance/', 'assets/promotional/', 'rust/vendor/upstream/', 'experiments/', 'research/', 'formations/', 'ai-data/', '.codex/', '.claude/', '.idea/')
PRIVATE_FILES = {'build_native.ps1', 'tests/verify_reference.py', 'tests/GuiRegression.java', 'app/REFERENCE_PROVENANCE.json', 'rust/vendor/upstream-source-hashes.json', '1P.txt', '2P.txt', 'config.ini', 'result.txt', 'simple_result.txt', 'battle_cache.bin', 'strength_cache.bin'}
TEXT_SUFFIXES = {'.py', '.ps1', '.bat', '.java', '.rs', '.md', '.json', '.xml', '.toml', '.yml', '.yaml', '.txt', '.properties'}
MACHINE_PATH = re.compile(r'(?<![A-Za-z0-9])[A-Za-z]:[\\/]+(?:Users|GameDevelopment|ProgramData|Agents|program)[\\/]+[^\s\x00"`<>]+', re.I)
SECRET = re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}')


def inspect(name, content):
    errors = []
    if name.startswith(PRIVATE_ROOTS) or name in PRIVATE_FILES or name.startswith('runtime/') or name.endswith('.exe'):
        errors.append(f'源码含本地数据或构建产物：{name}')
    if len(content) > 100 * 1024 * 1024:
        errors.append(f'普通Git文件超过100MiB：{name}')
    if SECRET.search(content):
        errors.append(f'疑似凭据：{name}')
    if Path(name).suffix.lower() in TEXT_SUFFIXES or name.endswith(('.dll', '.exe', '.class')):
        text = content.decode('utf-8-sig', errors='replace')
        if MACHINE_PATH.search(text):
            errors.append(f'含个人机器路径：{name}')
        if re.search(r'"sourceRawPath"\s*:\s*"(?:ai-data|experiments)/', text):
            errors.append(f'含本地运行证据引用：{name}')
        if name.startswith('docs/ai/knowledge/') and Path(name).suffix == '.md':
            if re.search(r'(?m)^### #\d|W/L/D/U/E\s*=|(?<![A-Za-z0-9-])#\d{1,3}(?![\d-])', text):
                errors.append(f'知识文档含具体实验案例：{name}')
    if name.endswith(('.jar', '.xlsx')):
        with zipfile.ZipFile(io.BytesIO(content)) as archive:
            for entry in archive.infolist():
                if not entry.is_dir():
                    errors.extend(e for e in inspect(f'{name}!/{entry.filename}', archive.read(entry))
                                  if '源码含本地数据或构建产物' not in e)
    return errors


def git(*args):
    return subprocess.check_output(['git', *args], cwd=ROOT, timeout=60)


def audit_revision(revision):
    errors, checked = [], 0
    commits = git('rev-list', revision).decode().splitlines()
    seen = set()
    for commit in commits:
        for entry in git('ls-tree', '-rz', commit).split(b'\0'):
            if not entry:
                continue
            meta, path = entry.split(b'\t', 1)
            mode, kind, object_id = meta.decode().split()
            name = path.decode('utf-8')
            identity = (name, object_id)
            if identity in seen:
                continue
            seen.add(identity)
            if kind != 'blob':
                errors.append(f'不支持的子模块：{name}')
                continue
            errors.extend(inspect(name, git('cat-file', 'blob', object_id)))
            checked += 1
    return {'revision': revision, 'commits': len(commits), 'checkedBlobs': checked, 'errors': errors}


def audit_archive(path):
    errors = []
    with zipfile.ZipFile(path) as archive:
        files = [e for e in archive.infolist() if not e.is_dir()]
        prefixes = {e.filename.split('/')[0] for e in files}
        prefix = next(iter(prefixes)) + '/' if len(prefixes) == 1 else ''
        for entry in files:
            name = entry.filename.removeprefix(prefix)
            if name.startswith('/') or '..' in Path(name).parts:
                errors.append(f'不安全ZIP路径：{name}')
            errors.extend(inspect(name, archive.read(entry)))
    return {'archive': str(path), 'checkedFiles': len(files), 'errors': errors}


if __name__ == '__main__':
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        sys.exit('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--revision', default='HEAD')
    parser.add_argument('--archive', type=Path)
    args = parser.parse_args()
    report = audit_archive(args.archive) if args.archive else audit_revision(args.revision)
    print(json.dumps(report, ensure_ascii=False, indent=2))
    sys.exit(bool(report['errors']))
