// Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
//      philipp.stanner@rohde-schwarz.com
//      hagen.pfeifer@rohde-schwarz.com
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use mio::*;
use mio::net::{TcpListener, TcpStream};

use std::net::{SocketAddr, IpAddr, Ipv6Addr};
use std::io::{ErrorKind, BufReader, Read, Write};
use std::sync::atomic::Ordering;

use std::time::{SystemTime, UNIX_EPOCH};

use std::collections::VecDeque;

use crate::{TracerContext, BufferElement, CON_DATA, QUEUE_TOTAL_SIZE,
            MAX_TRACEPOINT_NAME_LEN};

pub const HEADER_LEN: usize = 12;

// magic nr: 'RuSt'
pub const MAGIC_NUMB: [u8; 4] = [0x52, 0x75, 0x53, 0x74];
const REC_BUF_SZ: usize = 4096;

#[repr(u16)]
enum Command {
    TracepointListRequest       = 1,
    TracepointListReply         = 2,
    TracepointEnableRequest     = 3,
    TracepointDisableRequest    = 4,
    TracePush                   = 5,
    Invalid                     = 42,
}


pub(crate) fn init() -> Option<TcpListener>
{
    let mut listener: Option<TcpListener> = None;
    // random port to minimize risk for conflicts
    for port in 61455u16..u16::max_value() {
        let addr = SocketAddr::new(
            IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)), port);

        if let Ok(l) = TcpListener::bind(&addr) {
            println!("tracy: TCP: Bound to port number {} on all interfaces.",
                     port);
            listener = Some(l);
            break;
        }
    }

    listener
}


pub(crate) fn establish_connection(mut ctx: &mut TracerContext)
{
    match ctx.listener.accept() {
        Ok((socket, _addr)) => {
            let temp_con = socket.try_clone().unwrap();
            ctx.connection = Some(socket);
            ctx.client_connected.store(true, Ordering::SeqCst);
            ctx.poll.register(&temp_con,
                CON_DATA,
                Ready::readable(),
                PollOpt::edge())
                .expect("Panicked at registering socket in poll.");
        },
        Err(_) => eprintln!("tracy: Could not establish connection."),
    }
}


pub(crate) fn receive(mut ctx: &mut TracerContext)
{
    let mut reader = BufReader::with_capacity(REC_BUF_SZ,
                                              ctx.connection.as_mut().unwrap()
                                              .try_clone().unwrap());
    let mut header: [u8; 12] = [0; 12];

    loop {
        if let Err(e) = reader.read_exact(&mut header) {
            if e.kind() != ErrorKind::WouldBlock {
                ctx.close_and_clean_connection();
            }
            return;
        }

        // In case of invalid header: Close the connection
        let (cmd, len) = match check_parse_header(&header) {
            Ok((a, b)) => (a, b),
            Err(_) => {
                ctx.close_and_clean_connection();
                read_empty(&mut reader, &mut ctx);
                return;
            },
        };

        execute_command(&mut ctx, cmd, len, &mut reader);
    }
}


fn execute_command(mut ctx: &mut TracerContext,
                   cmd: Command,
                   len: u32,
                   mut reader: &mut BufReader<TcpStream>)
{
    match cmd {
        Command::TracepointListRequest => send_tracepoint_list(&mut ctx),
        Command::TracepointEnableRequest =>
            set_tracepoints(&mut ctx, len, &mut reader, true),
        Command::TracepointDisableRequest =>
            set_tracepoints(&mut ctx, len, &mut reader, false),
        _ => (), // can never occur, because check_parse_header()
    }
}


fn send_tracepoint_list(mut ctx: &mut TracerContext)
{
    let mut msg: VecDeque<u8> = VecDeque::with_capacity(1024);

    for tracepoint in ctx.tracepoints.keys() {
        let len = tracepoint.len() as u16;

        for b in len.to_be_bytes().iter() {
            msg.push_back(*b);
        }

        for b in tracepoint.as_bytes().iter() {
            msg.push_back(*b);
        }
    }

    push_front_header(&mut msg, Command::TracepointListReply);

    if send_slices(&mut ctx, &msg).is_err() {
        ctx.close_and_clean_connection();
    }
}


pub(crate) fn send_trace_data(mut ctx: &mut TracerContext)
{
    let mut que: VecDeque<u8> = VecDeque::with_capacity(QUEUE_TOTAL_SIZE);
    let mut last_was_complete = true;

    // Take first element of buffer, if one exists
    while let Some(front) = ctx.buffer.get(0) {
        // If there's space in the send-buffer, fill it, otherwise append the
        // header to the front and send the data
        if front.len() + que.len() + HEADER_LEN < QUEUE_TOTAL_SIZE {
            encode_append_trace_data(&mut que, ctx.buffer.pop_front().unwrap());
            last_was_complete = false;
        } else {
            push_front_header(&mut que, Command::TracePush);

            if send_slices(ctx, &que).is_err() {
                ctx.close_and_clean_connection();
                return;
            }

            que.clear();
            last_was_complete = true;
        }
    }

    if !last_was_complete {
        push_front_header(&mut que, Command::TracePush);

        if send_slices(&mut ctx, &que).is_err() {
            ctx.close_and_clean_connection();
        }
    }
}


// FIXME: Take care of signaling the application that the client is not
// accepting data anymore (WouldBlock)
//
// Necessary because you can't send a VecDeque (Ringbuffer) with the default
// write functions
//
// In Case of WouldBlock, most likely the client set the window size to 0.
fn send_slices(ctx: &mut TracerContext, que: &VecDeque<u8>) ->
    Result<(), std::io::Error>
{
    let (first, second) = que.as_slices();
    let mut send_buf: Vec<u8> = Vec::with_capacity(que.len());

    // We assume that allocating & copying is less expensive than two syscalls
    send_buf.extend_from_slice(first);
    send_buf.extend_from_slice(second);

    if let Err(e) = ctx.connection.as_mut().unwrap().write_all(&send_buf) {
        match e.kind() {
            ErrorKind::WouldBlock => (),
            _ => return Err(e),
        }
    }

    Ok(())
}


fn push_front_header(que: &mut VecDeque<u8>, cmd: Command)
{
    // flags are currently unused
    let flags: u16 = 0;
    let length = que.len() as u32;
    for byte in length.to_be_bytes().iter().rev() {
        que.push_front(*byte);
    }

    let tmp = cmd as u16;
    for byte in tmp.to_be_bytes().iter().rev() {
        que.push_front(*byte);
    }

    for byte in flags.to_be_bytes().iter().rev() {
        que.push_front(*byte);
    }

    for byte in MAGIC_NUMB.iter().rev() {
        que.push_front(*byte);
    }
}


// Consumes ownership of bufelm
fn encode_append_trace_data(que: &mut VecDeque<u8>, bufelm: BufferElement)
{
    let tp_len = bufelm.tracepoint.len() as u16;
    let tp_len_arr = tp_len.to_be_bytes();
    for byte in tp_len_arr.iter() {
        que.push_back(*byte);
    }

    for letter in bufelm.tracepoint.into_bytes() {
        que.push_back(letter);
    }

    let timestamp = timestamp_to_u64(bufelm.timestamp).to_be_bytes();
    for byte in timestamp.iter() {
        que.push_back(*byte);
    }

    let data_len = bufelm.data.len() as u16;
    let data_len_arr = data_len.to_be_bytes();
    for byte in data_len_arr.iter() {
        que.push_back(*byte);
    }

    // Take by reference with iter, so only one large deallocation at the end
    for byte in bufelm.data.iter() {
        que.push_back(*byte);
    }
}


fn set_tracepoints(ctx: &mut TracerContext, len: u32,
                       reader: &mut BufReader<TcpStream>,
                       state: bool)
{
    let mut i: u32 = 0;
    let mut tp_name_arr = [0u8; MAX_TRACEPOINT_NAME_LEN];
    let mut tp_name: &str;
    let mut name_len_arr = [0u8; 2];
    let mut name_len: u16;

    while i < len {
        if reader.read_exact(&mut name_len_arr).is_err() {
            ctx.close_and_clean_connection();
            return;
        }

        name_len = u16::from_be_bytes(name_len_arr);
        i += 2;

        if name_len > MAX_TRACEPOINT_NAME_LEN as u16 {
            eprintln!("tracy: Client violated protocol. Received invalid TP-Name\
                 length: {}", name_len);
            ctx.close_and_clean_connection();
            return;
        }

        if reader.read_exact(&mut tp_name_arr[..name_len as usize]).is_err() {
            ctx.close_and_clean_connection();
            return;
        }
        i += name_len as u32;

        // Convert the received bytes to string-slice
        tp_name = std::str::from_utf8(&tp_name_arr[..name_len as usize])
            .unwrap_or_default();

        if let Some(val_ref) = ctx.tracepoints.get_mut(tp_name) {
            val_ref.store(state, Ordering::SeqCst);
        }

        tp_name_arr = [0u8; MAX_TRACEPOINT_NAME_LEN];
    }
}


// reads the socket empty and throws the data away
// Closes connection if there's a problem other than WouldBlock
fn read_empty(reader: &mut BufReader<TcpStream>, ctx: &mut TracerContext)
{
    // TODO: Which size on the stack is acceptable?
    let mut trash: [u8; 64] = [0u8; 64];

    loop {
        match reader.read(&mut trash) {
            Ok(n) => if n == 0 { return },
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock => return,
                _ => {
                    eprintln!("tracy: Read error: {}", e);
                    ctx.close_and_clean_connection();
                    return;
                },
            },
        }
    }
}


fn check_parse_header(header: &[u8; 12]) -> Result<(Command, u32), ()>
{
    let mut magic_no: [u8; 4] = [0; 4];
    let mut flags: [u8; 2] = [0; 2];
    let mut command: [u8; 2] = [0; 2];
    let mut length: [u8; 4] = [0; 4];

    for i in 0..4 {
        magic_no[i] = header[i];
    }

    if !check_magic_number(magic_no) {
        return Err(());
    }

    for i in 4..6 {
        flags[i-4] = header[i];
    }

    for i in 6..8 {
        command[i-6] = header[i];
    }

    for i in 8..12 {
        length[i-8] = header[i];
    }

    let len = u32::from_be_bytes(length);
    let flags = u16::from_be_bytes(flags);
    let cmd = u16::from_be_bytes(command);

    // Check if client performs one of the permitted commands and check if the
    // data-length for these cases makes sense
    let cmd = cmd_number_to_enum(cmd);
    if check_cmd_validity(&cmd, len).is_err() {
        eprintln!("Tracy: Received invalid command.");
    }
    check_flags(flags)?;

    Ok((cmd, len))
}


// Flags are currently unused. If they're not all 0, reject request
fn check_flags(flags: u16) -> Result<(), ()>
{
    if flags != 0 {
        eprintln!("Tracy: Received header flags invalid.");
        Err(())
    } else {
        Ok(())
    }
}


fn cmd_number_to_enum(cmd: u16) -> Command
{
    match cmd {
        cmd if cmd == Command::TracepointListRequest as u16 =>
            Command::TracepointListRequest,
        cmd if cmd == Command::TracepointEnableRequest as u16 =>
            Command::TracepointEnableRequest,
        cmd if cmd == Command::TracepointDisableRequest as u16 =>
            Command::TracepointDisableRequest,
        cmd if cmd == Command::TracepointListReply as u16 =>
            Command::TracepointListReply,
        cmd if cmd == Command::TracePush as u16 => 
            Command::TracePush,
        _ => 
            Command::Invalid,
    }
}


fn check_cmd_validity(cmd: &Command, len: u32) -> Result<(), ()>
{
    match cmd {
        Command::TracepointListRequest => 
            if len != 0 {
                Err(())
            } else {
                Ok(())
            },
        Command::TracepointEnableRequest => 
            if len == 0 {
                Err(())
            } else {
                Ok(())
            },
        Command::TracepointDisableRequest =>
            if len == 0 {
                Err(())
            } else {
                Ok(())
            },
        // Client is only allowed to give the upper commands
        _ => Err(())
    }
}


fn timestamp_to_u64(time: SystemTime) -> u64
{
    // as_nanos() method is still nightly, so do this workaround
    match time.duration_since(UNIX_EPOCH) {
        Ok(n) => {
            let secs = n.as_secs(); // is already u64
            let nanos = n.subsec_nanos() as u64;

            ((secs * 1e9 as u64) + nanos)
        },
        Err(_) => 0,
    }
}


fn check_magic_number(number: [u8; 4]) -> bool
{
    number[0]==MAGIC_NUMB[0] && number[1]==MAGIC_NUMB[1] 
        && number[2]==MAGIC_NUMB[2] && number[3]==MAGIC_NUMB[3]
}
