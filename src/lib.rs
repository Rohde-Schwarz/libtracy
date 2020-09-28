// Copyright 2019, 2020 Rohde & Schwarz GmbH & Co KG
//      philipp.stanner@rohde-schwarz.com
//      hagen.pfeifer@rohde-schwarz.com

// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

// FIXME: send_trace_data is currently called whenever data shall be send and
// does not check if a TCP-connection is established. Checking is probably not
// necessary, as on the disconnection of TCP the stream and all the timers get
// turned off, resulting in no more send-events occuring. Check if this is a
// wise approach

mod udp_beacon;
mod tcp_handler;

extern crate mio;
extern crate mio_extras;

use mio::*;
use mio::net::{TcpListener, TcpStream};
use mio_extras::channel;
use mio_extras::channel::{Sender, Receiver};
use mio_extras::timer::{Timer, Timeout};

use std::thread;
use std::time::{Duration, SystemTime};

// for null-pointer-generation
use std::ptr;
use std::str::FromStr;

use std::net::{UdpSocket, SocketAddr};

use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_uint};

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use std::collections::{HashMap, VecDeque};

static SERVER_VERSION: &str = "1.1.0";
static PROTOCOLL_VERSION: &str = "1.1.0";

const MAX_TRACEPOINT_NAME_LEN: usize = 32;
const MAX_SUBMIT_LEN: usize = 2048;

const QUEUE_TOTAL_SIZE: usize = 4096;

const TIMESTAMP_LEN: usize = 8;

const QUEUE_TIMEOUT_IDENT: usize = 42;
const UDP_TIMEOUT_IDENT: usize = 9001;

const CHAN: Token = Token(1);
const TIMER: Token = Token(2);
const CON_NEW: Token = Token(3);
const CON_DATA: Token = Token(4);


enum ChannelMessage {
    Payload(BufferElement),
    NewTracepoint(Tracepoint),
    Terminate,
}


enum TracerState {
    Normal,
    Terminate,
    DataProcessed,
}


// Handler struct passed to the C-Application
struct TracerNg {
    send_to_tracer_thread: Sender<ChannelMessage>,
    client_connected: Arc<AtomicBool>,
    tracepoints: HashMap<String, Arc<AtomicBool>>,
}

// structuring a new tracepoint to be inserted
struct Tracepoint {
    name: String,
    state: Arc<AtomicBool>,
}


// Used to capsule data from init() for tracer-thread
// The app-user is allowed to choose a default interface by passing NULL
struct InitData {
    hostname: String,
    process_name: String,
    send_interval: Duration,
    announce_interval: Duration,
    announce_addr: Option<SocketAddr>,
    announce_iface: Option<String>,
}

// structures data from application in submit-function: tracepoint name,
// associated data and a timestamp when the data was submitted.
// Enqueued in tracer-thread, later serialized and sent over TCP
struct BufferElement {
    tracepoint: String,
    timestamp: SystemTime,
    data: Vec<u8>,
}

impl BufferElement {
    fn len(&self) -> usize
    {
        self.tracepoint.len() + TIMESTAMP_LEN + self.data.len()
    }
}


struct TracerContext {
    app_cfg: InitData,
    poll: Poll,
    buffer: VecDeque<BufferElement>,
    buffer_occupancy: usize,

    rec: Receiver<ChannelMessage>,

    timer: Timer<usize>,
    queue_timeout: Option<Timeout>,
    udp_timeout: Option<Timeout>,

    udp_sock: Option<UdpSocket>,
    listener: TcpListener,
    connection: Option<TcpStream>,
    // TODO: Check if just checking the Hashmap is faster
    client_connected: Arc<AtomicBool>,
    tracepoints: HashMap<String, Arc<AtomicBool>>,
    sequence_no: u64,
}

impl TracerContext {
    fn append(&mut self, element: BufferElement)
    {
        self.buffer_occupancy += element.len();
        self.buffer.push_back(element);
    }

    #[allow(dead_code)]
    fn clear_buffer(&mut self)
    {
        self.buffer.clear();
        self.buffer_occupancy = 0;
    }

    fn check_start_queue_timer(&mut self)
    {
        if self.queue_timeout.is_none() {
            self.queue_timeout = 
                Some(self.timer.set_timeout(self.app_cfg.send_interval,
                                            QUEUE_TIMEOUT_IDENT));
        }
    }

    fn check_stop_queue_timer(&mut self)
    {
        // TODO: Find out why the hell the timer wants to move the timeout,
        // despite only having a reference as parameter
        let tmp = self.queue_timeout.clone();
        if self.queue_timeout.is_some() {
            self.timer.cancel_timeout(&tmp.unwrap());
        }

        self.queue_timeout = None;
    }

    fn check_start_udp_timer(&mut self)
    {
        if self.udp_timeout.is_none() {
            self.udp_timeout = 
                Some(self.timer.set_timeout(self.app_cfg.announce_interval,
                                            UDP_TIMEOUT_IDENT));
        }
    }

    fn check_stop_udp_timer(&mut self)
    {
        // TODO: Find out why the hell the timer wants to move the timeout,
        // despite only having a reference as parameter
        let tmp = self.udp_timeout.clone();
        if self.udp_timeout.is_some() {
            self.timer.cancel_timeout(&tmp.unwrap());
        }

        self.udp_timeout = None;
    }

    // Handler for TCP connections which either failed during usage or which are
    // terminated on purpose by either client or library
    fn close_and_clean_connection(&mut self)
    {
        self.client_connected.store(false, Ordering::SeqCst);

        if let Ok(tmp) = self.connection.as_mut().unwrap().try_clone() {
            let _ = self.poll.deregister(&tmp);
        }

        self.connection = None;
        self.check_stop_queue_timer();

        for value in self.tracepoints.values() {
            value.store(false, Ordering::SeqCst);
        }

        self.check_start_udp_timer();
    }

    fn insert_tracepoint(&mut self, tracepoint: Tracepoint)
    {
        self.tracepoints.insert(tracepoint.name, tracepoint.state);
    }
}


#[no_mangle]
extern "C" fn tracy_init(hostname: *const c_char,
                         process_name: *const c_char,
                         buffer_flush_interval: c_uint, //ms
                         announce_interval: c_uint, //ms
                         announce_iface: *const c_char,
                         announce_mcast_addr: *const c_char,
                         flags: c_int) -> *const TracerNg
{
    let mut announce = false;
    let _ = flags; // flags unused. Avoid compiler warning
    let is_null = hostname.is_null() || process_name.is_null() ||
                    buffer_flush_interval == 0;
    if is_null {
        return ptr::null();
    }

    // There can't be a client connected yet
    let client_connected_thr = Arc::new(AtomicBool::new(false));
    let client_connected_ret = Arc::clone(&client_connected_thr);
    let (snd, rec): (Sender<ChannelMessage>, Receiver<ChannelMessage>) = 
                     channel::channel();

    let init_data = InitData {
        hostname: rawpt_to_str(hostname)
            .expect("tracy: hostname broken."),
        process_name: rawpt_to_str(process_name)
            .expect("tracy: process_name broken"),
        send_interval: Duration::from_millis(buffer_flush_interval as u64),
        announce_interval:
            Duration::from_millis(announce_interval as u64),
        announce_iface: rawpt_to_str(announce_iface),
        announce_addr: rawpt_to_addr(announce_mcast_addr),
    };

    let tracey = TracerNg {
        send_to_tracer_thread: snd,
        client_connected: client_connected_ret,
        tracepoints: HashMap::with_capacity(256),
    };

    if announce_interval > 0 && init_data.announce_iface.is_some() &&
        init_data.announce_addr.is_some() {
        announce = true;
    }

    thread::spawn(move | | tracer_thread_main(init_data, client_connected_thr,
                                              rec, announce));
    // Place the struct on the heap and give control to a raw pointer
    Box::into_raw(Box::new(tracey))
}


fn rawpt_to_addr(cstring: *const c_char) -> Option<SocketAddr>
{
    let s: String = rawpt_to_str(cstring)?;
    string_to_addr(s)
}


fn rawpt_to_str(cstring: *const c_char) -> Option<String>
{
    if cstring.is_null() {
        return None;
    }

    let s: String;
    unsafe {
        s = CStr::from_ptr(cstring).to_string_lossy().into_owned();
    };

    Some(s)
}


fn string_to_addr(s: String) -> Option<SocketAddr>
{
    match SocketAddr::from_str(&s[..]) {
        Ok(addr) => {
            Some(addr)
        },
        Err(e) => {
            eprint!("tracy: Could not resolve user addr.: {}", e);
            None
        },
    }
}


#[no_mangle]
extern "C" fn tracy_register(tracy: *mut TracerNg,
                                 tp_name_param: *const c_char) -> c_int
{
    let tracey: &mut TracerNg;
    let tracepoint: Tracepoint;
    let tp_name: String;
    let tracepoint_state = Arc::new(AtomicBool::new(false));

    if tracy.is_null() {
        eprintln!("tracy_register: Received NULL-Pointer. Ignoring request.");
        return -1;
    }

    unsafe {
        tracey = &mut *tracy;
        tp_name = CStr::from_ptr(tp_name_param).to_string_lossy().into_owned();
    }

    let tp_name_repaired = match fix_tracepoint_str(tp_name) {
        Ok(x) => x,
        _ => return -1,
    };

    tracepoint = Tracepoint {
        name: tp_name_repaired.clone(),
        state: Arc::clone(&tracepoint_state),
    };

    if !tracey.tracepoints.contains_key(&tp_name_repaired) {
        tracey.tracepoints.insert(tp_name_repaired, tracepoint_state);
        let msg = ChannelMessage::NewTracepoint(tracepoint);
        send_to_tracer(&tracey, msg);
        0
    } else {
        eprintln!("tracy_register: Tracepoint already registered.");
        -1
    }
}


// FIXME Rusts os::raw does not contain the C-bool type.
#[no_mangle]
extern "C" fn tracy_tracepoint_enabled(tracy: *const TracerNg,
                                           tp_name_param: *const c_char) -> bool
{
    let tracey: &TracerNg;
    let tp_name: String;

    unsafe {
        tracey = &*tracy;
        tp_name = CStr::from_ptr(tp_name_param).to_string_lossy().into_owned();
    }

    tracepoint_enabled(&tracey, &tp_name)
}


#[no_mangle]
extern "C" fn tracy_finit(tracey: *mut TracerNg)
{
    let tracer: TracerNg;
    // Box takes ownership and deallocates the heap-located TracerNg struct
    // when going out of scope, including the Arc<AtomicBool>
    tracer = unsafe{ *Box::from_raw(tracey) };

    send_to_tracer(&tracer, ChannelMessage::Terminate);
}


// TODO:
// submit checks de facto two times if the client is conncted: Once with
// the AtomicBool client_connected, later again by looking in the HashMap if the
// tracepoint is activated. Maybe only checking the HashMap is better
#[no_mangle]
extern "C" fn tracy_submit(tmp_tracey: *const TracerNg,
                               tp_name_param: *const c_char,
                               data: *const u8,
                               data_len: usize)
{
    let tracey: &TracerNg;
    let buffer_element: BufferElement;
    let tracepoint: String;

    if tmp_tracey.is_null() || tp_name_param.is_null() || data.is_null() {
        eprintln!("tracy_submit: Received NULL-pointer. Ignoring request.");
        return;
    }
    
    if data_len == 0 || data_len > MAX_SUBMIT_LEN {
        eprintln!("tracy_submit: Invalid data_length. Ignoring request.");
        return;
    }

    // Don't pack raw pointer in a Box, otherwise the memory of tmp_tracey
    // would get deallocated when submit returns.
    tracey = unsafe{&*tmp_tracey};
    if !tracey.client_connected.load(Ordering::SeqCst) {
        return;
    }

    unsafe {
        tracepoint = CStr::from_ptr(tp_name_param)
            .to_string_lossy().into_owned();
    }

    let tracepoint_repaired = match fix_tracepoint_str(tracepoint) {
        Ok(x) => x,
        _ => {
            eprintln!("tracy_submit: Tracepoint-String broken. Ignoring.");
            return;
        },
    };

    if !tracepoint_enabled(&tracey, &tracepoint_repaired) {
        return;
    }

    unsafe {
        buffer_element = BufferElement {
            tracepoint: tracepoint_repaired.clone(),
            timestamp: SystemTime::now(),
            data: std::slice::from_raw_parts(data, data_len).to_vec(),
        };
    }

    let msg = ChannelMessage::Payload(buffer_element);
    send_to_tracer(&tracey, msg);
}


fn tracepoint_enabled(tracey: &TracerNg, tracepoint: &String) -> bool
{
    match tracey.tracepoints.get(tracepoint) {
        Some(truth) => truth.load(Ordering::SeqCst),
        None => false,
    }
}


fn send_to_tracer(tracey: &TracerNg, chan_msg: ChannelMessage)
{
    if let Err(e) = tracey.send_to_tracer_thread.send(chan_msg) {
        eprintln!("tracy: Failed to send message to tracer-thread: {:?}", e);
    }
}


fn fix_tracepoint_str(mut tracepoint: String) -> Result<String, ()>
{
    if !tracepoint.is_ascii() {
        eprintln!("tracy: tracepoint is not ascii. Ignoring request.");
        return Err(());
    } 

    if tracepoint.len() > MAX_TRACEPOINT_NAME_LEN {
        eprintln!("tracy: tracepoint-ID-String too long. Limiting to {} chars",
                MAX_TRACEPOINT_NAME_LEN);
        tracepoint.truncate(MAX_TRACEPOINT_NAME_LEN);
    }

    Ok(tracepoint.to_lowercase())
}


fn tracer_thread_main(app_cfg_data: InitData,
                      client_connected_in: Arc<AtomicBool>,
                      rec_param: Receiver<ChannelMessage>,
                      announce: bool)
{
    let mut events = Events::with_capacity(1024);
    let udp_iface = app_cfg_data.announce_iface.clone();

    let mut ctx = TracerContext {
        app_cfg: app_cfg_data,
        poll: Poll::new().expect("tracy: Poll creation"),
        // 'buffer' is holding the structs "BufferElement"
        buffer: VecDeque::with_capacity(1024),
        timer: Timer::default(),
        rec: rec_param,
        queue_timeout: None,
        udp_timeout: None,
        buffer_occupancy: 0,
        udp_sock: None, 
        listener: tcp_handler::init()
            .expect("tracy: Could not bind TCP socket."),
        connection: None,
        client_connected: client_connected_in,
        tracepoints: HashMap::with_capacity(128),
        sequence_no: 0,
    };

    // If the parameters given by the caller indicate that he wishes
    // UDP announcing, try to bind a socket and start announcing
    if announce {
        ctx.udp_sock = match udp_beacon::init(udp_iface) {
            Ok(sock) => Some(sock),
            Err(e) => {
                eprintln!("Could not bind udp sock: {}", e);
                None
            },
        };
        ctx.check_start_udp_timer();
    }

    ctx.poll.register(&ctx.rec, CHAN, Ready::readable(), PollOpt::edge())
        .expect("tracy: Panicked at registering channel in poll.");
    ctx.poll.register(&ctx.timer, TIMER, Ready::readable(), PollOpt::edge())
        .expect("tracy: Panicked at registering timer in poll.");
    ctx.poll.register(&ctx.listener, CON_NEW, Ready::readable(), PollOpt::edge())
        .expect("tracy: Panicked at registering TcpListener in poll.");

    loop {
        ctx.poll.poll(&mut events, None).expect("tracy: Panicked in poll.");

        if let TracerState::Terminate = event_handler(&events, &mut ctx) {
            return;
        }
    }
}


// FIXME: Error handling & return of handler-functions. Especially channel-handler
// signals with its state when main shall terminate. Find a more rusty solution
fn event_handler(events: &Events,
                  mut ctx: &mut TracerContext) -> TracerState
{
    let mut ret = TracerState::Normal;

    for event in events.iter() {
        match event.token() {
            CHAN => match channel_handler(&mut ctx) {
                TracerState::Terminate =>
                    return TracerState::Terminate,
                state => ret = state,
            },
            TIMER => timer_handler(&mut ctx),
            CON_NEW => if ctx.connection.is_none() {
                    tcp_handler::establish_connection(&mut ctx);
                    ctx.check_stop_udp_timer();
            },
            CON_DATA => tcp_handler::receive(&mut ctx),
            _ => (),
        }
    }

    ret
}


fn channel_handler(mut ctx: &mut TracerContext) -> TracerState
{
    let mut ret = TracerState::Normal;

    while let Ok(data) = ctx.rec.try_recv() {
        match data {
            ChannelMessage::Payload(payload) => 
                channel_data_handler(&mut ctx, payload),
            ChannelMessage::NewTracepoint(tracepoint) => 
                ctx.insert_tracepoint(tracepoint),
            ChannelMessage::Terminate => {
                // Send remaining data one last time before killing thread
                if ctx.connection.is_some() {
                    tcp_handler::send_trace_data(&mut ctx);
                }
                return TracerState::Terminate;
            },
        }
        ret = TracerState::DataProcessed;
    }

    ret
}


fn timer_handler(mut ctx: &mut TracerContext)
{
    while let Some(timeout) = ctx.timer.poll() {
        match timeout {
            QUEUE_TIMEOUT_IDENT => {
                ctx.queue_timeout = None;
                tcp_handler::send_trace_data(&mut ctx);
            },
            UDP_TIMEOUT_IDENT => {
                ctx.udp_timeout = None;
                let _ = udp_beacon::announce_tracer(&mut ctx);
                ctx.check_start_udp_timer();
            },
            _ => (),
        }
    }
}


fn channel_data_handler(mut ctx: &mut TracerContext, data: BufferElement)
{
    // Append data in any case, as it is already allocated.
    ctx.append(data);

    if ctx.buffer_occupancy > QUEUE_TOTAL_SIZE {
        ctx.check_stop_queue_timer();
        tcp_handler::send_trace_data(&mut ctx);
    } else {
        ctx.check_start_queue_timer();
    }
}
