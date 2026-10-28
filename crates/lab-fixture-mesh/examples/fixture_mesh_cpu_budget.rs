//! Three serial CPU observations after one warmup per independently authored preset.
//! Includes validation/hashing; not a GPU upload, presented-frame or FPS benchmark.
use lab_fixture::{Fixture, Preset};
use lab_fixture_mesh::{Config, build};
use std::{
    hint::black_box,
    io::{self, Write as _},
    time::Instant,
};

fn main() -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "preset,sample,fixture_generation_validation_ms,mesh_conversion_validation_hash_ms,entities,logical_buffer_bytes,planned_batches"
    )?;
    for preset in Preset::ALL {
        for sample in 0..=3 {
            let start = Instant::now();
            let fixture = Fixture::generate(preset).map_err(io::Error::other)?;
            let generated = start.elapsed();
            let start = Instant::now();
            let mesh = build(&fixture, Config::default()).map_err(io::Error::other)?;
            let converted = start.elapsed();
            if sample > 0 {
                writeln!(
                    out,
                    "{preset:?},{sample},{:.6},{:.6},{},{},{}",
                    generated.as_secs_f64() * 1000.0,
                    converted.as_secs_f64() * 1000.0,
                    mesh.entities().len(),
                    mesh.buffer_bytes(),
                    mesh.planned_batches().len()
                )?;
            }
            black_box((&fixture, &mesh));
        }
    }
    out.flush()
}
