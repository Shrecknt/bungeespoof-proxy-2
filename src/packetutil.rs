use std::error::Error;

use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub async fn send_prefixed_packet(
    connection: &mut TcpStream,
    data: &Vec<u8>,
) -> Result<(), Box<dyn Error>> {
    let mut buffer: Vec<u8> = vec![];
    write_varint(&mut buffer, i32::try_from(data.len())?).await?;
    buffer.write_all(data).await?;

    connection.write_all(&buffer).await?;

    Ok(())
}

// yoinked from https://github.com/mat-1/azalea/blob/1fb4418f2c9cbd004c64c2f23d2d0352ee12c0e5/azalea-buf/src/write.rs#L36
// thanks mat <3
pub async fn write_varint(buf: &mut impl std::io::Write, val: i32) -> Result<(), Box<dyn Error>> {
    let mut buffer = [0];
    let mut value = val;
    if value == 0 {
        buf.write_all(&buffer)?;
    }
    while value != 0 {
        buffer[0] = (value & 0b0111_1111) as u8;
        value = (value >> 7) & (i32::max_value() >> 6);
        if value != 0 {
            buffer[0] |= 0b1000_0000;
        }
        buf.write_all(&buffer)?;
    }
    Ok(())
}

pub async fn read_varint_len(stream: &mut TcpStream) -> Result<(u32, i32), Box<dyn Error>> {
    let mut buf = [0u8];
    let mut res = 0;
    let mut count = 0u32;

    loop {
        stream.readable().await?;
        stream.read_exact(&mut buf).await?;
        res |= (buf[0] as i32 & (0b0111_1111_i32))
            .checked_shl(7 * count)
            .ok_or("Unsupported protocol")?;

        count += 1;
        if count > 5 {
            break Err("Unsupported protocol".into());
        } else if (buf[0] & (0b1000_0000_u8)) == 0 {
            break Ok((count, res));
        }
    }
}
