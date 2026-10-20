//! Write allocated image extents without treating holes as a secure erase.

use super::progress::Progress;
use anyhow::{Context, Result, ensure};
use rustix::fs::{SeekFrom as NativeSeek, seek};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
};

const CHUNK: usize = 8 * 1024 * 1024;

pub(crate) fn extents(source: &File) -> Result<Vec<(u64, u64)>> {
    let size = source.metadata()?.len();
    let mut result = Vec::new();
    let mut offset = 0;
    while offset < size {
        let start =
            match seek(source, NativeSeek::Data(offset)) {
                Ok(start) => start,
                Err(rustix::io::Errno::NXIO) => break,
                Err(error) => return Err(error).context(
                    "Source filesystem does not support sparse image extents; use --full-write.",
                ),
            };
        let end = seek(source, NativeSeek::Hole(start))?.min(size);
        ensure!(
            start >= offset && start < end,
            "Invalid sparse image extent"
        );
        result.push((start, end));
        ensure!(
            result.len() <= 1_000_000,
            "Image has too many sparse extents"
        );
        offset = end;
    }

    Ok(result)
}

pub(crate) fn copy(source: &mut File, destination: &mut File, sparse: bool) -> Result<u64> {
    let initial_size = source.metadata()?.len();
    let ranges = if sparse {
        extents(source)?
    } else {
        vec![(0, source.metadata()?.len())]
    };

    let total: u64 = ranges.iter().map(|(a, b)| b - a).sum();
    let mut progress = Progress::new(
        if sparse {
            "Writing sparse extents"
        } else {
            "Writing image"
        },
        total,
    );
    let mut buffer = vec![0; CHUNK];
    let mut copied = 0;

    for (start, end) in ranges {
        source.seek(SeekFrom::Start(start))?;
        destination.seek(SeekFrom::Start(start))?;
        let mut remaining = end - start;
        while remaining > 0 {
            let size = source.read(&mut buffer[..remaining.min(CHUNK as u64) as usize])?;
            ensure!(size != 0, "Unexpected end of image extent.");
            destination.write_all(&buffer[..size])?;
            copied += size as u64;
            remaining -= size as u64;
            progress.report(copied);
        }
    }

    ensure!(
        source.metadata()?.len() == initial_size,
        "Image changed size while writing"
    );
    progress.report(copied);

    Ok(copied)
}
