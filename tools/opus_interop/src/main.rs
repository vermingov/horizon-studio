// The voice chat's Opus as a pipe, for tools/opus_interop.py:
//
//   opus-interop enc IN.f32 OUT.pkts [BITRATE]   encodes as the voice chat sends (VOIP, FEC, 5% loss)
//   opus-interop dec IN.pkts OUT.f32             decodes every packet
//   opus-interop plc IN.pkts OUT.f32             every 7th packet lost: concealed
//   opus-interop fec IN.pkts OUT.f32             every 7th packet lost: recovered from the next one's FEC
//
// PCM is 48 kHz mono f32 little-endian; packets are a u16 (little-endian) length, then the packet.

use hstudio_voice::jitter::{FrameDecoder, Opus};
use hstudio_voice::net::encoder;
use hstudio_voice::proto::{Codec, FRAME_SIZE};
use std::io::Write;

fn packets(data: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 <= data.len() {
        let n = usize::from(u16::from_le_bytes([data[i], data[i + 1]]));
        out.push(data[i + 2..i + 2 + n].to_vec());
        i += 2 + n;
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let [_, mode, input, output, ..] = args.as_slice() else {
        eprintln!("usage: opus-interop enc|dec|plc|fec IN OUT [BITRATE]");
        std::process::exit(2);
    };
    let data = std::fs::read(input).expect("the input file");
    let mut out = std::io::BufWriter::new(std::fs::File::create(output).expect("the output file"));
    let mut write_pcm = |pcm: &[f32]| {
        for s in pcm {
            out.write_all(&s.to_le_bytes()).unwrap();
        }
    };
    match mode.as_str() {
        "enc" => {
            let pcm: Vec<f32> = data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
            let mut enc = encoder(Codec::Voip);
            if let Some(b) = args.get(4) {
                enc.bitrate_bps = b.parse().expect("a bitrate");
            }
            let mut buf = [0u8; 1275];
            let mut packed = Vec::new();
            for frame in pcm.chunks_exact(FRAME_SIZE) {
                let n = enc.encode(frame, FRAME_SIZE, &mut buf).expect("a frame encodes");
                packed.extend_from_slice(&(n as u16).to_le_bytes());
                packed.extend_from_slice(&buf[..n]);
            }
            std::fs::write(output, packed).unwrap();
        }
        "dec" | "plc" | "fec" => {
            let mut dec = Opus::new();
            for (k, p) in packets(&data).iter().enumerate() {
                if mode != "dec" && k % 7 == 3 {
                    if mode == "plc" {
                        write_pcm(&dec.conceal());
                    }
                    continue;
                }
                if mode == "fec" && k % 7 == 4 {
                    write_pcm(&dec.recover(p));
                }
                write_pcm(&dec.decode(p));
            }
        }
        other => panic!("no mode {other}"),
    }
}
