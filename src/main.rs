mod packetutil;
use packetutil::{read_varint_len, send_prefixed_packet, write_varint};

mod server_address;
use server_address::ServerAddress;

mod resolve_address;
use resolve_address::resolve_address;

use clap::Parser;
use serde_json::Value;
use std::{error::Error, str::FromStr};
use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    select,
};
use uuid::Uuid;

async fn handle_login(
    client: &mut TcpStream,
    server: &mut TcpStream,
    username: &str,
    uuid: &str,
    spoofed_hostname: &str,
    spoofed_client_ip: &str,
) -> Result<(), Box<dyn Error>> {
    let uuid = Uuid::from_str(uuid)?;
    let player_uuid = &uuid.to_string().replace('-', "");

    let playername = username.as_bytes();
    let playername_len = playername.len();

    let (_, packet_len) = read_varint_len(client).await?;
    let (_, packet_id) = read_varint_len(client).await?;
    if packet_id == 0x00 {
        // handshake
        let (_, protocol_version) = read_varint_len(client).await?;
        let (_, hostname_len) = read_varint_len(client).await?;
        let mut hostname = vec![0; hostname_len.try_into().unwrap()];
        client.read_exact(&mut hostname).await?;
        let port = client.read_u16().await?;
        let (_, next_state) = read_varint_len(client).await?;

        if next_state == 1 {
            // status
            let mut send_packet = vec![];
            write_varint(&mut send_packet, packet_id).await?;
            write_varint(&mut send_packet, protocol_version).await?;
            write_varint(&mut send_packet, hostname_len).await?;
            send_packet.append(&mut hostname);
            send_packet.write_u16(port).await?;
            write_varint(&mut send_packet, next_state).await?;
            send_prefixed_packet(server, &send_packet).await?;
        } else if next_state == 2 {
            // login

            // handshake segment
            let mut send_packet: Vec<u8> = vec![];
            write_varint(&mut send_packet, packet_id).await?;
            write_varint(&mut send_packet, protocol_version).await?;

            let custom_hostname = format!(
                "{}\0{}\0{}",
                spoofed_hostname, spoofed_client_ip, player_uuid
            );
            let custom_hostname_bytes = custom_hostname.as_bytes();
            write_varint(
                &mut send_packet,
                custom_hostname_bytes.len().try_into().unwrap(),
            )
            .await?;
            send_packet.write_all(custom_hostname_bytes).await?;

            send_packet.write_u16(port).await?;
            write_varint(&mut send_packet, next_state).await?;
            send_prefixed_packet(server, &send_packet).await?;

            // login start segment
            let (_, _packet_len) = read_varint_len(client).await?;
            let (_, packet_id) = read_varint_len(client).await?;
            assert_eq!(packet_id, 0x00);

            let mut send_packet: Vec<u8> = vec![];
            send_packet.write_u8(0x00).await?;
            let (_, _playername_len) = read_varint_len(client).await?;
            let mut _playername = vec![0; _playername_len.try_into().unwrap()];
            client.read_exact(&mut _playername).await?;

            let custom_playername = playername;
            let custom_playername_len = playername_len;

            write_varint(&mut send_packet, custom_playername_len.try_into().unwrap()).await?;
            send_packet.write_all(custom_playername).await?;
            if protocol_version == 759 || protocol_version == 760 {
                let has_sig_data = client.read_u8().await?;
                if has_sig_data != 0x00 {
                    // i dont feel like handling sig data, so ill throw an error instead :3
                    panic!("has_sig_data is not 0x00 (got {has_sig_data:x?}) are you sure you have NCR enabled?");
                }
                send_packet.write_u8(0x00).await?;
            }
            if protocol_version >= 759 {
                let has_uuid = client.read_u8().await?;
                send_packet.write_u8(has_uuid).await?;
                if has_uuid == 0x01 {
                    send_packet.write_all(uuid.as_bytes()).await?;
                }
            }
            send_prefixed_packet(server, &send_packet).await?;
        } else {
            unreachable!("Bad next state {next_state}");
        }
    } else if packet_len == 254 && packet_id == 0xFA {
        // legacy ping
    } else {
        // something weird happened
        // lets pretend everything is cool and normal (:
        let mut send_buf: Vec<u8> = vec![];
        write_varint(&mut send_buf, packet_id).await?;
        let mut remaining_buf: Vec<u8> = vec![0; usize::try_from(packet_len).unwrap()];
        client.read_exact(&mut send_buf).await?;
        send_buf.append(&mut remaining_buf);
        send_prefixed_packet(server, &send_buf).await?;
    }

    Ok(())
}

async fn proxy(
    client: &str,
    server: &str,
    username: &str,
    uuid: &str,
    spoofed_hostname: &str,
    spoofed_client_ip: &str,
) -> Result<(), Box<dyn Error>> {
    let listener = TcpListener::bind(client).await?;
    println!("Listening on interface {client}, proxied to {server}");
    loop {
        let (mut client, peer_addr) = listener.accept().await?;
        println!("Recieved connection from {peer_addr}, proxying connection");
        let mut server = TcpStream::connect(server).await?;
        println!("Connection established to remote server");

        match handle_login(
            &mut client,
            &mut server,
            username,
            uuid,
            spoofed_hostname,
            spoofed_client_ip,
        )
        .await
        {
            Ok(()) => {
                println!("Successfully spoofed login");
            }
            Err(err) => {
                println!("Login failed");
                return Err(err);
            }
        }

        let (mut client_read, mut client_write) = client.into_split();
        let (mut server_read, mut server_write) = server.into_split();

        let c2s = tokio::spawn(async move { io::copy(&mut client_read, &mut server_write).await });
        let s2c = tokio::spawn(async move { io::copy(&mut server_read, &mut client_write).await });

        select! {
            _ = c2s => println!("C2S done!"),
            _ = s2c => println!("S2C done!"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to connect to
    #[arg(short = 'd', long)]
    hostname: String,

    /// Where to listen for connections
    #[arg(short = 'l', long, default_value_t = String::from("0.0.0.0:25570"))]
    listen: String,

    /// Username to log-in as
    #[arg(short = 'u', long)]
    username: String,

    /// UUID to log-in as
    #[arg(short = 'i', long, default_value_t = String::from("from-username"))]
    uuid: String,

    /// Hostname to send to server
    #[arg(short = 'n', long, default_value_t = String::from("0.0.0.0"))]
    send_hostname: String,

    /// Client IP to send to server
    #[arg(long, default_value_t = String::from("192.168.0.1"))]
    client_ip: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    const MOJANG_UUID_API: &str = "https://api.mojang.com/users/profiles/minecraft/";

    let args = Args::parse();

    let uuid = if args.uuid == "from-username" {
        let request_url = format!("{}{}", MOJANG_UUID_API, args.username);
        let text_data = reqwest::get(request_url).await?.text().await?;
        let data: Value = serde_json::from_str(&text_data)?;
        match data["id"].as_str() {
            Some(uuid) => {
                println!(
                    "Got UUID '{uuid}' for username '{}' from Mojang API",
                    args.username
                );
                uuid.to_string()
            }
            None => panic!("Unable to get UUID from Mojang API, got response {data}"),
        }
    } else {
        args.uuid
    };

    let listen_address = ServerAddress::try_from(args.listen.as_str())?;
    let mut host_address = ServerAddress::try_from(args.hostname.as_str())?;
    host_address = resolve_address(&host_address).await?;

    proxy(
        &format!("{}:{}", listen_address.host, listen_address.port),
        &format!("{}:{}", host_address.host, host_address.port),
        &args.username,
        &uuid,
        &args.send_hostname,
        &args.client_ip,
    )
    .await
}
