//! GDB Remote Serial Protocol (RSP) client helpers for the SBPF debugger.
//!
//! Provides low-level packet construction, parsing, and convenience wrappers
//! for communicating with the GDB stub exposed by the SBPF VM when the
//! `sbpf-debugger` feature is enabled.
//! 
//! A verbatim from Mollusk, maybe it's best to have this in a separate crate
//! for easy reuse.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

/// Reads a single GDB RSP packet from the stream.
/// Packets are delimited by `#` followed by a 2-character checksum.
pub fn read_reply<R: BufRead>(reader: &mut R) -> std::io::Result<String> {
    let mut buf = Vec::new();

    // Read till the # character.
    reader.read_until(b'#', &mut buf)?;
    // Then read exactly 2 bytes representing the checksum.
    let mut cbuf = [0];
    let _ = reader.read(&mut cbuf)?;
    let _ = buf.write(&cbuf)?;
    let _ = reader.read(&mut cbuf)?;
    let _ = buf.write(&cbuf)?;
    let reply = String::from_utf8_lossy(&buf).to_string();

    Ok(reply)
}

/// Builds a GDB `m` (read memory) command packet for the given address and
/// size.
pub fn gdb_read_memory_cmd(addr: u64, size: usize) -> Vec<u8> {
    let payload = format!("m{addr:x},{size:x}");
    let checksum: u8 = payload.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    format!("${payload}#{checksum:02x}").into_bytes()
}

/// Parses a GDB RSP packet payload into raw bytes.
/// Handles the `+` ack prefix, `$` framing, `O` console output prefix,
/// `#xx` checksum suffix, and `*` run-length encoding.
pub fn gdb_parse_packet(input: &str) -> Option<Vec<u8>> {
    const GDB_RLE_OFFSET: u8 = 29;

    let data = input.strip_prefix('+').unwrap_or(input);
    let data = data.strip_prefix('$')?;
    // might be a console output $O..
    let data = data.strip_prefix('O').unwrap_or(data);
    let data = data.split('#').next()?;

    let mut hex_str = String::new();
    let mut chars = data.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '*' {
            let count_char = chars.next()?;
            let repeat = (count_char as u8).saturating_sub(GDB_RLE_OFFSET) as usize;
            let last = hex_str.chars().last()?;
            for _ in 0..repeat {
                hex_str.push(last);
            }
        } else {
            hex_str.push(c);
        }
    }

    hex::decode(&hex_str).ok()
}

/// Builds a GDB `p` (read single register) command packet for the given
/// register number.
pub fn gdb_read_register_cmd(reg_num: usize) -> Vec<u8> {
    let payload = format!("p{reg_num:x}");
    let checksum: u8 = payload.bytes().fold(0u8, |acc, b| acc.wrapping_add(b));
    format!("${payload}#{checksum:02x}").into_bytes()
}

/// Reads a contiguous memory region from the stub in fixed-size chunks.
/// Splits the request to stay within the stub's packet-size limits.
pub fn stub_read_memory_chunked<R: BufRead, W: Write>(
    writer: &mut W,
    reader: &mut R,
    addr: u64,
    total_size: usize,
    chunk_size: usize,
) -> std::io::Result<Vec<u8>> {
    let mut result = Vec::with_capacity(total_size);
    let mut offset = 0;

    while offset < total_size {
        let size = std::cmp::min(chunk_size, total_size - offset);
        writer.write_all(&gdb_read_memory_cmd(addr + offset as u64, size))?;
        writer.flush()?;

        let reply = read_reply(reader)?;
        let parsed_reply = gdb_parse_packet(&reply).ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid data",
        ))?;
        result.extend_from_slice(&parsed_reply);

        offset += size;
    }

    Ok(result)
}

/// Reads a single 64-bit register value from the stub.
pub fn stub_read_register<R: BufRead, W: Write>(
    writer: &mut W,
    reader: &mut R,
    reg_num: usize,
) -> std::io::Result<u64> {
    let cmd = gdb_read_register_cmd(reg_num);
    writer.write_all(&cmd)?;
    writer.flush()?;
    let reply = read_reply(reader)?;
    let parsed_reply = gdb_parse_packet(&reply).ok_or(std::io::Error::other("invalid packet"))?;
    let data = parsed_reply
        .get(..8)
        .and_then(|s| s.try_into().ok())
        .ok_or(std::io::Error::other("expected 8 bytes"))?;
    let reg_value = u64::from_le_bytes(data);
    Ok(reg_value)
}

/// Fetches debug metadata from the stub via the `qRcmd,metadata` monitor
/// command. Returns key-value pairs (e.g. `program_id`, `cpi_level`, `caller`)
/// that the SBPF runtime passes through the GDB stub.
pub fn stub_fetch_debug_metadata<R: BufRead, W: Write>(
    mut reader: &mut R,
    writer: &mut W,
) -> Result<HashMap<String, String>, std::io::Error> {
    // Take advantage of the metadata monitor command in sbpf
    // to check what the runtime is already passing to us.
    // (lldb) process plugin packet monitor metadata
    // "metadata" -> 6d65746164617461
    writer.write_all(b"$qRcmd,6d65746164617461#9d")?;
    let reply = read_reply(&mut reader)?;
    let parsed = gdb_parse_packet(&reply).ok_or(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        "Can't parse metadata monitor command reply",
    ))?;

    // Drain the OK following.
    let reply = read_reply(&mut reader)?;
    assert_eq!("$OK#9a", reply);

    // We expect a plain text metadata with a newline appended so have it trimmed.
    let parsed = String::from_utf8_lossy(&parsed).trim_end().to_string();
    let parsed_map: HashMap<_, _> = parsed
        .split(';')
        .filter_map(|e| e.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();

    Ok(parsed_map)
}

/// Sends a `vCont;c` (continue all threads) command and waits for the
/// program to exit (`W00` — clean exit with status 0).
pub fn stub_send_continue_command<R: BufRead, W: Write>(
    mut reader: &mut R,
    writer: &mut W,
) -> Result<(), std::io::Error> {
    writer.write_all(b"$vCont;c:p1.-1#0f")?;
    let reply = read_reply(&mut reader)?;
    assert_eq!("+$W00#b7", reply);
    Ok(())
}

/// Connects to the GDB stub with retries (100ms apart).
/// Returns a buffered reader and a writer over the same TCP stream.
pub fn stub_connect<A: ToSocketAddrs>(
    stub_addr: A,
    mut retries: usize,
) -> Result<(BufReader<TcpStream>, TcpStream), std::io::Error> {
    let (reader, writer) = loop {
        match std::net::TcpStream::connect(&stub_addr) {
            Err(e) => {
                if retries == 0 {
                    return Err(e);
                }
                retries -= 1;
                std::thread::sleep(Duration::from_millis(100));
                continue;
            }
            Ok(stream) => break (BufReader::new(stream.try_clone()?), stream),
        }
    };
    Ok((reader, writer))
}