//! Suppress empty Nix build records without changing stdout or diagnostic content.

use airlock_nix_core::runtime::process::OwnedChild;
use std::{
    io::{self, Read, Write},
    process::{Command, ExitStatus, Stdio},
};

/// Stream human-readable build diagnostics while retaining the child's exit status.
pub(crate) fn run(command: &mut Command) -> io::Result<ExitStatus> {
    let mut child = OwnedChild(command.stderr(Stdio::piped()).spawn()?);
    let stderr = child.0.stderr.take().expect("piped Nix stderr");
    forward(stderr, io::stderr().lock())?;
    child.0.wait()
}

/// Bound buffering even for very large source snippets in Nix error messages.
fn forward(mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    let mut bytes = Vec::new();
    let mut continuation = false;
    let mut buffer = [0; 8192];
    loop {
        let size = match input.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if size == 0 {
            output.write_all(&bytes)?;
            return output.flush();
        }

        for part in buffer[..size].split_inclusive(|byte| *byte == b'\n') {
            let complete = part.ends_with(b"\n");
            if continuation {
                output.write_all(part)?;
            } else {
                bytes.extend_from_slice(part);
                if bytes.len() >= buffer.len() || !empty_record(&bytes, complete) {
                    output.write_all(&bytes)?;
                    bytes.clear();
                    continuation = !complete;
                } else if complete {
                    bytes.clear();
                }
            }
            if complete {
                continuation = false;
            }
        }
        output.flush()?;
    }
}

/// Recognize an empty record or its incomplete prefix, allowing Nix's SGR colors.
fn empty_record(bytes: &[u8], complete: bool) -> bool {
    let mut plain = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"\x1b[") {
            index += 2;
            while index < bytes.len() && (bytes[index].is_ascii_digit() || bytes[index] == b';') {
                index += 1;
            }
            if index == bytes.len() && !complete {
                break;
            }
            if bytes.get(index) != Some(&b'm') {
                return false;
            }
            index += 1;
        } else if bytes[index] == 0x1b && index + 1 == bytes.len() && !complete {
            break;
        } else {
            plain.push(bytes[index]);
            index += 1;
        }
    }

    let separator = plain.iter().position(|byte| *byte == b'>');
    let prefix_end = separator.unwrap_or(plain.len());
    (!complete || separator.is_some())
        && (!complete || prefix_end > 0)
        && plain[..prefix_end]
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+-._?=".contains(byte))
        && separator
            .is_none_or(|separator| plain[separator + 1..].iter().all(u8::is_ascii_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deliver fixed-size fragments to exercise records split across pipe reads.
    struct Fragmented<'a> {
        /// Bytes not yet delivered to the filter.
        remaining: &'a [u8],
        /// Maximum bytes made available by one read.
        size: usize,
    }

    impl Read for Fragmented<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            let size = buffer.len().min(self.size);
            self.remaining.read(&mut buffer[..size])
        }
    }

    /// Empty records disappear while colors, binary bytes, progress and incomplete diagnostics survive.
    #[test]
    fn preserves_useful_output() -> io::Result<()> {
        let input = b"nixos-disk-image>\nnixos-disk-image> \r\n\x1b[32;1mnixos-disk-image>\x1b[0m \n\nnixos-disk-image> copying store\nerror: build failed\nhello >\n\xff>\nimage> final diagnostic";
        let expected = b"\nnixos-disk-image> copying store\nerror: build failed\nhello >\n\xff>\nimage> final diagnostic";
        for size in 1..=input.len() {
            let mut output = Vec::new();
            forward(
                Fragmented {
                    remaining: input,
                    size,
                },
                &mut output,
            )?;
            assert_eq!(output, expected, "read size {size}");
        }
        // A prompt is ordinary output as soon as its punctuation arrives; it
        // must not be held until a newline while the child waits for input.
        assert!(!empty_record(b"Password: ", false));
        Ok(())
    }

    /// Long diagnostics are streamed intact, including continuations that resemble empty records.
    #[test]
    fn preserves_long_lines() -> io::Result<()> {
        let mut input = vec![b'x'; 8192];
        input.extend_from_slice(b"nixos-disk-image>\nimage> useful\n");
        let mut output = Vec::new();
        forward(&input[..], &mut output)?;
        assert_eq!(output, input);

        let mut input = b"image>".to_vec();
        input.resize(8192, b' ');
        input.extend_from_slice(b"useful diagnostic\n");
        output.clear();
        forward(&input[..], &mut output)?;
        assert_eq!(output, input);
        Ok(())
    }
}
