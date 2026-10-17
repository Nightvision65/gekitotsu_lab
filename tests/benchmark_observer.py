"""同一批冻结对局按轮换顺序实测无观察/基础/关闭半血的端到端开销。"""
import argparse
import os
from pathlib import Path
import statistics
import tempfile
import time
from verify_integration import ai,observe,ROOT

def main():
    if os.environ.get('CONDA_DEFAULT_ENV')!='opencode':raise SystemExit('请先conda activate opencode')
    p=argparse.ArgumentParser();p.add_argument('--out',required=True);p.add_argument('--repeats',type=int,default=5);args=p.parse_args()
    out=Path(args.out)
    if out.exists():raise SystemExit('拒绝覆盖性能证据')
    candidate=ai.formations(ROOT/'docs/ai/examples/candidate.txt')[0]['code']
    pool=ai.formations(ROOT/'tests/fixtures/standard200.txt')[::10]
    modes={'plain':{'plain':True},'basic':{},'basic-no-half':{'recordHalfHp':False}}
    times={key:[] for key in modes};bytes_={key:0 for key in modes}
    with tempfile.TemporaryDirectory(prefix='gkt-bench-') as folder:
        folder=Path(folder)
        for index,mode in enumerate(modes):observe({'p1':candidate,'p2':pool[0]['code'],**modes[mode]},folder,f'warm-{index}')
        for repeat in range(args.repeats):
            totals={key:0.0 for key in modes}
            for i,opponent in enumerate(pool):
                expected=None
                order=list(modes);rotation=(repeat+i)%3;order=order[rotation:]+order[:rotation]
                for mode in order:
                    req={'p1':candidate,'p2':opponent['code'],'maxFrames':65536,'snapshotInterval':120,**modes[mode]}
                    start=time.perf_counter();data=observe(req,folder,f'{repeat}-{i}-{mode}');elapsed=time.perf_counter()-start
                    totals[mode]+=elapsed;bytes_[mode]+=len(ai.dumps(data).encode())
                    if expected is None:expected=data['result']
                    assert data['result']==expected
            for mode in modes:times[mode].append(totals[mode])
            print(f'性能轮次{repeat+1}/{args.repeats}：{ai.dumps(totals)}',flush=True)
    medians={key:statistics.median(value) for key,value in times.items()}
    result={'matchesPerRepeat':len(pool),'repeats':args.repeats,'observerSha256':ai.file_hash(ROOT/'rust/target/release/gkt-observe.exe'),
        'fixtureSha256':ai.file_hash(ROOT/'tests/fixtures/standard200.txt'),'method':'轮换顺序；包含进程启动、模拟、JSON落盘与读取；不含SQLite写入；先预热',
        'seconds':times,'medianSeconds':medians,'basicOverheadPercent':100*(medians['basic']/medians['plain']-1),
        'halfHpIncrementalPercent':100*(medians['basic']/medians['basic-no-half']-1),'rawBytesAcrossRepeats':bytes_}
    out.parent.mkdir(parents=True,exist_ok=True);out.write_text(ai.dumps(result),encoding='utf-8');print(ai.dumps(result),flush=True)

if __name__=='__main__':main()
