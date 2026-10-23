//! Canonical sparse ext4 artifacts retain a VM's boot disk independently of Nix GC.
//! The stream names no files: validated extents are written into one new sparse image.

use super::util::*;
pub use crate::root_artifact::RootDescriptor;
use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use crate::root_artifact::{BLOCK, MAGIC};

/// Encode a regular ext4 image into its canonical sparse identity and optional stream.
pub fn encode(source: &Path, mut target: Option<&mut dyn Write>) -> Result<RootDescriptor> {
    let metadata = std::fs::symlink_metadata(source)?;
    ensure!(metadata.is_file(), "root image must be a regular file");
    let size = metadata.len();
    ensure!(
        size > 0 && size <= 64 * 1024u64.pow(3),
        "root image size out of bounds"
    );
    let mut input = File::open(source)?;
    input.seek(SeekFrom::Start(1080))?;
    let mut signature = [0; 2];
    input.read_exact(&mut signature)?;
    ensure!(
        signature == [0x53, 0xef],
        "root image is not an ext filesystem"
    );

    let mut digest = Sha256::new();
    let mut count = 0;
    let mut emit = |data: &[u8]| -> Result<()> {
        digest.update(data);
        count += data.len() as u64;
        if let Some(output) = &mut target {
            output.write_all(data)?;
        }
        Ok(())
    };
    emit(MAGIC)?;
    emit(&size.to_be_bytes())?;
    let mut offset = 0;
    let mut seek_holes = true;
    let mut buffer = [0; BLOCK as usize];
    while offset < size {
        if seek_holes {
            match rustix::fs::seek(&input, rustix::fs::SeekFrom::Data(offset)) {
                Ok(data) => offset = offset.max(data / BLOCK * BLOCK),
                Err(rustix::io::Errno::NXIO) => break,
                Err(rustix::io::Errno::INVAL | rustix::io::Errno::OPNOTSUPP) => seek_holes = false,
                Err(error) => return Err(error.into()),
            }
        }
        if offset >= size {
            break;
        }
        input.seek(SeekFrom::Start(offset))?;
        let length = (size - offset).min(BLOCK) as usize;
        input.read_exact(&mut buffer[..length])?;
        if buffer[..length].iter().any(|b| *b != 0) {
            emit(&offset.to_be_bytes())?;
            emit(&(length as u32).to_be_bytes())?;
            emit(&buffer[..length])?;
        }
        offset += length as u64;
    }
    Ok(RootDescriptor {
        schema: "airlock.root-artifact.v1".into(),
        sha256: hex(&digest.finalize()),
        logical_bytes: size,
        payload_bytes: count,
        format: "ext4".into(),
        boot_abi: "airlock.nix-closure-root.v1".into(),
    })
}

/// Reconstruct one new sparse image and verify both wire and filesystem identities.
pub fn decode(source: &Path, target: &Path, descriptor: &RootDescriptor) -> Result<()> {
    descriptor.validate()?;
    ensure!(
        std::fs::metadata(source)?.len() == descriptor.payload_bytes,
        "root payload size mismatch"
    );
    let mut input = File::open(source)?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)?;
    let mut digest = Sha256::new();
    let mut header = vec![0; MAGIC.len() + 8];
    input.read_exact(&mut header)?;
    ensure!(
        header.starts_with(MAGIC)
            && u64::from_be_bytes(header[MAGIC.len()..].try_into()?) == descriptor.logical_bytes,
        "invalid sparse root header"
    );
    digest.update(header);
    output.set_len(descriptor.logical_bytes)?;
    let mut end = 0;
    loop {
        let mut record = [0; 12];
        let first = input.read(&mut record[..1])?;
        if first == 0 {
            break;
        }
        input.read_exact(&mut record[1..])?;
        let offset = u64::from_be_bytes(record[..8].try_into()?);
        let size = u32::from_be_bytes(record[8..].try_into()?) as u64;
        ensure!(
            offset >= end
                && offset % BLOCK == 0
                && offset < descriptor.logical_bytes
                && size == BLOCK.min(descriptor.logical_bytes - offset),
            "invalid sparse root extent"
        );
        let mut data = vec![0; size as usize];
        input.read_exact(&mut data)?;
        ensure!(
            data.iter().any(|b| *b != 0),
            "noncanonical zero root extent"
        );
        digest.update(record);
        digest.update(&data);
        output.seek(SeekFrom::Start(offset))?;
        output.write_all(&data)?;
        end = offset + size;
    }
    ensure!(
        hex(&digest.finalize()) == descriptor.sha256,
        "root artifact checksum mismatch"
    );
    output.sync_all()?;
    ensure!(
        encode(target, None)? == *descriptor,
        "materialized root differs from descriptor"
    );
    Ok(())
}
