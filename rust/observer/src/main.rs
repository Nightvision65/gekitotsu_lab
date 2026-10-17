use gkt_basic_core as engine;
mod recording;
fn supports_mode(mode:&str)->bool {mode=="basic"}
fn execute(req:&recording::Request,a:&engine::decode::CompiledFort,b:&engine::decode::CompiledFort,rec:&mut recording::Recorder)->(engine::engine::GameTask,engine::engine::Result) {
    if req.plain {engine::engine::GameTask::run_single(a,b,req.max_frames,req.seed)}
    else {engine::engine::GameTask::run_single_observed(a,b,req.max_frames,req.seed,Some(rec))}
}
fn decorate(_out:&mut serde_json::Value,_game:&engine::engine::GameTask) {}
fn plain_output(req:&recording::Request)->serde_json::Value {
    let a=gkt_core::decode::compile_fort("1P",&req.p1);let b=gkt_core::decode::compile_fort("2P",&req.p2);
    let (game,r)=gkt_core::engine::GameTask::run_single(&a,&b,req.max_frames,req.seed);
    serde_json::json!({"schemaVersion":"gkt-ai-battle-v1","engine":"vendored-rust-149e365-catalog-hp-v1","mode":"basic","request":req,
        "result":{"status":r.status,"winnerHp":r.winner_hp,"frames":r.frame_passed,"p1Hp":game.hp[0],"p2Hp":game.hp[1],"finalSeed":game.rng_seed},
        "summary":{"complete":true,"initialSeed":null,"unitCount":0,"eventCount":0,"snapshotCount":0,"droppedEvents":0},
        "units":[],"events":[],"timeline":[],"flights":[],"flightSamples":[]})
}
fn main() {if let Err(e)=recording::run() {eprintln!("基础对局观察失败：{e}");std::process::exit(2);}}
