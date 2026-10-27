//! Deterministic public counts only. No input paths, timings or GPU assertions.
use lab_fixture::{Fixture, Kind, Preset};
use lab_fixture_mesh::{Config, build};
use std::io::{self, Write as _};

fn main() -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    write!(
        out,
        "preset,formula_version,entities,vertices,triangles,logical_buffer_bytes,planned_batches"
    )?;
    for kind in Kind::ALL {
        write!(
            out,
            ",{kind:?}_entities,{kind:?}_vertices,{kind:?}_triangles"
        )?;
    }
    writeln!(out)?;
    for preset in Preset::ALL {
        let fixture = Fixture::generate(preset).map_err(io::Error::other)?;
        let mesh = build(&fixture, Config::default()).map_err(io::Error::other)?;
        write!(
            out,
            "{preset:?},{},{},{},{},{},{}",
            mesh.revision().formula_version,
            mesh.entities().len(),
            mesh.vertices().len(),
            mesh.indices().len() / 3,
            mesh.buffer_bytes(),
            mesh.planned_batches().len()
        )?;
        for kind in Kind::ALL {
            let count = mesh.by_kind()[&kind];
            write!(
                out,
                ",{},{},{}",
                count.entities, count.vertices, count.triangles
            )?;
        }
        writeln!(out)?;
    }
    out.flush()
}
