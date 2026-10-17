"""给Rust构建重映射机器路径，避免将用户目录写进发布二进制。"""
import os
from pathlib import Path
import shlex


def rust_environment():
    env = dict(os.environ)
    root = Path(__file__).resolve().parents[2]
    cargo_home = Path(env.get('CARGO_HOME', Path.home() / '.cargo')).resolve()
    flags = env.get('CARGO_ENCODED_RUSTFLAGS', '').split('\x1f') if env.get('CARGO_ENCODED_RUSTFLAGS') else shlex.split(env.get('RUSTFLAGS', ''))
    if os.name == 'nt':
        flags += ['-C', 'target-feature=+crt-static']
    for source, replacement in ((root, '/gekitotsu_lab'), (cargo_home, '/cargo')):
        # rustc diagnostic strings may use native or slash-separated paths.
        for spelling in dict.fromkeys((str(source), source.as_posix())):
            flags.append(f'--remap-path-prefix={spelling}={replacement}')
    env['CARGO_ENCODED_RUSTFLAGS'] = '\x1f'.join(flags)
    return env
