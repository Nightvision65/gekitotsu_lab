"""从干净公开提交生成源码包与Windows便携包；不推送、不上传。"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import xml.etree.ElementTree as ET
import zipfile

from audit_public import ROOT, MACHINE_PATH, audit_revision, inspect


def run(command, cwd, timeout, capture=False):
    proc = subprocess.Popen([str(x) for x in command], cwd=cwd,
                            env={**os.environ, 'PYTHONDONTWRITEBYTECODE': '1'},
                            stdout=subprocess.PIPE if capture else None,
                            stderr=subprocess.PIPE if capture else None)
    try:
        stdout, stderr = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        if os.name == 'nt':
            subprocess.run(['taskkill', '/PID', str(proc.pid), '/T', '/F'], capture_output=True, timeout=10)
        else:
            proc.kill()
        proc.communicate(timeout=10)
        raise
    if proc.returncode:
        raise RuntimeError(f'命令失败：{command[0]}\n{(stderr or b"").decode("utf-8", errors="replace")}')
    return stdout


def main():
    if os.environ.get('CONDA_DEFAULT_ENV') != 'opencode':
        sys.exit('请先conda activate opencode')
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--maven', default='mvn')
    parser.add_argument('--cargo', default='cargo')
    parser.add_argument('--out', type=Path, default=Path('dist'))
    parser.add_argument('--timeout', type=int, default=300)
    parser.add_argument('--offline', action='store_true')
    parser.add_argument('--skip-build', action='store_true')
    args = parser.parse_args()
    git = lambda *words: run(['git', *words], ROOT, 60, True)
    if git('status', '--porcelain').strip():
        raise RuntimeError('发布只接受干净提交；请先检查并提交公开变更。')
    revision = git('rev-parse', 'HEAD').decode().strip()
    audit = audit_revision(revision)
    if audit['errors']:
        raise RuntimeError(json.dumps(audit, ensure_ascii=False))
    ns = {'m': 'http://maven.apache.org/POM/4.0.0'}
    pom = ET.parse(ROOT / 'pom.xml').getroot()
    version = pom.findtext('m:version', namespaces=ns)
    artifact = pom.findtext('m:artifactId', namespaces=ns)
    assert artifact == 'gekitotsu_lab'
    jar_name = f'{artifact}-{version}.jar'
    out = (ROOT / args.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    source_zip = out / f'{artifact}-{version}-source.zip'
    portable_zip = out / f'{artifact}-{version}-windows-x64.zip'
    sums = out / f'{artifact}-{version}-SHA256SUMS.txt'
    for path in (source_zip, portable_zip, sums):
        if path.exists():
            raise RuntimeError(f'拒绝覆盖现有发布文件：{path.name}')
    if not args.skip_build:
        command = ['powershell', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File',
                   ROOT / 'tools/ai/build.ps1', '-Maven', args.maven, '-Cargo', args.cargo,
                   '-TimeoutSeconds', str(args.timeout)]
        if args.offline:
            command.append('-Offline')
        run(command, ROOT, args.timeout * 3 + 20)
        run([sys.executable, 'tools/ai/gravity.py', 'build', '--timeout', str(args.timeout)], ROOT, args.timeout + 15)
    jar = ROOT / 'target' / jar_name
    binaries = [ROOT / 'rust/target/release' / name for name in ('gkt-observe.exe', 'gkt-detail.exe')]
    binaries.append(ROOT / 'rust/gravity/target/release/gkt-gravity.exe')
    for path in [jar, *binaries]:
        if not path.is_file():
            raise RuntimeError(f'缺少构建产物：{path.relative_to(ROOT)}')
    for path in binaries:
        if MACHINE_PATH.search(path.read_bytes().decode('utf-8', errors='replace')):
            raise RuntimeError(f'发布二进制含构建机路径：{path.name}；请使用tools/ai构建入口重新构建。')
    jpackage = shutil.which('jpackage')
    if not jpackage:
        raise RuntimeError('PATH中没有jpackage；请安装完整JDK21或更新版本。')
    with tempfile.TemporaryDirectory(prefix='gekitotsu-release-') as temporary:
        stage = Path(temporary)
        source = stage / 'source.zip'
        run(['git', 'archive', '--format=zip', f'--output={source}', revision], ROOT, 60)
        print('从公开提交装配源码与便携包', flush=True)
        with zipfile.ZipFile(source) as archive, zipfile.ZipFile(source_zip, 'x', compression=zipfile.ZIP_DEFLATED) as target:
            for entry in archive.infolist():
                if entry.is_dir():
                    continue
                payload = archive.read(entry)
                errors = inspect(entry.filename, payload)
                if errors:
                    raise RuntimeError(str(errors))
                target.writestr(f'{artifact}-{version}-source/{entry.filename}', payload)
        inputs = stage / 'input'
        inputs.mkdir()
        shutil.copy2(jar, inputs / jar_name)
        run([jpackage, '--type', 'app-image', '--name', artifact, '--app-version', version,
             '--input', inputs, '--main-jar', jar_name, '--main-class', 'org.example.Main',
             '--icon', ROOT / 'assets/icon.ico', '--dest', stage / 'image',
             '--java-options', '-Dfile.encoding=UTF-8'], ROOT, args.timeout)
        image = stage / 'image' / artifact
        with zipfile.ZipFile(source) as archive:
            archive.extractall(image)
        for pool in ('standard200', 'validation2000', 'external10433'):
            identity = json.loads((image / 'benchmarks' / pool / 'manifest.json').read_text(encoding='utf-8'))
            actual = hashlib.sha256((image / 'benchmarks' / pool / 'opponents.txt').read_bytes()).hexdigest()
            if actual != identity['normalizedFileSha256']:
                raise RuntimeError(f'Git导出的冻结阵集字节不符：{pool}；检查.gitattributes和索引。')
        run([sys.executable, 'benchmarks/external10433/summarize.py'], image, 60)
        for binary in binaries:
            destination = image / binary.relative_to(ROOT)
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(binary, destination)
        # Exercise both observers and the gravity checker from their packaged paths.
        fixture = (image / 'docs/ai/examples/candidate.txt').read_text(encoding='utf-8').strip().rsplit('&', 1)[-1]
        with tempfile.TemporaryDirectory(prefix='gekitotsu-tools-smoke-') as smoke:
            request = Path(smoke) / 'request.json'
            results = []
            for name, mode in (('gkt-observe.exe', None), ('gkt-detail.exe', 'detailed')):
                response = Path(smoke) / (name + '.json')
                value = {'p1': fixture, 'p2': fixture, 'maxFrames': 8}
                if mode:
                    value['mode'] = mode
                request.write_text(json.dumps(value), encoding='utf-8')
                run([image / 'rust/target/release' / name, request, response], image, 30)
                results.append(json.loads(response.read_text(encoding='utf-8'))['result'])
            assert results[0] == results[1]
            response = Path(smoke) / 'gravity.json'
            request.write_text(json.dumps({'formations': [{'name': 'release-smoke', 'code': fixture[:6]}]}), encoding='utf-8')
            run([image / 'rust/gravity/target/release/gkt-gravity.exe', request, response], image, 30)
            assert json.loads(response.read_text(encoding='utf-8'))['formations'][0]['auditPassed']
            run([sys.executable, '-c',
                 "import sys;sys.path.insert(0,'tools/ai');import gravity;"
                 "gravity.BUILD.write_text(gravity.ai.dumps({'policy':gravity.POLICY,'sourceSha256':gravity.source_hash(),'binarySha256':gravity.ai.file_hash(gravity.BINARY)})+'\\n',encoding='utf-8');"
                 "gravity.fingerprint()"], image, 30)
        # Always start with empty GUI inputs; never read the user's working files.
        for name in ('1P.txt', '2P.txt'):
            (image / name).write_bytes(b'')
        licenses = image / 'licenses'
        licenses.mkdir()
        shutil.copy2(ROOT / 'rust/vendor/LICENSE', licenses / 'gkt-core-MIT.txt')
        shutil.copy2(ROOT / 'LICENSE', licenses / 'gekitotsu_lab-MIT.txt')
        java = shutil.which('java')
        if not java:
            raise RuntimeError('PATH中没有java')
        print('核验便携包JAR、观察器与空白GUI输入', flush=True)
        with tempfile.TemporaryDirectory(prefix='gekitotsu-smoke-') as smoke:
            request, response = Path(smoke) / 'request.json', Path(smoke) / 'response.json'
            request.write_text('{"action":"probe"}', encoding='utf-8')
            # Run with the bundled runtime, not the build machine's Java.
            bundled_java = image / 'runtime/bin/java.exe'
            # jpackage may omit command-line launchers; the app launcher forwards --ai.
            launcher = [bundled_java, '-jar', image / 'app' / jar_name] if bundled_java.exists() else [image / f'{artifact}.exe']
            run([*launcher, '--ai', request, response], Path(smoke), 60)
            probe = json.loads(response.read_text(encoding='utf-8'))
            if 'error' in probe or not probe.get('rustAvailable') or probe.get('labVersion') != version or probe.get('kitVersion') != '1.9.1':
                raise RuntimeError(str(probe))
            run([sys.executable, 'tools/ai/ai.py', '--store', Path(smoke) / 'store.sqlite', 'probe'], image, 60, True)
        for path in (image / '1P.txt', image / '2P.txt'):
            assert path.read_bytes() == b''
        forbidden = ('experiments', 'research', 'formations', 'ai-data', 'local-maintenance', 'assets/promotional', '.git', 'config.ini', 'battle_cache.bin')
        assert not any((image / name).exists() for name in forbidden)
        payloads = {}
        for path in sorted(image.rglob('*')):
            if path.is_file():
                payloads[path.relative_to(image).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest = {'project': artifact, 'version': version, 'sourceCommit': revision,
                    'kitVersion': probe['kitVersion'], 'platform': 'windows-x64', 'guiInputs': 'empty', 'files': payloads,
                    'engineRules': {'sourceObservers': 'catalog-hp-v1', 'bundledJNI': 'legacy-149e365'},
                    'smokeChecks': ['packaged-launcher-probe-native-available', 'packaged-python-jar-discovery',
                                    'packaged-observer-detail-results', 'packaged-gravity-and-source-fingerprint',
                                    'empty-gui-inputs', 'no-private-workspace-directories']}
        (image / 'release-manifest.json').write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
        with zipfile.ZipFile(portable_zip, 'x', compression=zipfile.ZIP_DEFLATED) as archive:
            for path in sorted(image.rglob('*')):
                if path.is_file():
                    archive.write(path, path.relative_to(image).as_posix())
    sums.write_text(''.join(f'{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.name}\n'
                            for path in (source_zip, portable_zip)), encoding='utf-8')
    print(json.dumps({'source': source_zip.name, 'portable': portable_zip.name, 'checksums': sums.name,
                      'commit': revision, 'published': False}, ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
