use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

pub fn uncompress_stream(input: &[u8], file_size: usize, mem_size: usize) -> Result<Vec<u8>> {
    let mut never_cancel = || false;
    uncompress_stream_inner(input, file_size, mem_size, None, &mut never_cancel)
}

/// Decompress and hash the exact decompressed payload in one pass. The
/// decompressed bytes remain available for consumers that classify the
/// resource; fingerprint-only callers can use the digest without walking the
/// output a second time.
pub fn uncompress_stream_with_digest(
    input: &[u8],
    file_size: usize,
    mem_size: usize,
) -> Result<(Vec<u8>, [u8; 32])> {
    let mut digest = Sha256::new();
    let mut never_cancel = || false;
    let output = uncompress_stream_inner(input, file_size, mem_size, Some(&mut digest), &mut never_cancel)?;
    Ok((output, digest.finalize().into()))
}

/// Same decoded data and SHA-256 as the normal decoder, but Health can cancel
/// even inside a very large compressed resource, rather than waiting until the
/// whole resource has finished decoding.
pub fn uncompress_stream_with_digest_cancellable<F: FnMut() -> bool>(
    input: &[u8],
    file_size: usize,
    mem_size: usize,
    mut cancelled: F,
) -> Result<(Vec<u8>, [u8; 32])> {
    let mut digest = Sha256::new();
    let output = uncompress_stream_inner(input, file_size, mem_size, Some(&mut digest), &mut cancelled)?;
    Ok((output, digest.finalize().into()))
}

fn uncompress_stream_inner(
    mut input: &[u8],
    file_size: usize,
    mem_size: usize,
    mut digest: Option<&mut Sha256>,
    cancelled: &mut dyn FnMut() -> bool,
) -> Result<Vec<u8>> {
    if input.len() < file_size {
        bail!("input shorter than declared file_size");
    }
    input = &input[..file_size];

    if input.len() < 2 {
        bail!("unexpected EOF in compression header");
    }
    let b0 = input[0];
    input = &input[2..];

    let datalen = if (b0 & 0x80) != 0 { 4 } else { 3 }
        * if (b0 & 0x01) != 0 { 2 } else { 1 };
    if input.len() < datalen {
        bail!("unexpected EOF reading compressed size field");
    }

    let mut real_size = 0usize;
    for &b in &input[..datalen] {
        real_size = (real_size << 8) + b as usize;
    }
    input = &input[datalen..];

    if real_size != mem_size {
        bail!(
            "resource indicates uncompressed size 0x{real_size:X} but index says 0x{mem_size:X}"
        );
    }

    let mut out = Vec::with_capacity(mem_size);
    let mut chunks = 0usize;
    while !input.is_empty() {
        if chunks % 256 == 0 && cancelled() {
            bail!("{}", crate::operation::CANCELLED_ERROR);
        }
        dechunk(&mut input, &mut out, digest.as_deref_mut()).context("dechunk failed")?;
        chunks += 1;
    }
    if cancelled() {
        bail!("{}", crate::operation::CANCELLED_ERROR);
    }
    if out.len() != mem_size {
        bail!(
            "decompressed 0x{:X} bytes, expected 0x{:X}",
            out.len(),
            mem_size
        );
    }
    Ok(out)
}

fn dechunk(
    input: &mut &[u8],
    out: &mut Vec<u8>,
    mut digest: Option<&mut Sha256>,
) -> Result<()> {
    if input.is_empty() {
        bail!("unexpected EOF (no packing byte)");
    }

    let packing = input[0];
    *input = &input[1..];

    let mut copy_size = 0usize;
    let mut copy_offset = 0usize;
    let datalen: usize;

    if packing < 0x80 {
        if input.is_empty() {
            bail!("unexpected EOF in chunk header");
        }
        let b = input[0];
        *input = &input[1..];

        datalen = (packing & 0x03) as usize;
        copy_size = (((packing >> 2) & 0x07) + 3) as usize;
        copy_offset = ((((packing as usize) << 3) & 0x300) | b as usize) + 1;
    } else if packing < 0xC0 {
        if input.len() < 2 {
            bail!("unexpected EOF in chunk header");
        }
        let b0 = input[0];
        let b1 = input[1];
        *input = &input[2..];

        datalen = ((b0 >> 6) & 0x03) as usize;
        copy_size = ((packing & 0x3F) + 4) as usize;
        copy_offset = ((((b0 as usize) << 8) & 0x3F00) | b1 as usize) + 1;
    } else if packing < 0xE0 {
        if input.len() < 3 {
            bail!("unexpected EOF in chunk header");
        }
        let b0 = input[0];
        let b1 = input[1];
        let b2 = input[2];
        *input = &input[3..];

        datalen = (packing & 0x03) as usize;
        copy_size = ((((packing as usize) << 6) & 0x300) | b2 as usize) + 5;
        copy_offset =
            ((((packing as usize) << 12) & 0x10000) | ((b0 as usize) << 8) | b1 as usize) + 1;
    } else if packing < 0xFC {
        datalen = ((packing & 0x1F) as usize + 1) << 2;
    } else {
        datalen = (packing & 0x03) as usize;
    }

    let output_start = out.len();
    if datalen > 0 {
        if input.len() < datalen {
            bail!("unexpected EOF reading literal data");
        }
        out.extend_from_slice(&input[..datalen]);
        *input = &input[datalen..];
    }

    if copy_size > 0 {
        if copy_offset == 0 || copy_offset > out.len() {
            bail!(
                "invalid copy offset {copy_offset} at output pos {}",
                out.len()
            );
        }
        if copy_size < copy_offset && copy_offset > 8 {
            copy_blocks(out, copy_offset, copy_size);
        } else {
            copy_bytes(out, copy_offset, copy_size);
        }
    }

    if let Some(digest) = digest.as_deref_mut() {
        digest.update(&out[output_start..]);
    }

    Ok(())
}

fn copy_blocks(out: &mut Vec<u8>, offset: usize, mut len: usize) {
    while len > 0 {
        let chunk = offset.min(len);
        let start = out.len() - offset;
        // The source and destination may be adjacent, but this branch only
        // handles non-overlapping LZ copies (chunk < offset). Vec's
        // memmove-style range extension avoids allocating a temporary buffer
        // for every chunk of a large compressed resource.
        out.extend_from_within(start..start + chunk);
        len -= chunk;
    }
}

fn copy_bytes(out: &mut Vec<u8>, offset: usize, mut len: usize) {
    if offset == 1 {
        let value = *out
            .last()
            .expect("validated LZ copy offset must reference output");
        out.resize(out.len() + len, value);
        return;
    }
    while len > 0 {
        // Copy one complete available pattern at a time. Recomputing the
        // source after each extension preserves the overlapping LZ semantics
        // (including offset == 1) while replacing a byte-at-a-time loop with
        // a bounded memmove.
        let chunk = offset.min(len);
        let start = out.len() - offset;
        out.extend_from_within(start..start + chunk);
        len -= chunk;
    }
}

#[cfg(test)]
mod tests {
    use super::{
        copy_blocks, copy_bytes, uncompress_stream, uncompress_stream_with_digest,
        uncompress_stream_with_digest_cancellable,
    };
    use sha2::{Digest, Sha256};

    #[test]
    fn non_overlapping_copy_preserves_block_order() {
        let mut output = b"012345".to_vec();
        copy_blocks(&mut output, 3, 5);
        assert_eq!(output, b"01234534534");
    }

    #[test]
    fn overlapping_copy_preserves_repeating_pattern() {
        let mut output = b"ab".to_vec();
        copy_bytes(&mut output, 2, 8);
        assert_eq!(output, b"ababababab");
    }

    #[test]
    fn overlapping_single_byte_copy_preserves_run() {
        let mut output = b"x".to_vec();
        copy_bytes(&mut output, 1, 9);
        assert_eq!(output, b"xxxxxxxxxx");
    }

    #[test]
    fn decompressed_digest_matches_the_exact_payload() {
        let payload = b"abcde";
        let compressed = [
            0x00, 0x00, // header: three-byte decompressed-size field
            0x00, 0x00, 0x05, // exact decompressed size
            0xE0, b'a', b'b', b'c', b'd', // four literal bytes
            0xFD, b'e', // one literal byte
        ];
        let decoded = uncompress_stream(&compressed, compressed.len(), payload.len()).unwrap();
        let (decoded_with_digest, digest) =
            uncompress_stream_with_digest(&compressed, compressed.len(), payload.len()).unwrap();
        assert_eq!(decoded, payload);
        assert_eq!(decoded_with_digest, payload);
        let expected: [u8; 32] = Sha256::digest(payload).into();
        assert_eq!(digest, expected);
        let (streamed, streamed_digest) =
            uncompress_stream_with_digest_cancellable(&compressed, compressed.len(), payload.len(), || false).unwrap();
        assert_eq!(streamed, payload);
        assert_eq!(streamed_digest, digest);
        let error = uncompress_stream_with_digest_cancellable(
            &compressed, compressed.len(), payload.len(), || true
        ).unwrap_err();
        assert!(error.to_string().contains(crate::operation::CANCELLED_ERROR));
    }
}
