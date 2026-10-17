use gkt_diagnostic_core as engine;
#[path="../recording.rs"] mod recording;
fn supports_mode(mode:&str)->bool {mode=="detailed"}
fn plain_output(_req:&recording::Request)->serde_json::Value {unreachable!("详细模式不能关闭观察")}
fn execute(req:&recording::Request,a:&engine::decode::CompiledFort,b:&engine::decode::CompiledFort,rec:&mut recording::Recorder)->(engine::engine::GameTask,engine::engine::Result) {
    engine::engine::GameTask::run_single_detailed(a,b,req.max_frames,req.seed,Some(rec),req.max_detailed_events)
}
fn decorate(out:&mut serde_json::Value,game:&engine::engine::GameTask) {
    let detail=game.detail.as_ref().expect("详细跟踪未启用").borrow();
    detail.append_output(out);
}
fn main() {if let Err(e)=recording::run() {eprintln!("详细对局观察失败：{e}");std::process::exit(2);}}
