//! The sync wire protocol over a Unix domain socket: std only. Each message is one frame: a 4-byte
//! big-endian length, then that many bytes of UTF-8 JSON. One request per batch:
//! `{"v":1,"op":"sync","records":[{"id":"jack-…","record":"<handoff JSON v1>"}, …]}`
//! answered by `{"v":1,"results":[{"id":"jack-…","ok":true,"detail":"…"}, …]}` (one per record,
//! in order) or `{"v":1,"error":"…"}` when the whole batch can't be taken (e.g. no database).
//! A connection may carry several requests; frames over the configured limit are refused.
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

pub const VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Rec {
    pub id: String,
    pub record: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub v: u32,
    pub op: String,
    pub records: Vec<Rec>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub id: String,
    pub ok: bool,
    #[serde(default)]
    pub detail: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct Response {
    pub v: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<Outcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub fn write_frame(w: &mut impl Write, body: &[u8]) -> std::io::Result<()> {
    let n = u32::try_from(body.len()).map_err(|_| std::io::Error::other("frame too big"))?;
    w.write_all(&n.to_be_bytes())?;
    w.write_all(body)?;
    w.flush()
}

/// Reads one frame; `Ok(None)` at a clean end of stream.
pub fn read_frame(r: &mut impl Read, max: usize) -> std::io::Result<Option<Vec<u8>>> {
    let mut len = [0u8; 4];
    match r.read_exact(&mut len) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(e) => return Err(e),
    }
    let n = u32::from_be_bytes(len) as usize;
    if n > max {
        return Err(std::io::Error::other(format!(
            "frame of {n} bytes is over the {max} limit"
        )));
    }
    let mut body = vec![0u8; n];
    r.read_exact(&mut body)?;
    Ok(Some(body))
}

/// What a receiver does with one batch: one result per record, or an error for the whole batch.
pub type Handler<'a> =
    dyn FnMut(&[(&str, &str)]) -> Result<Vec<Result<String, String>>, String> + 'a;

/// Answers every request on one connection until the peer hangs up.
pub fn answer(
    stream: &mut (impl Read + Write),
    max: usize,
    handle: &mut Handler,
) -> std::io::Result<()> {
    while let Some(body) = read_frame(stream, max)? {
        let resp = match serde_json::from_slice::<Request>(&body) {
            Err(e) => Response {
                v: VERSION,
                error: Some(format!("bad request: {e}")),
                ..Default::default()
            },
            Ok(q) if q.v != VERSION || q.op != "sync" => Response {
                v: VERSION,
                error: Some(format!("unsupported request v{} op {:?}", q.v, q.op)),
                ..Default::default()
            },
            Ok(q) => {
                let recs: Vec<(&str, &str)> = q
                    .records
                    .iter()
                    .map(|r| (r.id.as_str(), r.record.as_str()))
                    .collect();
                match handle(&recs) {
                    Err(e) => Response {
                        v: VERSION,
                        error: Some(e),
                        ..Default::default()
                    },
                    Ok(rs) => Response {
                        v: VERSION,
                        results: q
                            .records
                            .iter()
                            .zip(rs)
                            .map(|(r, x)| {
                                let (ok, detail) = match x {
                                    Ok(d) => (true, d),
                                    Err(e) => (false, e),
                                };
                                Outcome {
                                    id: r.id.clone(),
                                    ok,
                                    detail,
                                }
                            })
                            .collect(),
                        error: None,
                    },
                }
            }
        };
        let out = serde_json::to_vec(&resp).map_err(std::io::Error::other)?;
        write_frame(stream, &out)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_round_trip_and_oversize_is_refused() {
        let mut buf = Vec::new();
        write_frame(&mut buf, b"{}").unwrap();
        write_frame(&mut buf, b"[1]").unwrap();
        let mut r = &buf[..];
        assert_eq!(read_frame(&mut r, 10).unwrap().unwrap(), b"{}");
        assert_eq!(read_frame(&mut r, 10).unwrap().unwrap(), b"[1]");
        assert_eq!(read_frame(&mut r, 10).unwrap(), None);
        let mut r = &buf[..];
        assert!(read_frame(&mut r, 1).is_err());
    }
}
