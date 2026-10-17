//! 激突要塞极速模拟器核心库。
//!
//! oracle = `fixtures/oracle/`（Java 工作区快照，commit f9fccb3 + 未提交改动），
//! 行为唯一规格是那份源码；本库在结果逐位一致的前提下追求速度（见 AGENTS.md）。

#[rustfmt::skip]
pub mod fdlibm_consts;
pub mod decode;
pub mod engine;
pub mod fdlibm;
#[rustfmt::skip]
pub mod gen_consts;
#[rustfmt::skip]
pub mod gen_literals;
pub mod geom;
pub mod jmath;
pub mod rng;
pub mod trig;
pub mod utils;
