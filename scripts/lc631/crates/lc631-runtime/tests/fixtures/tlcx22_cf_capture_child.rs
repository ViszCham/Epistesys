//! 日本語: 実child専用fixture。stdout frameはcompiler metadataではない。
//! English: Actual-child fixture; its stdout frame is not compiler metadata.
use std::io::{Read,Write};
pub const MODE_ENV:&str="EPISTESYS_CF_CAPTURE_FIXTURE_MODE";
pub const MAGIC:&[u8]=b"\0EPI_CF_CAPTURE_V1\0";
pub const PARTIAL:&[u8]=b"{\"$message_type\":\"diagnostic\",\"level\":\"error\",\"message\":\"fixture from_report";
pub const ICE:&[u8]=b"{\"$message_type\":\"diagnostic\",\"level\":\"error: internal compiler error\",\"message\":\"fixture simulated ICE; not rustc\",\"code\":null,\"spans\":[],\"children\":[],\"rendered\":null}\n";

fn frame(payload:&[u8])->std::io::Result<()> {
    let mut out=std::io::stdout().lock();
    out.write_all(MAGIC)?;
    out.write_all(&(payload.len() as u32).to_le_bytes())?;
    out.write_all(payload)?;
    out.flush()
}
fn stop(code:i32)->! {std::process::exit(code)}
pub fn dispatch(mode:&str)->! {
    match mode {
        "metadata"=>{
            let mut input=Vec::new();
            let result=std::io::stdin().take(65537).read_to_end(&mut input);
            if result.is_err() || input.len()>65536 {stop(64);}
            let payload:Vec<u8>=(0..=255).collect();
            if frame(&payload).is_err() {stop(65);}
            stop(0)
        }
        "echo"=>{
            let mut input=Vec::new();
            if std::io::stdin().take(65537).read_to_end(&mut input).is_err()
                || input.len()>65536 {stop(64);}
            if frame(&input).is_err() {stop(65);}
            stop(0)
        }
        "stderr-flood"=>{
            if frame(b"stderr flood entered; fixture bytes").is_err() {stop(65);}
            let mut err=std::io::stderr().lock();
            if err.write_all(PARTIAL).is_err() {stop(65);}
            let block=[b'x';4096];
            for _ in 0..256 {if err.write_all(&block).is_err() {break;}}
            let _=err.flush();
            stop(1)
        }
        "stdout-flood"=>{
            let payload=[b'y';65536];
            let _=frame(&payload);
            stop(0)
        }
        "sleep"|"blocked-stdin"=>{
            if frame(b"fixture is alive; no compiler metadata").is_err() {stop(65);}
            std::thread::sleep(std::time::Duration::from_secs(30));
            stop(0)
        }
        "partial-json"=>{
            if frame(b"partial fixture diagnostic").is_err() {stop(65);}
            let _=std::io::stderr().write_all(PARTIAL);
            let _=std::io::stderr().flush();
            stop(1)
        }
        "ice"=>{
            if frame(b"simulated ICE fixture").is_err() {stop(65);}
            let _=std::io::stderr().write_all(ICE);
            let _=std::io::stderr().flush();
            stop(1)
        }
        "unknown-exit"=>{
            if frame(b"unknown fixture exit").is_err() {stop(65);}
            let _=std::io::stderr().write_all(b"fixture borrow marker is not a compiler rejection\n");
            stop(42)
        }
        _=>stop(64),
    }
}
